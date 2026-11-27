# Vertrag Paketbau, Lieferumfang und Prüfbefehl (Fassung 1)

Stand 20.11.2026, Meilenstein M1 (Probelauf mit dem deb für Ubuntu 26.04) und M4 (Paket und Betrieb). Die Nummer im Kreis der Verträge 1 bis 5 vergibt das Projekt. **Reihenfolge der Teile (Projektentscheid):** Teil A das deb für Ubuntu 26.04.1 LTS (M1, erster Test, eine Person, ein Laptop), Teil B das Windows-Paket (MSI), Teil C das macOS-Paket (pkg); danach die gemeinsamen Abschnitte (Manifest, Lieferumfang, Stückliste, Prüfbefehl, Sicherung).
Der Vertrag behandelt Paketbau, Stückliste, Prüfbefehl und Sicherung. Er behandelt die
Anforderungen A19, A20, A60, A69, A72, A73, A75, A108, A116, A122, A126 und A139. Der Vertrag steht vor dem Code. Alle Fälle
sind heute rot, weil es noch keine Umsetzung gibt. Verdeckte Fälle und Mutanten folgen mit dem Bau. Fälle, die nur unter
Windows laufen, stehen als Außentest für die Windows-VM im Abschnitt „Außentests“.

**Nachtrag Halter (23.11.2026, Schritt I, Linux):** Das deb liefert den Halter (`rust/spec/halter/README.md`,
Abschnitt 2.3, Nachtrag B05) als Programm `/usr/lib/werkbank/werkbank-halter` mit der Benutzereinheit `wb-halter.service`, und
`postinst` schreibt `/etc/werkbank/halter.json` (Eigentümer `root`). Die Anmeldung der Copilot CLI liegt nicht mehr in
`/var/lib/werkbank/<nutzer>/copilot.schluessel`, sondern als `anmeldung.copilot` in der Ablage des Halters
`<heim>/.local/state/agent-workbench/halter/geheim/` (Anmelde-Vertrag, Abschnitt 2). Die achte Prüfung von `werkbank-pruefen`
prüft diesen Ort. Entscheidung 4 (kein Konto, keine Gruppe) gilt weiter; ein Systemnutzer ist Schritt II und liegt beim Betreiber.
Was sich ändert, steht an der Stelle mit „alt → neu“; die Fälle heißen im Namen `halter` oder `ablage`.

**Nachtrag Rolle aus der Gruppe (25.11.2026, Projektentscheid, 25.11.2026, Schritt I, Linux):** Der Halter nimmt die Rolle eines Peers
aus dessen effektiver gid (Halter-Vertrag 2.3 und 2.3a). Dafür legt `postinst` die Systemgruppe `wb-bruecke` an (ohne Konto, ohne
Mitglied) und setzt `/usr/lib/werkbank/werkbank-modellweg` über `dpkg-statoverride` auf `root:wb-bruecke` mit Modus 2755; das Archiv
selbst bleibt `root:root` 0755 (Stufe 9 von Teil A unverändert). Schritt 2c schreibt `/etc/werkbank/halter.json` jetzt in der Form
`$defs/konfig` des Halter-Vertrags. `purge` entfernt den Eintrag von `dpkg-statoverride` und die Gruppe. `werkbank-pruefen` meldet eine
Rollengruppe mit Mitglied, Passwort oder falscher Form und ein Rollenprogramm ohne `root:wb-bruecke` 2755 als Befund (Teil der Prüfung
`dienste`). Die Fälle heißen im Namen `rolle`. Was sich ändert, steht an der Stelle mit „alt → neu“.

**Nachtrag Erststart (25.11.2026, Entscheid der Leitung, Schritt I, Linux):** Das Paket liefert das Erststart-Profil von Vertrag 2
(`rust/spec/einstellungen/erststart.json`, Abschnitt „Erststart-Profil“) als eigene Datei `/usr/share/werkbank/probelauf/erststart.json`
neben der Vorführ-Vorlage `einstellungen.json`. `postinst` baut aus ihm `/etc/werkbank/halter.json` (Schritt 2c): am Modell-Host
`api.individual.githubcopilot.com` genau die zwei Pfade `GET /models` und `POST /chat/completions` und der Schalter
`erststart_antwortform` des Halters für genau diese zwei (Halter-Vertrag 2.3a und 4). Aus demselben Profil kopiert `postinst` die
Arbeitskopie der Einstellungen (Schritt 2); so tragen Route, Modell (`gpt-6-luna`, nie `auto`) und Pfade dieselbe Vorlage. Grund des
Paketwegs: In Schritt I ändert niemand `halter.json` (Abschnitt „Welche Felder der Admin setzt“); ein Weg von Hand widerspräche dem.
Die Vorführ-Vorlage bleibt mit der Quelle `vorfuehrung.json` im Paket (Prüfung 5, `paket.vorfuehrung_einstellung`, Annahme 9).

**Nachtrag Schreibhelfer (27.11.2026, Entscheid des Betreibers, 27.11.2026, Linux):** Das deb liefert den Schreibhelfer der
Firmenvorgabe `/usr/lib/werkbank/werkbank-vorgabe` unter dem Systemnutzer `werkbank-vorgabe`, gestartet über
`werkbank-vorgabe.socket`. Ordner und Datei der Firmenvorgabe gehören dem Helfernutzer (`/etc/werkbank/vorgabe/firmenvorgabe.json`,
`0644`). Konto, Einheiten, Rechte, Erstausstattung und Entfernen stehen in Teil A3; Protokoll und Codes in
`rust/spec/regelebenen/README.md`, Abschnitt 15.12. Nachtrag (Entscheid des Betreibers, 27.11.2026): Der Ordner des Helfers
gehört dem Helfernutzer (OPEN 33 a geschlossen). `postinst` trägt das installierende Konto als ersten Admin ein, nur wenn noch keine
Firmenvorgabe da ist (Teil A3, „Erster Admin“; OPEN 33 b geschlossen).

Was dieser Vertrag nicht tut: Er baut kein Paket, schreibt keinen Rust-Code und ändert kein System. Die Sicherung und
Wiederherstellung (A19) ist nur eine Schnittstelle mit Zusagen; der Bau folgt später.

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `manifest.schema.json` | Form des Manifests als JSON Schema 2020-12 |
| `referenz.py` | Referenzprüfer in Python (Standardbibliothek): Manifest, Listen, Stückliste, Steuerdatei, Entfernen, Einrichten und Prüfbefehl; legt die Reihenfolge der Stufen fest |
| `faelle/<name>.json` | Eingabe; Name beginnt mit der Form: `manifest-`, `liste-`, `stueckliste-`, `steuerdatei-`, `profil-`, `entfernen-`, `remove-`, `einrichten-`, `bauwerkzeuge-`, `pruefung-` |
| `erwartet/<name>.json` | `{"gueltig": true}`, `{"gueltig": false, "code": "paket.<code>"}` oder bei `pruefung` `{"rc", "strom", "zeilen"}` |
| (entfernt) `faelle-ausstehend/`, `erwartet-ausstehend/` | Seit der Code der Crates den Nachtrag Halter besteht, stehen dessen Fälle in `faelle/` und `erwartet/`; die beiden Ordner sind entfernt (alt: dort lagen die Fälle, die der damalige Code nicht bestand). |
| `bau-faelle.py` | erzeugt `faelle/` und `erwartet/` (die Menge `AUSSTEHEND` ist leer); die erwarteten Werte stehen dort von Hand, unabhängig von `referenz.py` |
| `pruefen.py` | prüft Paarung, Form, Referenz, Schema und die Deckung der Codes durch Fälle |

Form der Fälle:

- `manifest`: `text` (UTF-8) oder `b64` (rohe Bytes), optional `auffuellen` (Zahl der Leerzeichen, die am Ende folgen; so bleibt der Fall `zu_gross` klein).
- `liste`: `liste` (Positivliste als Text), `ausschluss` (Ausschlussliste als Text), `quellen` (die Quellen, die im Arbeitsbaum vorhanden sind), `baum` (die Dateien des gestuften Baums, aus dem das Paket entsteht).
- `stueckliste`: `text` (Ausgabe von `wb lizenzen`) und `im_paket` (die Bibliotheken im Paket als `name` und `version`).
- `steuerdatei`: `text` (die Steuerdatei `DEBIAN/control`), `manifest_version` und optional `geruest` (wahr beim Gerüstbau; ohne Angabe gilt: nein).
- `profil`: `text` (eine Profildatei des Pakets).
- `remove`: `nutzer`, `marke`, `nach_installation` und `nach_remove` (Listen von Einträgen), `meldung` (Ausgabe des Entfernens) und `journal`.
- `entfernen`: `nutzer`, `ergebnisdatei` (der Ort von `--ergebnis`), `vorher` und `nachher` (Listen von Einträgen des Systems vor der Installation und nach `apt purge`).
- `einrichten`: `nutzer`, `marke` (der Test-Schlüssel) und die beobachteten Stellen `stdout`, `stderr`, `journal`, `argv`, `umgebung`, `protokolle`, `ergebnisdatei`, dazu `echo` und `datei` (`modus`, `eigentuemer`, `inhalt`).
- `bauwerkzeuge`: `text` (die Liste der Bauwerkzeuge einer Plattform) und `plattform`.
- `laufkonfig`: `nutzer` (oder `null`), `uid`, `heim` (aus `passwd`), `harness` (je gefundenem Harness `programm` und `node`, beide als echte Pfade), `vorher` (frühere Zulassung der CA oder `null`), `vorher_form` (optional: `indent1`, `zeilen`, `doppelt`, `als-wert`, `leerzeichen-im-wert`; die frühere Datei wurde mit einem anderen Werkzeug neu geschrieben); die Erwartung ist `datei` (der Inhalt von `/etc/werkbank/lauf.json` als Objekt oder `null`), dazu `modus` und `eigentuemer`. Siehe „lauf.json“.
- `systemnutzer` (Schritt II): `vorher` und `nachher` (je `konten` und `gruppen`) und `preinst_rc`. Siehe Teil A2.
- `umzug` (Schritt II): `nutzer`, `quelle_vorher`, `quelle_nachher`, `ziel_vorher`, `ziel_nachher`, `journal`, `postinst_rc`. Siehe Teil A2.
- Feld `schritt` (Schritt II, optional in `liste`, `remove` und `entfernen`): mit dem Wert `2` gilt Teil A2, dazu in `liste` das Feld `gruppen`, in `entfernen` die Felder `rest` und `journal`. Ohne das Feld gilt Teil A.
- `pruefung`: `aufruf` (Argumente nach dem Prüfbefehl) und `lage` (der Zustand des Rechners, den die Einzelprüfungen gemeldet haben; `plattform` ist `windows`, `linux` oder `macos`).

## Teil A: Linux-Paket (deb für Ubuntu 26.04.1 LTS, x86_64), Meilenstein M1: der Probelauf

Das deb ist der erste Meilenstein (Entscheidung des Betreibers). Die MSI folgt danach (Teil B, unverändert). Der Betreiber fährt den Probelauf selbst auf seinem Ubuntu-26.04-Laptop und meldet sich dort mit einem Organisationskonto bei der Copilot CLI an. Von uns greift nichts auf das Gerät zu. Er bekommt **ein deb** und die Zeilen der Anleitung unten. Anforderungen (20.11.2026): Installation ohne Netz; Software bevorzugt als signiertes deb; der System-Truststore gilt immer, es gibt kein festes CA-Bündel; `sudo` ist vorhanden.

**Namen sind Arbeitsnamen** (Projektentscheid): Paket `werkbank`, Befehle `werkbank-probelauf` und `werkbank-pruefen`, Programme unter `/usr/lib/werkbank/`, feste Daten unter `/usr/share/werkbank/probelauf/`, alles je Nutzer Veränderliche unter `/var/lib/werkbank/<nutzer>/`. Ändert sich ein Name, ändern sich Pflichtdateien, Sätze und Fälle.

**Was gemessen ist und was nicht:** Auf Ubuntu 24.04.5 mit bwrap 0.9.0 ist der erste Entwurf des AppArmor-Profils und des Probelaufs gemessen. Auf Ubuntu 26.04.1 (Kernel 7.0, bwrap und Python aus dem Archiv) ist seit dem 20.11.2026 in Wegwerf-VMs aus dem Cloud-Abbild gemessen: das Profil in der Fassung V1 mit dem festen Starter und seinen drei Leselinien (Abschnitt „Das AppArmor-Profil: nur für den festen Einschluss, nie für das System-bwrap“), der Probelauf des Einschlusses und die festen Codes 11 bis 18 des `postinst` (Abschnitt „Skripte des Pakets“), die Installation mit `apt`, der Start von Kern und Brücke und der Zustand nach `purge` (Abschnitte „Dienste“, „Skripte des Pakets“, „Entfernen“), der Bau des vollständigen deb auf dem Bauhost (Abschnitt „Der Bauhost“), der Lauf von `werkbank-lauf` mit `pi` im Einschluss bis zum Start des Harness (Abschnitt „lauf.json“), die Sitzungsordner und was `werkbank-pruefen` auf einer laufenden Installation meldet (Abschnitt „Skripte des Pakets“ und der Absatz vor der Prüfung `copilot`). Ebenfalls gemessen (21.11.2026): der Programmordner einer npm-Installation unter `<heim>/.local`, die Prüfung beim Eintragen (Code 19) und die Admin-Felder von `kern.json` und `bruecke.json` (Abschnitte „lauf.json“ mit „Messung Programmordner“ und „Welche Felder der Admin setzt“). **Nicht gemessen** sind der Lauf mit der Copilot CLI (nie gestartet), die Cloud-Route und ein Rechner mit Richtlinien einer Organisation, anderem Kernel oder Secure Boot. Die Außentests AL-1 bis AL-10 (Abschnitt „Zusagen und Außentests des deb“) bleiben die Abnahme auf einem frischen Rechner.

### Entscheidungen

1. **Paketname `werkbank`, Architektur `amd64`, Version gleich der Manifest-Version `a.b.c`.** Der Paketname `werkbank` gilt auch im Manifest unter Windows (`paket`).
2. **Plattformname `ubuntu-26.04-x64`** im Manifest, Pfade relativ zu `/`. Der alte Name `linux-x64` gilt nicht mehr (Fall `manifest-wert-plattform-alt`).
3. **Alles, was je Nutzer veränderlich ist, liegt unter `/var/lib/werkbank/<nutzer>/`, nie im Heimordner** (abgestimmt). Der Ordner gehört dem Nutzer, Modus 0700. Das deb legt ihn bei der Installation für den Nutzer an, der `sudo` aufruft. Der Einschluss bindet `/var/lib/werkbank` nie ein. **Ausnahme (Nachtrag Halter):** die Ablage des Halters `<heim>/.local/state/agent-workbench/halter/geheim/` liegt im Heim, weil der Halter-Vertrag sie dort festlegt (2.3) und sie der Person gehört. Das Paket legt dort nichts an; `werkbank-anmelden` und der Halter legen sie an, `purge` löscht sie (Entscheidung 5).
4. **Alle Dienste laufen unter der Kennung des Nutzers, als Benutzereinheiten.** Grund: Die Anmeldung der Copilot CLI liegt im Heimordner des Nutzers; Kern-Dienst und Sitzungen müssen als er laufen. Der Modellweg liest die Schlüsseldatei des Nutzers (0600). Die Trennung zum Agenten trägt der Einschluss (eigener Einhängebaum, eigener PID-Raum); die Kennung ist gleich (Vertrag 1, Abschnitt E3). Das deb legt kein Konto und keine Gruppe an; ein Systemkonto wäre ein Rückstand beim Entfernen und brächte hier keinen Gewinn. (Hergeleitet; die Plattformschicht bestätigt, Annahme 1.) **Nachtrag Halter:** Auch der Halter ist eine Benutzereinheit unter der Kennung der Person (`wb-halter.service`); er hält die Anmeldung der Cloud-Route, der Modellweg liest sie nicht mehr. Ein eigener Systemnutzer für den Halter ist Schritt II (Halter-Vertrag OPEN 9 und 18) und hier nicht eingebaut. **Schritt II:** Für den Systemnutzer `wb-halter` und drei Rollengruppen ist diese Entscheidung aufgehoben (Entscheid des Betreibers, 23.11.2026); es gilt Teil A2. Für Schritt I gilt dieser Absatz bis auf den folgenden Nachtrag unverändert. **Nachtrag Rolle aus der Gruppe:** Auch in Schritt I legt
das Paket genau **eine Systemgruppe** an, `wb-bruecke` (`addgroup --system`), ohne Konto und ohne Mitglied; kein Konto wird angelegt.
alt: „Das deb legt kein Konto und keine Gruppe an“. Grund: Die Rolle `bruecke` beim Halter folgt aus der effektiven gid des Modellwegs
(Projektentscheid, 25.11.2026); ohne eigene Gruppe gäbe es keine gid, die nur das setgid-Rollenprogramm trägt. `purge` entfernt sie
wieder (Entscheidung 5). alt: „Der Modellweg liest die Schlüsseldatei des Nutzers (0600)“ galt auch für die Anmeldung der Copilot CLI; es gilt jetzt nur noch für `modellweg.schluessel` der lokalen Route.
5. **Entfernen: `sudo apt purge werkbank` löscht alles, `sudo apt remove werkbank` lässt den Schlüssel liegen** (Projektentscheid: der Schlüssel ist Nutzerdaten). `remove` lässt die Schlüsseldatei `/var/lib/werkbank/<nutzer>/modellweg.schluessel` samt Nutzerordner stehen und **sagt es**: `prerm` ruft `werkbank-pruefen --vor-entfernen`, und der Befehl schreibt den festen Satz `pruef.schluessel_rest` (ohne Inhalt, Länge und Pfad) auf die Ausgabe und ins Journal. `purge` entfernt Programme, Daten, Einheiten, die Profile und `/var/lib/werkbank` samt Schlüssel. Es bleiben nur die Ergebnisdatei am Ort von `--ergebnis`, die Anmeldung der Copilot CLI (ihr eigener Ordner im Heim, nicht unser Paket) und die Systemprotokolle (`dpkg`, `apt`, Journal). Fälle belegen beides (Abschnitt „Entfernen“). **Nachtrag Halter:** Die Ablage des Halters ist Nutzerdaten wie der Schlüssel. `remove` lässt `anmeldung.copilot` und die Ablage liegen und sagt es (`pruef.copilot_rest`); `purge` löscht jede Datei in `geheim/` (überschreiben, dann entfernen) und danach `geheim/`, `halter/` und `agent-workbench/`, jeweils nur wenn leer, und nie `.local/state` oder `.local`. Das geschieht **als die Person** (`setpriv --reuid=<uid> --regid=<gid> --init-groups`), jede Ebene relativ zur vorigen geöffnet und ohne einem Verweis zu folgen (Abschnitt „Entfernen“). Grund: Die Ablage legen Programme des Pakets an; „`purge` löscht alles“ gilt für sie wie für `/var/lib/werkbank`.
6. **Der Schlüssel und die Adresse des Modell-Gateways trägt der Betreiber von Hand ein.** Der Läufer fragt beim ersten Aufruf danach. Kein `debconf`: `debconf` legt Antworten in seiner Datenbank ab und mischt sie in die Ausgabe von `apt`; ein Schlüssel darf dort nie stehen.
7. **Die Schritte des Pakets nach dem Entpacken brechen die Installation nie ab.** Was fehlt, sagt `werkbank-pruefen`.
8. **Der System-Truststore gilt immer.** Das Paket hängt von `ca-certificates` ab und enthält keine Wurzelzertifikate und keine Bibliothek, die welche mitbringt (Fälle `stueckliste-verboten-*`).
9. **Das AppArmor-Profil gilt nur für den festen Einschluss, nie für das System-`bwrap`** (nach Messung auf Ubuntu 24.04.5). Von den zwei Formen gilt nach der Messung auf 26.04.1 Form A in der Fassung V1 (Abschnitt „Das AppArmor-Profil“).
10. **Signatur für den Probelauf: eine Prüfsumme (sha256), die der Betreiber vergleicht.** Die Signatur für den internen apt-Spiegel und die von `gpg` kommen mit Fassung 2 (Tabelle unten, OPEN 2).

### Der Befehl zum Bauen

```
rust/paket.sh deb [--version <a.b.c>] [--ausgabe <ordner>] [--geruest]
```

Der Bauhost ist ein Ubuntu 26.04 auf x86_64 (gleiche Serie wie das Ziel, damit die gelinkten Bibliotheken passen; OPEN 4). Die Schritte folgen denen der MSI (Teil B), mit diesen Unterschieden:

| Schritt | Handlung | bei Fehler |
|---|---|---|
| 1 | Werkzeug-Ordner, Toolchain und `dpkg-deb` prüfen | Exit 127 |
| 2 | `cargo build --release --locked --target x86_64-unknown-linux-gnu`, ohne die Merkmale `attrappe` und `test*`; danach der **statische Bau** der Programme aus „Statischer Bau“ (zweiter Aufruf mit `RUSTFLAGS="-C target-feature=+crt-static"` und eigenem `--target-dir target/statisch`) | Exit 70 |
| 3 | `./gates.sh licenses`, die SBOM nach `usr/share/werkbank/sbom.cdx.json` | Exit 65 oder der Code des Tors |
| 4 | Baum stufen, die Gegenprobe des Lieferumfangs (Abschnitt „Positivliste und Ausschlussliste“), die Prüfung jeder Profildatei (Abschnitt „Das AppArmor-Profil“), die Prüfung, dass die Programme aus „Statischer Bau“ statisch sind, und die Modi: der Bau-Befehl setzt `umask 022`, `bau.py` setzt den Modus jedes Eintrags der Stufe fest (Ordner 0755, Programme 0755, Daten 0644), danach prüft Stufe 9 der Gegenprobe, dass nichts für Gruppe oder Welt schreibbar ist | Exit 65, ein `paket.*`-Code |
| 5 | `DEBIAN/control` schreiben (`Depends` aus `dpkg-shlibdeps` plus die festen sechs) und prüfen | Exit 65, ein `paket.steuer_*`-Code |
| 6 | `usr/share/werkbank/manifest.json` schreiben und prüfen | Exit 65, ein `paket.*`-Code |
| 7 | `dpkg-deb --build --root-owner-group` | Exit 70 |
| 8 | Prüfsumme schreiben: `werkbank_<version>_amd64.deb.sha256` | Exit 70 |

Ergebnis in `--ausgabe`: `werkbank_<version>_amd64.deb`, `werkbank_<version>_amd64.deb.sha256` (eine Zeile `<sha256>  <dateiname>`) und `werkbank_<version>_amd64.manifest.json` (Kopie des Manifests). Das deb ist in Fassung 1 **nicht signiert**. Für Schritt I gilt die sha256, die die Person über einen zweiten Weg prüft (A71); das ist der Schutz des Probelaufs. Die Signatur des deb kommt vor Schritt II.

**Gerüstbau (`--geruest`).** Er baut ein deb aus den Programmen, die es schon gibt, und nennt die fehlenden im Bericht. Er ist ein Testbau (`bau: test`, Merkmal `geruest`) und nie für einen Betriebsrechner. **Die Version trägt das Gerüst sichtbar:** Die Steuerdatei führt `Version: <A.B.C>~geruest` (Tilde: die Fassung sortiert vor der Auslieferung `<A.B.C>`, `apt` ersetzt ein Gerüst durch die Auslieferung ohne Umweg), der Dateiname folgt der Steuerdatei: `werkbank_<A.B.C>~geruest_amd64.deb`, ebenso `.deb.sha256`, `.manifest.json`, `.stueckliste.tsv` und `.bericht.json`. Das Manifest führt weiter `<A.B.C>` (Stufe 8 kennt keine Tilde), `bau: test` und das Merkmal `geruest`. Ohne `--geruest` trägt die Version nie `~geruest`. Fälle: `steuerdatei-gueltig-geruest`, `steuerdatei-version-geruest-*`.

### Statischer Bau

Zwei Programme des deb sind **statisch gelinkt** (`-C target-feature=+crt-static`, glibc, static-pie): der feste Starter `/usr/lib/werkbank/werkbank-einschluss` und der Weiterreicher `/usr/lib/werkbank/einschluss-weiter` (Quelle `bin:einschluss:einschluss-weiter`, `root`, 0755). Grund: Der Starter läuft unter einem AppArmor-Profil, das genau die Lesezeilen des Profils nennt; ein dynamisch gelinktes Programm lädt den Lader und Bibliotheken, die das Profil nicht kennt (gemessen: Starter und Attrappe statisch auf Ubuntu 26.04.1, OPEN 3; dynamisch nicht gemessen). Der Weiterreicher läuft im Einschluss, wo es keine Bibliotheken des Systems gibt.

**Projektentscheid, 21.11.2026 (Nachtrag 3 des Projekts):** `werkbank-lauf`, sein Trampolin `werkbank-lauf-kind` und `werkbank-ordnermarke` laufen **außerhalb des Einschlusses und ohne Profil**. Sie sind **dynamisch** gelinkt, gegen das OpenSSL der Distribution (Crate `openssl`, Bibliothek `libssl.so.3`); `dpkg-shlibdeps` trägt `libssl3t64` in `Depends` ein (die feste Liste führt es ohnehin). Der Truststore des Systems gilt. Ein Sicherheitsupdate von OpenSSL kommt darum über `apt`; das deb muss dafür nicht neu gebaut werden. Es gibt kein statisches OpenSSL im Paket, keine Zusatzbibliotheken (zstd, zlib, jitterentropy) und keinen Code `paket.openssl_statisch_fehlt`. Gemessen auf dem Bauhost: Ein statischer Bau von `werkbank-lauf` gegen die `libcrypto.a` von Ubuntu bricht das Linken ab (`undefined symbol` für `ZSTD_*` und `jent_*`); das war der Anlass, den Weg zu verwerfen. Ein Grund für statisch beim Trampolin ist nicht gemessen: es läuft nicht unter dem Profil des Starters (hergeleitet: das Profil `usr.lib.werkbank.werkbank-einschluss` hängt nur am Pfad des Starters).

`paket.sh` baut die statischen Programme in einem zweiten `cargo build` (nur `-p <crate> --bin <programm>` je Eintrag, `--target-dir target/statisch`) und stuft sie von dort; der erste Aufruf baut alles unverändert. Ein Eintrag, dessen Programm das Crate nicht als `[[bin]]` führt, wird nicht gebaut. In Schritt 4 liest der Bau von jedem gestuften Programm dieser Liste den ELF-Kopf: hat es einen Programminterpreter (`PT_INTERP`) oder eine Bibliotheksliste (`DT_NEEDED`), gilt `paket.programm_nicht_statisch` (Exit 65, Grund `Programm ist nicht statisch gelinkt`). Fehlt ein Programm im Gerüstbau, wird nur geprüft, was gestuft ist. Fälle `statisch-*` (Referenz `statisch_code`, Form `statisch`); die Fälle `statisch-*-dynamisch-ist-erlaubt` halten fest, dass Läufer, Trampolin und Ordnermarke dynamisch sein dürfen.

| Ziel | Quelle |
|---|---|
| `usr/lib/werkbank/werkbank-einschluss` | `bin:einschluss:werkbank-einschluss` |
| `usr/lib/werkbank/einschluss-weiter` | `bin:einschluss:einschluss-weiter` |

`usr/bin/werkbank-ordnermarke` (Nachtrag 1, Entscheidung zu Vertrag 1 OPEN 13; Quelle `bin:lauf:werkbank-ordnermarke`, `root`, 0755, dynamisch) steht in der Positivliste. Es ist **Pflichtdatei des Manifests** (Entscheidung des Paketeigners 21.11.2026; Quelle im Baum seit dem Merge): ein Manifest ohne die Datei oder mit anderer `art` als `programm` endet mit `paket.pflichtdatei_fehlt`. Fehlt die Quelle trotzdem, endet der Bau ohne `--geruest` mit `paket.quelle_fehlt`.

`usr/bin/werkbank-ca-zulassung` (Shell-Skript, `root`, 0755; Quelle `datei:paket/ubuntu-26.04-x64/werkbank-ca-zulassung`) ist ebenfalls **Pflichtdatei des Manifests** (Projektentscheid, 21.11.2026, Nachtrag 1 Punkt 3): Ohne ihn hat der Admin den zweiten Weg zur Zulassung der CA nicht, und die Meldungen des Pakets verweisen auf ihn. Ein Manifest ohne die Datei oder mit anderer `art` als `programm` endet mit `paket.pflichtdatei_fehlt`; `werkbank-pruefen` prüft sie mit (Pflichtdateien des Manifests: jetzt 31 statt 30 für Ubuntu; seit dem Nachtrag Halter 33 mit `werkbank-halter` und `wb-halter.service`). Fälle `manifest-pflichtdatei-linux-ca-zulassung` und `…-andere-art`.

| Code | Verletzung | Grund (fest) |
|---|---|---|
| `paket.programm_nicht_statisch` | ein Programm der Tabelle hat `PT_INTERP` oder `DT_NEEDED` | `Programm ist nicht statisch gelinkt` |
| `paket.kern_einheit_nonewprivs` | die Einheit des Kern-Dienstes setzt `NoNewPrivileges` (Wahrheitswert) oder eine Option, die es im Nutzer-Manager nach sich zieht | `Kern-Einheit setzt NoNewPrivileges oder eine Option, die es nach sich zieht` |

### Der Bauhost (Nachtrag 21.11.2026)

Das deb wird auf einem **Ubuntu-26.04-Rechner (x86_64) gebaut**, weil die Programme gegen die Bibliotheken des Ziels linken. Den Weg legt `rust/paket/bauhost.sh` fest; ein zweiter Mensch baut dasselbe deb mit denselben Schritten:

```
rust/paket/bauhost.sh [--version <a.b.c>] [--ausgabe <ordner>] [--geruest] [--nur-vorbereiten]
```

Das Skript prüft den Rechner (Ubuntu 26.04, x86_64, kein root), installiert die Bauabhängigkeiten aus dem Ubuntu-Archiv (`build-essential pkg-config libssl-dev dpkg-dev python3 curl ca-certificates`; `libssl-dev` liefert die Kopfdateien für das Crate `openssl`), legt die Rust-Toolchain und die Prüfwerkzeuge im Werkzeugordner unter `$HOME/.local/share/wb-rust-werkzeuge` an (`tools/werkzeuge-installieren.sh`; Version der Toolchain aus `rust/rust-toolchain.toml`, Werkzeuge mit festen Versionen), schreibt `bauhost-stand.txt` in den Ausgabeordner (Rechner, Kernel, Seriennummer des Abbilds, Version jedes Bauteils) und ruft `rust/paket.sh deb`.

**Das Abbild des gemessenen Baus:** das Cloud-Abbild von Ubuntu Server 26.04 (`ubuntu-26.04-server-cloudimg-amd64.img`, Seriennummer 20261108), SHA-256 `7b90593f5532b49b7e3fe3d2743f0d1f0d822e5b07cebcd8e00c0df8069ecab9`, Platte auf 40 GiB vergrößert, 8 CPUs, 10 GiB Speicher, Nutzer `wb` mit sudo ohne Kennwort. Wer einen anderen Ubuntu-26.04-Rechner nimmt, bekommt dieselben Pakete aus demselben Archiv; die Versionen schreibt `bauhost-stand.txt`.

### Prüfsumme und Signatur

**Der Probelauf (M1):** Der Betreiber bekommt die 64 Hexzeichen der Prüfsumme **auf einem zweiten Weg**, nicht im selben Ordner wie das deb (also nicht aus der `.sha256`-Datei allein). Er vergleicht mit einem Befehl, der ohne Netz läuft (`sha256sum` gehört zu `coreutils`, hergeleitet):

```
echo "<die 64 Zeichen vom zweiten Weg>  werkbank_<version>_amd64.deb" | sha256sum -c -
```

Die Ausgabe `werkbank_<version>_amd64.deb: OK` und der Rückgabewert 0 heißen: gleich. `FAILED` und Rückgabewert 1 heißen: nicht installieren. Eine Prüfsumme, die neben der Datei liegt, schützt nur gegen Übertragungsfehler, nicht gegen eine Fälschung; erst der zweite Weg schützt.

**Ab Fassung 2** (Fassung 1 baut das nicht):

| Schicht | Was | Wer prüft, womit | ohne Netz |
|---|---|---|---|
| 1 | abgetrennte Signatur (`gpg --detach-sign`) über `SHA256SUMS` | die IT vor der Installation: `gpgv --keyring <schlüssel> SHA256SUMS.sig SHA256SUMS`, dann `sha256sum -c SHA256SUMS` (beide in jeder Standardinstallation; hergeleitet) | ja |
| 2 | eingebettete Signatur im deb (`debsigs`, Mitglied `_gpgorigin`) | `debsig-verify` mit einer Richtlinie. `dpkg` prüft sie nicht von sich aus (Ubuntu setzt in `dpkg.cfg` die Option `no-debsig`; hergeleitet) | ja, mit Richtlinie und Schlüsselbund |
| 3 | Signatur des apt-Archivs (`InRelease`) | `apt` selbst, gegen den Schlüssel in `Signed-By`; der Betreiber signiert den Spiegel | ja für `file:`-Archive und interne Spiegel |

**Was `apt` und `dpkg` ohne Netz tun** (hergeleitet, AL-2 misst es): `apt install ./werkbank_<version>_amd64.deb` löst die Abhängigkeiten aus den vorhandenen Paketlisten und bereits installierten Paketen auf und lädt nichts, wenn alle da sind. Es prüft **keine** Signatur der Datei; ein manipuliertes deb würde installiert. Darum steht die Prüfsumme **vor** dem Installationsbefehl. `dpkg-sig` wird nicht benutzt (veraltet; hergeleitet).

### Die Anleitung des Probelaufs (vier Zeilen)

1. Prüfsumme vergleichen (Befehl oben).
2. `sudo apt install ./werkbank_<version>_amd64.deb` — **der eine Installationsbefehl**: ohne Netz, ohne Frage. Vorher muss `dpkg -s python3 bubblewrap apparmor ca-certificates systemd libssl3t64` alle sechs als installiert zeigen, sonst braucht `apt` ein Netz (OPEN 16).
3. Vor diesem Schritt: `werkbank-anmelden copilot` meldet die Copilot CLI an (Vertrag 9, `rust/spec/anmelden/README.md`); ohne Anmeldung endet der Läufer mit dem Code 3. `werkbank-probelauf --ergebnis ~/probelauf-ergebnis.json` — beim ersten Aufruf fragt der Läufer nach der Adresse des Modell-Gateways und nach dem Schlüssel der Organisation. **Der Schlüssel erscheint nie auf dem Bildschirm.** Ein zweiter Aufruf fragt nicht erneut; `werkbank-probelauf --neu-einrichten --ergebnis …` fragt wieder.
4. `sudo apt purge werkbank` — entfernt alles Beschriebene; es bleiben die Ergebnisdatei, die Anmeldung der Copilot CLI und die Systemprotokolle.

**AppArmor-Labels des Pakets:** Das Paket legt bei aktiver Sperre der Namensräume genau drei Labels an: `werkbank-einschluss` (am Pfad des Starters), `werkbank-einschluss-bwrap` und `werkbank-einschluss-kind`; keines hängt am System-`bwrap`. `sudo apt purge werkbank` entfernt alle drei restlos (Fälle `entfernen-profil-*`, `entfernen-gueltig-alle-drei-labels-weg`); Ubuntus eigenes Profil `bwrap-userns-restrict` (Labels `bwrap`, `unpriv_bwrap`) gehört nicht dem Paket und bleibt, wie es war.

**Start der Dienste ohne Neuanmeldung (gemessen in VM B, 21.11.2026).** `postinst` startet für den Nutzer nichts: `systemctl --global enable` wirkt für einen schon laufenden Nutzer-Manager erst beim nächsten Start des Managers (Anmeldung). Wer nach der Installation angemeldet bleibt, startet den Kern mit `systemctl --user start werkbank-kern` (ohne `daemon-reload`, ohne `linger`; die Einheit wird gefunden) und die Brücke mit `systemctl --user start werkbank-bruecke` (`BindsTo` zieht den Kern mit; der Befehl für den Kern allein startet die Brücke nicht). Den Halter startet `systemctl --user start wb-halter` (Nachtrag Halter; hergeleitet wie beim Kern, nicht gemessen). Der Läufer `werkbank-lauf` startet den Kern **nicht** selbst (in VM B gemessen: nach dem Start des Laufs war `werkbank-kern` weiter inaktiv und der Lauf erreichte trotzdem Schritt 8; `systemctl` steht im Läufer nirgends). Das Paket liefert **keine** Einheit `werkbank-modellweg.service` (Entscheidung 21.11.): „Lauf starten“ startet den Modellweg je Sitzung (Vertrag 8, Abschnitt 3); eine dauerhafte Einheit würde den Schlüssel ohne Sitzung lesen. `postinst` räumt die Einheit eines früheren Pakets bei einer Aktualisierung weg (Schritt 4a); `werkbank-pruefen` kennt sie nicht mehr.

Optional zwischen 2 und 3: `sudo werkbank-pruefen`. Der Prüfbefehl ändert nichts (Abschnitt „Prüfbefehl“); sein Code 20 gehört ihm, nicht `apt` (A108).

### Bestandteile: Datei, Ort, Eigentümer, Modus, Dienst, Lieferant

Alle Dateien und Ordner des Pakets gehören `root:root`; Ordner und Programme haben Modus 0755, übrige Dateien 0644, keiner ist für Gruppe oder Welt schreibbar, auch wenn der Bauende unter `umask 002` baut (Stufe 9 der Gegenprobe, Abschnitt „Positivliste und Ausschlussliste“). Dienste sind Benutzereinheiten unter `/usr/lib/systemd/user/`, vom Paket global aktiviert (`systemctl --global enable`). „Lieferant“ ist der Baustein, der den Teil baut; was dort Annahme ist, steht in „Abstimmung“.

| Teil | Datei und Ort | Dienst | Lieferant |
|---|---|---|---|
| Kern-Dienst mit der Brücke zum Browser (hört nur auf `127.0.0.1`, Token im URL-Fragment) | `/usr/lib/werkbank/werkbank-kern` | `werkbank-kern.service` | Vertrag 1 und Kern, Brücke |
| „Lauf starten“ mit Einschluss und Modellbrücke im Einschluss (**ohne eigenes Profil**; es ruft den Starter) | `/usr/lib/werkbank/werkbank-lauf` | keine (Kind des Kern-Dienstes) | Vertrag 1, Einschluss |
| Anmeldung der Copilot CLI (Befehl `werkbank-anmelden copilot`, Vertrag 9): legt die Anmeldung als `anmeldung.copilot` (0600) in die Ablage des Halters `<heim>/.local/state/agent-workbench/halter/geheim/` und legt fehlende Ebenen selbst an (0700). alt: „legt die Anmeldung in `/var/lib/werkbank/<nutzer>/copilot.schluessel` (0600) ab; der Zustandsordner wird von `postinst` angelegt“ | `/usr/bin/werkbank-anmelden` (Quelle `bin:anmelden:werkbank-anmelden`, Modus 0755, `root:root`) | keine | Crate `anmelden` |
| Halter (Nachtrag Halter; Halter-Vertrag 2.3): hält `anmeldung.copilot`, beantwortet die Brücke (`werkbank-modellweg`) über den festen Socket `/run/user/<uid>/wb-halter/halter.sock` (nie über `$XDG_RUNTIME_DIR`, Halter-Vertrag 2.3, Nachtrag 25.11.2026); liest `/etc/werkbank/halter.json` | `/usr/lib/werkbank/werkbank-halter` (`dienst`, root, 0755; Quelle `bin:halter:werkbank-halter`) | `wb-halter.service` | Halter |
| Fester Starter des Einschlusses (Entscheidung V1, 20.11.): das einzige Programm, das bwrap unter aktiver Sperre startet; ein übersetztes Programm ohne Kindprogramme außer `/usr/bin/bwrap`, mit festem Argumentsatz, ohne `roh` | `/usr/lib/werkbank/werkbank-einschluss` (`programm`, root, 0755; Quelle `bin:einschluss:werkbank-einschluss`) | keine | Einschluss |
| AppArmor-Profil **nur für den festen Starter** (Form A oder B, nie für `/usr/bin/bwrap`, nie für `werkbank-lauf`, nie `unconfined`) | Form A `/etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss` (Text `profile/v1.in`); Form B `/etc/apparmor.d/usr.lib.werkbank.bwrap` und die Kopie `/usr/lib/werkbank/bwrap` (von `postinst` angelegt) | keine (vom Kernel geladen) | Profil V1 |
| Suche mit dem Tor (`darf`) und der Kette (Protokollzeile, Kettenprüfung) | `/usr/lib/werkbank/rangkern`; `datentor` und `formate` sind einkompiliert | keine (je Sitzung vom Lauf gestartet, außerhalb des Einschlusses) | `rangkern`, Tor, Kette |
| Dünnes MCP-Werkzeug der Suche | `/usr/lib/werkbank/such-mcp` | keine (stdio-Server in der MCP-Konfiguration des Harness) | Vertrag 4 |
| Weiterreicher (Modellweg) | `/usr/lib/werkbank/werkbank-modellweg`; im Archiv `root:root` 0755 wie jedes Programm, nach `postinst` `root:wb-bruecke` mit Modus 2755 über `dpkg-statoverride` (Rollenprogramm der Rolle `bruecke`, Halter-Vertrag 2.3; Nachtrag Rolle aus der Gruppe, alt: 0755 `root:root` auch nach der Installation) | keine Einheit; der Läufer startet ihn je Sitzung | Gateway |
| Läufer des Probelaufs | `/usr/bin/werkbank-probelauf` (Quelle `datei:spec/vorfuehrung/laeufer/werkbank-probelauf`, eine Datei der Standardbibliothek-Python, die die Vorführung liefert); sie ruft `/usr/lib/werkbank/probelauf/drehbuch.py` mit `--konfig /usr/share/werkbank/probelauf/konfig.json` (Quelle von `drehbuch.py`: `datei:spec/vorfuehrung/laeufer/drehbuch.py`) | keine | Vorführung |
| Konfiguration des Läufers | `/usr/share/werkbank/probelauf/konfig.json`, Art `daten`, Quelle `datei:spec/vorfuehrung/laeufer/konfig.probelauf.json` (eine Datei `konfig.json` gibt es im Arbeitsbaum nicht; die Datei wird nicht umbenannt, das Ziel behält den Namen `konfig.json`; `konfig.attrappe.json` kommt nie ins Paket) | keine | Vorführung |
| Bibliothek des Läufers: das Paket `drehbuchlib` (`drehbuch.py` importiert es und läuft ohne es nicht), eine Datei je Modul: `ablauf.py`, `bericht.py`, `bruecke.py`, `fremd.py`, `konfig.py`, `lauf.py`, `messen.py`, `modell.py`, `probelauf.py`, `schritte.py`, `seite.py`, `weiterreicher.py`, `ziel.py` | `/usr/lib/werkbank/probelauf/drehbuchlib/<modul>.py` (je Datei `programm`) | keine | Vorführung |
| Prüfbefehl der Kette (`kette pruefe [--vorher <hex>] [--letzte <hex>] <datei>...`, Exit 0 ganz, 1 Bruch, 64 Aufruffehler; Vertrag 5 Abschnitt 3, `rust/spec/protokollzeile/README.md`) | `/usr/lib/werkbank/kette` (`programm`, root, 0755; Quelle `bin:formate:kette`, das `[[bin]] kette` des Crates `formate`) | keine | Kette |
| Prüfbefehl | `/usr/bin/werkbank-pruefen` | keine | Paket |
| Befehl für die Zulassung der CA (Shell-Skript, nur root; Abschnitt „Zulassung der CA“) | `/usr/bin/werkbank-ca-zulassung` (`programm`, root, 0755; Quelle `datei:paket/ubuntu-26.04-x64/werkbank-ca-zulassung`) | keine | Paket |
| Vorführ-Einstellungen (Vorlage; Quelle ist `rust/spec/einstellungen/vorfuehrung.json`, die Vorführdatei von Vertrag 2) | `/usr/share/werkbank/probelauf/einstellungen.json` | – | Vertrag 2, Inhalt Vorführung |
| Erststart-Profil (Nachtrag Erststart; Quelle `datei:spec/einstellungen/erststart.json`, Art `daten`): Vorlage von `halter.json` (Schritt 2c) und der Arbeitskopie der Einstellungen (Schritt 2) | `/usr/share/werkbank/probelauf/erststart.json` | – | Vertrag 2, Abschnitt „Erststart-Profil“ |
| Vorführ-Vault (Vorlage, 21 Notizen) | `/usr/share/werkbank/probelauf/vorfuehr-vault/<ordner>/<datei>.md`, je Datei Art `daten`, Quelle `datei:spec/such-socket/vorfuehr-vault/<ordner>/<datei>.md`; die Positivliste führt jede der 21 Dateien einzeln (kein Ordner als Eintrag), das Ziel in Kleinschrift (die Gegenprobe vergleicht den Baum in Kleinschrift, zum Beispiel Quelle `MI-4F9-wartungsfenster-presslinie.md`, Ziel `mi-4f9-wartungsfenster-presslinie.md`) | – | Vault |
| Stückliste, Manifest | `/usr/share/werkbank/sbom.cdx.json`, `/usr/share/werkbank/manifest.json` | – | Paket |

**Je Nutzer, angelegt für den Nutzer, der `sudo` aufruft** (Eigentümer der Nutzer, nicht root):

| Pfad | Modus | Inhalt | Wer legt an |
|---|---|---|---|
| `/var/lib/werkbank/<nutzer>/` | 0700 | der Ordner des Nutzers | `postinst` |
| `/var/lib/werkbank/<nutzer>/einstellungen.json` | 0600 | Arbeitskopie des Erststart-Profils (Nachtrag Erststart; alt: der Vorführ-Einstellungen), danach mit der Gateway-Adresse | `postinst` (Kopie), Läufer (Adresse; Annahme 11) |
| `/var/lib/werkbank/<nutzer>/modellweg.schluessel` | 0600 | der Schlüssel der Organisation; **nie im Paket** | Läufer, erster Aufruf |
| `/var/lib/werkbank/<nutzer>/copilot.schluessel` | 0600 | **entfällt (Nachtrag Halter).** Das frühere Ziel der Anmeldung; kein Programm des Pakets schreibt oder liest es mehr. Liegt die Datei nach einem Update noch da, meldet `werkbank-pruefen` sie (`pruef.copilot_alter_ort`); `purge` löscht sie mit dem Ordner. alt: „die Anmeldung der Route `copilot` (Vertrag 8, `rust/spec/modellweg/README.md`, Abschnitt 5): genau eine Zeile `^[\x21-\x7e]{1,4096}$`, mit oder ohne LF; **nie im Paket**; der Läufer liest daraus ‚angemeldet‘ oder `copilot.nicht-angemeldet`. Wie der Schlüssel nach `copilot login` in die Datei kommt, regelt dieser Vertrag nicht (offen beim Projekt)“ | niemand |
| `/var/lib/werkbank/<nutzer>/vorfuehr-vault/` | 0700 (Ordner), 0600 (Dateien) | Kopie der Vorlage, außerhalb von `~` | `postinst` |
| `/var/lib/werkbank/<nutzer>/protokoll/` | 0700 | das Protokoll der Kette | Kern-Dienst und Suche |
| `/var/lib/werkbank/<nutzer>/ordnermarken.json` | 0600 | die Ordnermarken des Nutzers; **nie im Paket** | „Lauf starten“, beim ersten Setzen |
| `/var/lib/werkbank/<nutzer>/sitzungen/` | 0700 | die Ziellisten der Cloud-Sitzungen | `postinst` |
| `/var/lib/werkbank/<nutzer>/dienst/` | 0700 (Eigentümer und Gruppe der Nutzer, kein Symlink) | der Ordner der Sitzungsliste des Kern-Dienstes (`lauf.bestand` in `kern.json`); bei Abweichung endet `postinst` mit dem Code 29. Herkunft: Projektentscheid, 27.11.2026; Vertrag `dienst` DI-L5 | `postinst` |
| `/var/lib/werkbank/<nutzer>/dienst/bestand.json` | 0600 | die Sitzungsliste des Kern-Dienstes; **nie im Paket**; `postinst` legt sie nie an und fasst sie nie an, ein Update lässt sie byte-gleich | Kern-Dienst |
| `/var/lib/werkbank/<nutzer>/gespraeche/` | 0700 (Eigentümer und Gruppe der Nutzer, kein Symlink) | der Ordner der Gespräche der Sitzungen (`gespraeche` in `lauf.json`, `lauf.gespraeche` in `kern.json`; je Sitzung `gespraeche/<sitzung>/`); bei Abweichung endet `postinst` mit dem Code 30. Herkunft: Projektentscheid, 27.11.2026; Vertrag `start`, Fassung 2 | `postinst` |
| `/var/lib/werkbank/<nutzer>/gespraeche/<sitzung>/` | 0700 | das Gespräch einer Sitzung (`gespraech.json` und, ab Fassung 2, `zustand/`); **nie im Paket**; `postinst` legt sie nie an und fasst sie nie an, ein Update lässt sie byte-gleich; `purge` entfernt sie mit dem Zustandsordner | „Lauf starten“, Schritt 8; der Kern-Dienst räumt sie beim Löschen der Sitzung |
| `/var/lib/werkbank/<nutzer>/sitzungen/<sitzung>.zielliste.json` | 0600 | die Zielliste des Modellwegs der Sitzung; bleibt nach der Sitzung liegen, damit der Läufer `werkbank-modellweg suche --zielliste` aufrufen kann; **nie im Paket** | „Lauf starten“, je Cloud-Sitzung |

**Im Heim, angelegt nicht vom Paket (Nachtrag Halter):**

| Pfad | Modus | Inhalt | Wer legt an |
|---|---|---|---|
| `<heim>/.local/state/agent-workbench/halter/geheim/` | 0700; jede Ebene ab `.local` der Person, für Gruppe und andere nicht schreibbar | die Ablage des Halters (Halter-Vertrag 2.3) | `werkbank-anmelden` oder der Halter, wer zuerst läuft |
| `<heim>/.local/state/agent-workbench/halter/geheim/anmeldung.copilot` | 0600 | die Anmeldung der Route `copilot`: das Token der Copilot CLI und ein LF (Form nach Halter-Vertrag 2.3); **nie im Paket**; nur der Halter liest es | `werkbank-anmelden copilot` |

`<heim>` ist das Heim aus `passwd`, nie aus der Umgebung (wie `heim` in `lauf.json`).

Der Zustandsordner `/var/lib/werkbank/<nutzer>/` und alles darunter (auch `ordnermarken.json` und die Ziellisten) gehört dem Nutzer; `purge` löscht ihn samt Inhalt (Fälle `entfernen-rueckstand-zustandsordner-nutzer`, `entfernen-rueckstand-sitzungsordner`, `entfernen-gueltig-ordnermarken-und-zielliste-weg`).

Ruft niemand `sudo` auf (etwa `pkexec` oder `apt` als root ohne `SUDO_USER`), legt `postinst` keinen Nutzerordner an und schreibt eine Zeile ins Journal; `werkbank-pruefen` meldet `pruef.nutzerordner_fehlt` (OPEN 20).

### Der Schlüssel und die Adresse des Gateways

- Der Läufer fragt beim ersten Aufruf (und bei `--neu-einrichten`) nach der **Adresse** und nach dem **Schlüssel**. Der Schlüssel wird ohne Echo gelesen (Terminal im Modus ohne Anzeige; ist die Standardeingabe kein Terminal, liest der Läufer eine Zeile und zeigt nichts).
- Der Schlüssel steht **nie** in der Befehlszeile (argv), nie in der Umgebung, nie in einer Ausgabe, nie in einem Protokoll, nie in der Ergebnisdatei, nie im Journal. Er geht in `modellweg.schluessel` mit Modus genau 0600, Eigentümer der Nutzer.
- Nur der Modellweg liest die Datei, **außerhalb des Einschlusses** (der Einschluss bindet `/var/lib/werkbank` nie ein).
- Die Adresse ist kein Geheimnis, steht aber nie in der Ergebnisdatei des Probelaufs.
- `werkbank-pruefen` prüft Art, Eigentümer, Modus und dass die Datei nicht leer ist. Er gibt nie den Inhalt aus, nicht die Länge und nicht den Pfad (feste Sätze).

Fälle (Form `einrichten`): Die Marke `SCHLUESSEL-PRUEFMARKE-0001` dient als Schlüssel. Der Fall nennt, was beim ersten Aufruf beobachtet wurde (Standardausgabe, Fehlerausgabe, Journal, Befehlszeile, Umgebung, Protokolle, Ergebnisdatei, Echo, Datei). Die Prüfung meldet genau einen Code in dieser Reihenfolge:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `einricht.schluessel_in_ausgabe` | die Marke steht in einer der beobachteten Stellen außer der Schlüsseldatei | `Schluessel erscheint ausserhalb seiner Datei` |
| 2 | `einricht.echo_an` | die Eingabe wurde angezeigt | `Eingabe des Schluessels mit Echo` |
| 3 | `einricht.datei_modus` | der Modus der Datei ist nicht genau 0600 | `Schluesseldatei nicht 0600` |
| 4 | `einricht.datei_eigentuemer` | die Datei gehört nicht dem Nutzer | `Schluesseldatei gehoert nicht dem Nutzer` |
| 5 | `einricht.datei_inhalt` | die Datei enthält nicht den eingegebenen Schlüssel | `Schluesseldatei hat falschen Inhalt` |

Der Fall „falsche Rechte der Datei“ hat zwei Seiten: Beim Einrichten gilt `einricht.datei_modus`; im Betrieb meldet der Prüfbefehl `pruef.schluessel_modus` (Fälle `pruefung-linux-schluessel-*`).

### Das AppArmor-Profil: nur für den festen Einschluss, nie für das System-bwrap

**Sicherheitsregel (Projektentscheid, 20.11.2026, gemessen auf Ubuntu 24.04.5):** Ein Profil mit dem Anhang `/usr/bin/bwrap` gibt **jedem** Aufrufer von `bwrap` einen Benutzer-Namensraum; die Sperre des Systems (`kernel.apparmor_restrict_unprivileged_userns`) wirkt dann für `bwrap` nicht mehr. Der frühere Entwurf dieses Vertrags (Profil `werkbank-einschluss-bwrap` mit Anhang `/usr/bin/bwrap`) ist **verworfen**. Die Freigabe gilt nur für den festen Einschluss. Das System-`bwrap` bleibt für jeden anderen Aufrufer gesperrt.

Es gibt zwei Formen. **Gemessen auf 26.04.1 und entschieden (V1, Betreiber 20.11.2026): Form A in der Fassung V1 gilt.** Form B bleibt als Rückfall ohne Paketänderung und ist nicht im Lieferumfang.

**V1:** Allein der feste Starter `/usr/lib/werkbank/werkbank-einschluss` trägt das Elternprofil; `werkbank-lauf` hat **kein** Profil und ruft den Starter. Der frühere Entwurf der Form A am Pfad von `werkbank-lauf` (`flags=(unconfined)`) **fällt ganz weg**: Er trägt auf 26.04.1 nicht, und kein Profil eines Pakets darf `unconfined` sein (Stufe 4 der Prüfung unten, Zustand `unconfined` des Prüfbefehls).

| Form | Datei im Paket | Inhalt in Kürze |
|---|---|---|
| **A (V1, gilt)** | `/etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss`, dazu das Programm `/usr/lib/werkbank/werkbank-einschluss` | drei Profile in einer Datei: das Elternprofil `werkbank-einschluss` mit dem Anhang `/usr/lib/werkbank/werkbank-einschluss` (`include <abstractions/base>`, sich selbst `mr`, **genau drei Leselinien** `/proc/sys/dev/tty/legacy_tiocsti r`, `/proc/sys/kernel/osrelease r` und `/{**,} r` sowie den Übergang `/usr/bin/bwrap Px -> werkbank-einschluss-bwrap`; **kein** `userns`), das **Kindprofil ohne Pfadanhang** `werkbank-einschluss-bwrap` (trägt `userns` und die Rechte von bwrap und gibt jedem Kind `&werkbank-einschluss-bwrap//&werkbank-einschluss-kind`) und das **gehärtete Kindprofil** `werkbank-einschluss-kind` (ohne Anhang, `audit deny capability`). Ohne den Übergang aus dem Starter ist kein Kindprofil für jemand anderen erreichbar. |
| **B (Rückfall)** | `/etc/apparmor.d/usr.lib.werkbank.bwrap`; dazu die eigene Kopie `/usr/lib/werkbank/bwrap` | ein Profil mit dem Anhang auf **genau** `/usr/lib/werkbank/bwrap`, das `userns` erlaubt. Die Kopie legt `postinst` aus `/usr/bin/bwrap` an (nicht im Paket, damit keine Weitergabe von `bubblewrap` nötig ist und Sicherheitsaktualisierungen durch einen `dpkg`-Trigger auf `/usr/bin/bwrap` ankommen). |

Text der Form A (V1, gemessen; Quelle `profile/v1.in`, derselbe Text steht als Fall `profil-gueltig-form-a` und in `rust/paket/ubuntu-26.04-x64/usr.lib.werkbank.werkbank-einschluss`):

```
abi <abi/4.0>,
include <tunables/global>

profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss {
  include <abstractions/base>
  /usr/lib/werkbank/werkbank-einschluss mr,
  /proc/sys/dev/tty/legacy_tiocsti r,
  /proc/sys/kernel/osrelease r,
  /{**,} r,
  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,
}

profile werkbank-einschluss-bwrap flags=(attach_disconnected,mediate_deleted) {
  allow capability,
  allow file rwlkm /{**,},
  allow network,
  allow unix,
  allow ptrace,
  allow signal,
  allow userns,
  allow mount,
  allow umount,
  allow pivot_root,
  allow pix /** -> &werkbank-einschluss-bwrap//&werkbank-einschluss-kind,
}

profile werkbank-einschluss-kind flags=(attach_disconnected,mediate_deleted) {
  allow file rwlkm /{**,},
  allow network,
  allow unix,
  allow ptrace,
  allow signal,
  allow userns,
  allow mount,
  allow umount,
  allow pivot_root,
  allow pix /** -> &werkbank-einschluss-kind,
  audit deny capability,
}
```

**Die drei Leselinien des Elternprofils (gemessen auf Ubuntu 26.04.1, Audit `DENIED`):** Ohne `/proc/sys/dev/tty/legacy_tiocsti r` und `/proc/sys/kernel/osrelease r` kann der Starter den Schalter TIOCSTI nicht lesen und endet mit Code 6 (die Vorprüfung hält TIOCSTI dann für offen); ohne `/{**,} r` scheitern `canonicalize`, `metadata` und `read_dir` der Prüfung der Quellen, und er endet mit Code 5 (das Audit zeigte `open` auf den Zustandsordner). Mit den drei Zeilen laufen alle Messungen; sonst gehört dem Elternprofil nichts. Die Zeile `/{**,} r` ist breit: Der Starter liest nur Metadaten und listet den Zustandsordner, er schreibt nie und startet nur `bwrap`; das Elternprofil trägt damit Lesen überall (das Kindprofil trägt ohnehin `rwlkm`). Die Prüfung des Pakets (Stufen 1 bis 4) bleibt unverändert; die Leselinien prüft der Test `test_bau.py` und die Messung in der Wegwerf-VM.

Das Skript `postinst` lädt die Profile mit `apparmor_parser -r -W`, **nur wenn die Sperre aktiv ist** (`kernel.apparmor_restrict_unprivileged_userns = 1`). Ist sie nicht aktiv, braucht es kein geladenes Profil; der Zustand ist `ok`, solange der Probelauf des Einschlusses gelingt. Der **Probelauf des Einschlusses** ist `werkbank-einschluss selbsttest` **als Nutzer ohne Vorrechte** (`setpriv --reuid=65534 --regid=65534 --clear-groups`; als root gelingt er auch ohne Profil und wäre wertlos; gemessen auf 26.04.1). Ohne das Profil scheitert er unter aktiver Sperre.

**Prüfung des Pakets (Tor, Schritt 4 des Bauens):** Jede Profildatei des Pakets geht durch diese Prüfung (Form `profil`); sie meldet genau einen Code in dieser Reihenfolge:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.profil_form` | leere Datei, Zeichen außerhalb ASCII, ungleiche Klammern, kein Profil darin, eine Klammer ohne Kopf | `Profil hat falsche Form` |
| 2 | `paket.profil_system_bwrap` | ein Profil hängt (auch über `*`, `**` oder `{…}`) am Pfad `/usr/bin/bwrap`, oder der Name des Profils ist dieser Pfad | `Profil gilt fuer das System-bwrap` |
| 3 | `paket.profil_userns_unbegrenzt` | `userns` oder `allow userns` steht in einem Profil, dessen Anhang weder fehlt (Kindprofil) noch genau `/usr/lib/werkbank/bwrap` ist | `userns ausserhalb des Einschlusses freigegeben` |
| 4 | `paket.profil_unconfined` | ein Profil trägt `unconfined` unter `flags=(…)` (jede Schreibweise der Liste, auch zwischen anderen Flaggen) | `Profil ohne Einschraenkung` |

**Prüfung auf dem Rechner** (`werkbank-pruefen`, Teil „Prüfbefehl“): Der Zustand `system_bwrap_frei` ist rot, wenn `/usr/bin/bwrap` für jeden Nutzer einen Benutzer-Namensraum bekommt, obwohl die Sperre aktiv ist, **und die Freigabe von einem Profil der Werkbank kommt oder ohne jedes Profil gelingt** (Prüfung: ein Nutzer ohne Vorrechte startet `/usr/bin/bwrap --unshare-user …` mit dem Programm `cat /proc/self/attr/current`; der Prüfbefehl liest das Label des Kindes; Label `unconfined`, leer oder mit `werkbank-` = rot). **Projektentscheid, 20.11.2026:** Kommt die Freigabe von Ubuntus eigenem Profil (`bwrap-userns-restrict`, Label `bwrap`, `unpriv_bwrap` oder gestapelt aus beiden), ist das **kein Befund**, sondern ein Hinweis: das Feld `system_bwrap: ubuntu` der Lage (Vorgabe `gesperrt`), die feste Zeile `pruef.hinweis_system_bwrap_ubuntu: Das System-bwrap ist durch das Ubuntu-Profil frei; die Firma kann dieses Profil entfernen.` vor `pruef.rot` oder `pruef.ok` (bei rc 20 vor der Stellenzeile), ohne Wirkung auf rc und ohne Zählung in `pruef.rot`. Ein anderes Label gilt als `nicht_pruefbar`. Der Satz steht auch in der Ergebnisdatei des Probelaufs. Bei Form B meldet `kopie_abweichend`, wenn `/usr/lib/werkbank/bwrap` nicht Byte für Byte dem `/usr/bin/bwrap` des Systems gleicht (der Trigger fehlte). Ist ein anderes Profil an `/usr/bin/bwrap` gehängt, das nicht von uns stammt, bleibt der Zustand `fremd` (das Paket fasst es nicht an). **Neu (Auflage, 20.11.2026):** Der Zustand `unconfined` ist rot, solange in `/sys/kernel/security/apparmor/profiles` ein Label `werkbank-*` oder ein Profil unter `/usr/lib/werkbank/` im Modus `unconfined` geladen ist (die Zeile trägt den Modus in Klammern hinter dem Label). Er geht allen anderen Zuständen des Profils vor, weil er die Zusage bricht, nicht eine Voraussetzung. Gegenprobe: ein Profil mit `unconfined` unter einem Namen `werkbank-*` laden, dann ist der Prüfbefehl rot, nach dem Entladen wieder grün (Fall `pruefung-linux-apparmor-unconfined`; AL-11). `fremd` und `abweichend` betreffen den Pfad des Starters (`/usr/lib/werkbank/werkbank-einschluss`) statt des Pfades von `werkbank-lauf`.

**Entfernen:** `prerm` (bei `remove`) und `postrm` (bei `remove` und `purge`) entladen jedes Profil des Produkts (`apparmor_parser -R`); bei Form B löscht `purge` auch die Kopie. Nach `apt purge` ist **kein Label `werkbank-*` und kein Profil unter `/usr/lib/werkbank/` mehr geladen** (Fälle `entfernen-profil-*`, `entfernen-bwrap-kopie-uebrig`); ein Rest aus einer älteren Installation zählt auch, wenn er schon vorher da war.

**Zustand von Ubuntus Profil `bwrap-userns-restrict` (Projektentscheid, 20.11.2026: unser Profil wird immer geladen, wenn die Sperre aktiv ist).** Bei aktiver Sperre meldet der Prüfbefehl den Zustand dieses Profils **in beiden Fällen** mit einer festen Zeile, damit die Administration ihn sieht. Das Profil gilt als geladen, wenn in `/sys/kernel/security/apparmor/profiles` ein Label `bwrap` oder `unpriv_bwrap` steht. Lagefeld `ubuntu_profil` (`geladen` oder `entladen`; fehlt das Feld, steht keine Zeile da: Sperre aus oder die Liste der Profile nicht lesbar). Die Zeile ist ein Hinweis, kein Befund: Sie steht vor der Zeile `pruef.hinweis_system_bwrap_ubuntu`, falls es sie gibt, und vor `pruef.rot`, `pruef.ok` oder (bei rc 20) der Stellenzeile; sie ändert weder rc noch die Zahl in `pruef.rot`. Wortlaut:
- geladen (S1): `pruef.hinweis_ubuntu_profil_geladen: Das Ubuntu-Profil bwrap-userns-restrict ist geladen.`
- entladen (S0): `pruef.hinweis_ubuntu_profil_entladen: Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen.`

Die Folge für den Einschluss (gemessen auf 26.04.1): In S1 läuft der Einschluss auch allein unter Ubuntus Profil, und das System-`bwrap` ist für jeden Nutzer frei (Hinweis `pruef.hinweis_system_bwrap_ubuntu`). In S0 läuft er nur mit unserem Profil; das System-`bwrap` ist gesperrt. Fälle `pruefung-linux-ubuntu-profil-*`; Außentests AL-12 (S1) und AL-13 (S0).

**Stückliste bei Form B:** Die Kopie ist `bubblewrap` aus dem Ubuntu-Archiv (LGPL-2.0-or-later, hergeleitet) und kein Crate. Sie steht deshalb in der Stückliste mit der Herkunft `ubuntu-archiv`; nur `bubblewrap` darf diese Herkunft tragen (Fälle `stueckliste-gueltig-bwrap-kopie`, `stueckliste-herkunft-ubuntu-fremder-name`). Das Paket selbst enthält die Kopie nicht; die Abhängigkeit `bubblewrap` bleibt Pflicht.

### Dienste

| Einheit | Zweck | Start |
|---|---|---|
| `werkbank-kern.service` | Kern-Dienst (`wb-dienst`, Crate `dienst`, eingebaut als `werkbank-kern`) | aktiviert; der Läufer startet ihn bei Bedarf (`systemctl --user start`) |
| `werkbank-bruecke.service` | Brücke zum Browser (`wb-bruecke`, Crate `bruecke`, eingebaut als `werkbank-bruecke`) | aktiviert; `BindsTo` und `After` der Kern-Einheit |
| `wb-halter.service` (Nachtrag Halter) | Halter (`/usr/lib/werkbank/werkbank-halter`, Halter-Vertrag 2.3) | aktiviert (`WantedBy=default.target`); unabhängig von Kern und Brücke |

**`wb-halter.service`** (Nachtrag Halter; Datei `/usr/lib/systemd/user/wb-halter.service`, `daten`, root, 0644, Quelle `datei:paket/ubuntu-26.04-x64/wb-halter.service`): `ExecStart=/usr/lib/werkbank/werkbank-halter` ohne Argument (Halter-Vertrag, Start Schritt 1), `UMask=0077`, `Restart=on-failure`, `RestartPreventExitStatus=5 64 74 78` (ein Fehler der Abschirmung, des Aufrufs, des Endpunkts oder der Konfiguration oder Ablage startet nicht im Kreis neu), `[Install] WantedBy=default.target`. **Keine** Zeile `Environment`, `EnvironmentFile` oder `PassEnvironment` und keine Option, die ein Geheimnis trägt. Den Socket legt der Halter selbst exklusiv an; es gibt **keine** Einheit `wb-halter.socket` (eine Socket-Aktivierung legte den Pfad vor dem Halter an, und Schritt 4 endete mit Exit 74). `PR_SET_DUMPABLE` und `RLIMIT_CORE` setzt der Halter selbst (Schritt 0); die Einheit braucht dafür keine Option. Hergeleitet, nicht gemessen: Start und Neustart der Einheit im Nutzer-Manager (AL-14).

Die Einheiten laufen unter der Kennung des Nutzers. Der Prüfbefehl liest ihren Zustand für den Nutzer, der `sudo` aufrief (`systemctl --user --machine=<nutzer>@ show …`), und prüft **drei** Einheiten (`pruef.dienst_fehlt` je Einheit): `werkbank-kern.service`, `werkbank-bruecke.service` und seit dem Nachtrag Halter `wb-halter.service` (alt: die dritte war `werkbank-modellweg.service`, die es seit dem 21.11. nicht mehr gibt). Der Halter steht in der Ausgabe nach der Brücke; Fälle `pruefung-linux-dienst-halter-*`. Die Brücke steht in der Ausgabe zuletzt, damit die Zeilen der anderen beiden bleiben, wo sie waren. Neue Fälle `pruefung-linux-dienst-bruecke-*` und `pruefung-linux-dienst-kern-und-bruecke-fehlen`; keine Erwartung eines bestehenden Falls ändert sich, denn eine nicht genannte Einheit gilt als `ok`.

**`werkbank-kern.service`** (Projektentscheid, 21.11.2026, vorgeschlagen): `ExecStart=/usr/lib/werkbank/werkbank-kern /etc/werkbank/kern.json`, `RuntimeDirectory=werkbank`, `RuntimeDirectoryMode=0700` (`/run/user/<uid>/werkbank`). **Kein** `NoNewPrivileges`, **kein** `PrivateTmp` und kein `RestrictAddressFamilies`: Die Sitzungen („Lauf starten“ mit Einschluss und Modellweg) sind Kinder des Dienstes und erben jede Härtung der Einheit. **Warum kein `NoNewPrivileges`:** Der Profilwechsel des festen Starters nach `bwrap` wird unter diesem Flag von AppArmor abgewiesen (Kernel-Meldung `info="no new privs"`, Selbsttest des Starters Code 6, in der Sitzung `lauf.einschluss_fehlt`); der Einschluss setzt `NoNewPrivs` für den Harness selbst (`bwrap`). Das Flag kommt auch indirekt: Im Nutzer-Manager ziehen die auf seccomp gestützten Optionen (`RestrictAddressFamilies`, `SystemCallFilter`, `SystemCallArchitectures`, `RestrictNamespaces`, `RestrictRealtime`, `RestrictSUIDSGID`, `MemoryDenyWriteExecute`, `LockPersonality`) es nach sich; die Liste steht in `referenz.KERN_EINHEIT_NNP_SCHLUESSEL`. Der Bau weist darum eine Kern-Einheit mit dem Flag oder einer dieser Optionen ab: Schritt 4, `paket.kern_einheit_nonewprivs` (Exit 65, Grund `Kern-Einheit setzt NoNewPrivileges oder eine Option, die es nach sich zieht`); Form `kerneinheit`, Fälle `kerneinheit-*`, Referenz `kern_einheit_code` (der letzte Wert einer Option gilt, ein leerer Wert setzt sie zurück, `NoNewPrivileges=no` ist erlaubt). Gemessen am 21.11.2026 in einer Wegwerf-VM (Ubuntu 26.04): Der Harness im Einschluss hat `NoNewPrivs` 1 (`/proc/<pid>/status`), der Kern-Dienst 0; der Selbsttest des Starters gibt unter den Einstellungen der Kern-Einheit rc 0, mit `NoNewPrivileges=yes` rc 6. Die Liste der Folgeoptionen ist aus der Dokumentation von systemd hergeleitet; gemessen ist nur `NoNewPrivileges=yes` selbst. Text für das Issue zum Paket: „Kern-Dienst läuft ohne NoNewPrivileges, weil der Profilwechsel des Starters es verlangt; der Einschluss setzt es für den Harness.“ **`werkbank-bruecke.service`:** `ExecStart=/usr/lib/werkbank/werkbank-bruecke /etc/werkbank/bruecke.json`, `BindsTo=werkbank-kern.service`, `After=werkbank-kern.service`, `NoNewPrivileges=yes`, `RestrictAddressFamilies=AF_INET AF_UNIX`, **kein** eigenes `RuntimeDirectory` (sie nutzt den Ordner des Kerns; so verschwindet er nicht mit der Brücke). Die Brücke bindet nur `127.0.0.1` (im Programm fest).

**kern.json und bruecke.json (Nachtrag 21.11.2026).** `postinst` (Schritt 2b, Block `# --- dienst.json begin/end`) schreibt für den Nutzer aus `SUDO_USER` (die uid aus `id -u`, nie aus der Umgebung) `/etc/werkbank/kern.json` und `/etc/werkbank/bruecke.json`, `root:root` 0644, atomar (`mktemp` im selben Ordner, `chmod`, `chown`, `mv -f`), bei jeder Installation neu; ein Fehler bricht die Installation nicht ab (Journalzeile). `<run>` ist `/run/user/<uid>`. Die Form folgt `crates/dienst/src/startdatei.rs` (DI1, DI12) und dem Vertrag der Brücke (BR1); die Referenz ist `dienstkonfig` (Form `dienstkonfig`, Fälle `dienstkonfig-*`). Hinweis: Der Vorschlag nannte `betrachter` und `anfang` nicht; der Dienst verlangt beide (`startdatei.rs`, Pflichtfelder `socket`, `betrachter`, `anfang`), sie stehen darum in der Datei. **`lauf.sitzungen`:** `kern.json` trägt im Schlüssel `lauf` zusätzlich `sitzungen`, genau mit dem Wert von `sitzungen` in `lauf.json` (`/var/lib/werkbank/s/<uid>`); der Kern-Dienst liest nur seine eigene Startdatei und vergibt damit keine Sitzungskennung, deren Ordner dort schon besteht. Fehlt das Feld oder weicht der Wert von `lauf.json` ab, ist das ein Fehler des Pakets (Prüfung in `pruefen.py` für jeden Fall `dienstkonfig-*` mit Dateien und in `test_bau.py` gegen die tatsächlichen Blöcke von `postinst`).

**Marker-Datei des Probelaufs (`lauf.marker_datei`, `lauf.marker_ordner`).** `kern.json` trägt im Schlüssel `lauf` zwei weitere Felder, beide für dieselbe uid wie `lauf.sitzungen`: `marker_datei` = `/run/user/<uid>/werkbank-probelauf/marker.json` und `marker_ordner` = `/var/lib/werkbank/z/<uid>`. Fehlt eines der Felder, oder liegt `marker_datei` nicht unter `/run/user/<uid>` desselben Nutzers, ist das ein Fehler des Pakets (Prüfung in `pruefen.py` für jeden Fall `dienstkonfig-*` mit Dateien und in `test_bau.py` gegen die tatsächlichen Blöcke von `postinst`).
- **Zweck.** Die Datei ist nur für den Probelauf gedacht. Der Kern bekommt die Marker eines Probelaufs aus keiner anderen Quelle; seine Einheit bleibt unverändert (kein `EnvironmentFile`, kein veränderlicher `ExecStart`). Marker wirken nur auf Zählung und Bericht.
- **Rechte.** Die Datei hat Modus 0600, der Ordner `/run/user/<uid>/werkbank-probelauf/` Modus 0700, beide gehören dem Nutzer. Der Läufer des Probelaufs legt Ordner und Datei an und entfernt beide am Ende; das Paket legt sie nicht an und liefert sie nicht aus.
- **Was der Kern liest.** Aus der Datei liest der Kern nur die Marker, in der Form `{"liste": [...]}`. Ein Schlüssel `ordner` darin ist ein Formfehler (Exit 2 des Kerns). Der Zählordner kommt nie aus dieser Datei, sondern aus `lauf.marker_ordner` (`root`-eigene Datei). Fehlt die Datei, gibt es keine Marker; fehlt `lauf.marker_ordner`, während die Datei gültig vorliegt, endet der Kern mit Exit 2.
- **Zählordner.** `/var/lib/werkbank/z` (`root:root`, 0755) und `/var/lib/werkbank/z/<uid>` (Eigentümer der Nutzer, 0700) legt `postinst` im Schritt 1a an, mit denselben Prüfungen wie `s/<uid>` und mit dem festen Code 17 (Symlink, fremder Eigentümer, falscher Modus oder Nicht-Ordner: Abbruch, nichts geändert). `purge` entfernt den Ordner mit `/var/lib/werkbank`.
- **Einschluss (gemessen am 21.11.2026 in einer Wegwerf-VM, Ubuntu 26.04, Paket mit dem Dienst, der `lauf.marker_datei` liest).** Mit gültiger `marker.json` (`{"liste": ["MP-2C5"]}`, Datei 0600, Ordner 0700) auf dem Wirt startet eine Sitzung über den Kern, und ihre `start.json` trägt `marker` mit der Liste und dem Ordner aus `kern.json` (`/var/lib/werkbank/z/<uid>`); der Läufer nimmt den Zählordner an (Schritt 5a), und die Zähldateien der Sitzung liegen dort. Der Harness im Einschluss sieht `/run/user/<uid>` leer (`readdir` ergibt `[]`); Ordner `werkbank-probelauf`, Datei `marker.json`, `/var/lib/werkbank` und `/var/lib/werkbank/z/<uid>` melden `ENOENT` (Lesen, Anhängen, Anlegen schlagen fehl); die Verbindung zu `/run/user/<uid>/werkbank/dienst.sock` und zu `/run/user/<uid>/bus` meldet `ENOENT`. Eine Datei, die der Harness in `/run/user/<uid>` anlegt, liegt in der privaten `/run` des Einschlusses; auf dem Wirt entsteht sie nicht. Auf dem Wirt sind Ordner, Datei und beide Sockets da, die Datei ist nach der Sitzung unverändert (21 Byte). Danach entfernt der Läufer Ordner und Datei; ohne Datei läuft der Kern ohne Marker und der echte `pi` erreicht das Eingabefeld. Abweichungen gemessen: Datei 0644, Ordner 0755, Schlüssel `ordner` in der Datei, Datei als Verweis und fehlendes `lauf.marker_ordner` bei gültiger Datei enden je mit Exit 2 des Kerns (`lauf.marker_datei ungueltig: datei_modus`, `ordner_modus`, `form`, `datei_verknuepfung`, bzw. `lauf.marker_ordner fehlt, die Marker-Datei braucht ihn`). Nach `apt purge` gibt es `/var/lib/werkbank` und damit `z` nicht mehr.

| Datei | Feld | Wert | Grund |
|---|---|---|---|
| `kern.json` | `socket` | `{"ordner": "<run>/werkbank", "name": "dienst.sock"}` | der Ordner, den `RuntimeDirectory` anlegt (0700); der Pfad hat bei der größten uid 41 Byte (Grenze 100) |
| | `betrachter` | `{"person": "<nutzer>", "raum": "werkbank", "ansicht": "browser"}` | Pflichtfeld des Dienstes (DI1); der Betrachter ist die Brücke des Nutzers |
| | `deflate` | `true` | die Brücke sendet den Wert des Dienstes als `X-Deflate`; beide Dateien tragen denselben |
| | `lauf.programm` | `/usr/lib/werkbank/werkbank-lauf` | Vorgabe |
| | `lauf.einstellungen` | `/var/lib/werkbank/<nutzer>/einstellungen.json` | der Ort, den `postinst` Schritt 2 anlegt (Vertrag 2) |
| | `lauf.env` | `{"TERM": "xterm-256color"}` | Vertrag 1, „Aufruf Lauf starten“: Die Umgebung des Dienstes ist leer bis auf `TERM`, alles Weitere setzt `werkbank-lauf` selbst (es liest nur sein eigenes `TERM`); kein Geheimnis, nie `SSL_CERT_FILE`, `SSL_CERT_DIR` oder `OPENSSL_CONF` (Stufe 0 von `lauf` endet sonst mit 78) |
| | `lauf.groesse` | `[120, 40]` | Vorgabe |
| | `lauf.ziellisten` | `/var/lib/werkbank/<nutzer>/sitzungen` (der Zustandsordner des Nutzers plus `/sitzungen`, nach dem Namen; nicht `lauf.sitzungen`) | der Ordner, in den `werkbank-lauf` die Zielliste einer Sitzung schreibt (`<zustand>/sitzungen`, `rust/crates/lauf/src/ablauf.rs:535-539`); der Kern prüft nur `<lauf.ziellisten>/s-<n>.zielliste.json` mit `lstat`: Jeder Eintrag dort (auch ein Verweis oder ein Ordner) belegt die Nummer, `ENOENT` gibt sie frei; er legt dort nichts an und entfernt nichts. Form: ein Text mit absolutem Pfad, dieselben Regeln wie `lauf.sitzungen` (1 bis 4096 Byte, kein NUL; sonst Exitcode 2 des Kerns). **Optionaler Schlüssel:** Der Kern auf `development` lehnt unbekannte Felder ab; dieser Teil gilt erst mit dem Kern-Schritt. Fall `dienstkonfig-ziellisten-folgen-dem-nutzernamen` |
| | `lauf.laufzeit` | `/var/lib/werkbank/l/<uid>` (genau `laufzeit` aus `lauf.json` desselben Nutzers) | optionaler Schlüssel, Regeln wie `lauf.sitzungen` (absoluter Pfad, 1 bis 4096 Byte, kein NUL). Der Kern gibt eine Sitzungsnummer `n` nur frei, wenn keiner dieser Einträge besteht (`lstat`, nur `ENOENT` gibt frei): `<lauf.sitzungen>/s-<n>`, `<lauf.laufzeit>/s-<n>`, `<lauf.ziellisten>/s-<n>.zielliste.json`, `<lauf.marker_ordner>/s-<n>.suche.json` und `s-<n>.modellweg.json`; er legt dort nichts an und entfernt nichts. Gilt wie `lauf.ziellisten` erst mit dem Kern-Schritt |
| | `lauf.bestand` | `/var/lib/werkbank/<nutzer>/dienst/bestand.json` (der Zustandsordner des Nutzers plus `/dienst/bestand.json`, nach dem Namen wie `lauf.einstellungen`; nicht nach der uid) | optionaler Schlüssel, ein absoluter Pfad (1 bis 4096 Byte, kein NUL; sonst Exitcode 2 des Kerns). Die Datei der Sitzungsliste des Dienstes; ihren Ordner `dienst` legt und prüft `postinst` (Eigentümer der Nutzer, Modus 0700, kein Verweis; bei Abweichung Code 29), die Datei legt nur der Dienst an. Ist der Ordner nicht so, startet der Dienst mit Exit 3. Fall `dienstkonfig-bestand-folgt-dem-nutzernamen`. Herkunft: Projektentscheid, 27.11.2026; Vertrag `dienst` DI-L5 |
| | `lauf.gespraeche` | `/var/lib/werkbank/<nutzer>/gespraeche` (der Zustandsordner des Nutzers plus `/gespraeche`, nach dem Namen wie `lauf.einstellungen` und `lauf.ziellisten`; nicht nach der uid; genau `gespraeche` aus `lauf.json` desselben Nutzers) | optionaler Schlüssel, ein absoluter Pfad (1 bis 4096 Byte, kein NUL; sonst Exitcode 2 des Kerns). Der Ordner der Gespräche; der Kern zählt `<lauf.gespraeche>/s-<n>` als belegt (Vertrag `dienst`, DI12 Punkt 15, `lstat`, nur `ENOENT` gibt frei) und entfernt dort nur beim Löschen der Sitzung. Den Ordner legt und prüft `postinst` (Eigentümer der Nutzer, Modus 0700, kein Verweis; bei Abweichung Code 30), die Einträge darin legt nur „Lauf starten“ an. Gilt wie `lauf.bestand` erst mit dem Dienstteil. Fall `dienstkonfig-gespraeche-folgt-dem-nutzernamen`. Herkunft: Projektentscheid, 27.11.2026; Vertrag `start`, Fassung 2 |
| | `anfang` | `{"personen": {"<nutzer>": {"art": "mensch", "rolle": "nutzer"}}, "sitzungen": []}` | Pflichtfeld (DI1): die Person des Betrachters muss in `personen` stehen; ohne `kind` und `groesse` startet der Dienst mit null Sitzungen |
| `bruecke.json` | `dienst.socket` | `<run>/werkbank/dienst.sock` | derselbe Socket |
| | `port` | `0` | das System wählt; die Brücke schreibt ihn in `port_datei` |
| | `token_datei` | `<run>/werkbank/bruecke.token` | im Ordner des Kerns (0700) |
| | `port_datei` | `<run>/werkbank/bruecke.port` | ebenso |
| | `deflate` | `true` | wie beim Dienst |

`remove` lässt beide Dateien liegen, `purge` löscht sie (und `/etc/werkbank`, wenn leer); Fälle `entfernen-kern-json-bleibt`, `entfernen-bruecke-json-bleibt`. Die Token-Datei ist kein Teil des Pakets (die Brücke legt sie zur Laufzeit an, 0600); ihr Inhalt steht in keinem Fall und in keinem Protokoll.

### halter.json (Nachtrag Halter)

`postinst` schreibt in Schritt 2c `/etc/werkbank/halter.json` für den Nutzer aus Schritt 1 (`SUDO_USER`, uid aus `id -u`, nie aus
der Umgebung), wie `kern.json`: Eigentümer `root:root`, Modus 0644, atomar (temporäre Datei im selben Ordner, `rename`), der
Ordner `/etc/werkbank` gehört `root:root` mit 0755. Die Datei ist kein `conffile` und hat in Schritt I **kein** Feld des Admins;
eine Änderung von Hand geht bei einem Update verloren. Grund für root: Läge die Datei bei der Person, lenkte jeder Prozess der
Person einen Schlüssel an eine andere Adresse (Halter-Vertrag 2.3); der Halter startet sonst nicht (Exit 78, `halter.konfig`).

Inhalt in der Form `$defs/konfig` des Halter-Vertrags (Abschnitt 2.3a; Nachtrag Rolle aus der Gruppe, alt: „die Feldnamen legt der
Halter-Vertrag fest, sobald er ein Schema für `halter.json` hat“), geschrieben als kanonisches JSON (RFC 8785) mit LF am Ende:

- `fassung` `1`;
- `konten`: genau die uid aus Schritt 1 (`id -u`), Wert `{"person": null}`;
- `rollen`: genau `{"bruecke": "wb-bruecke"}`; keine weiteren Rollen in Schritt I (kein Hub, keine Oberfläche, darum kein
  `schluessel.setzen`). alt: „die Rolle `bruecke` am Rollenprogramm `/usr/lib/werkbank/werkbank-modellweg`“;
- `ziele`: je Netzziel mit `"schluessel": true` einer Route der Art `cloud` aus den `netzziele` der Vorlage
  `/usr/share/werkbank/probelauf/erststart.json` (Nachtrag Erststart, das Erststart-Profil; nie aus der Arbeitskopie der Person und nie
  aus der Vorführ-Vorlage `einstellungen.json`; alt: `/usr/share/werkbank/probelauf/einstellungen.json`) ein Ziel. Fehlt das Profil,
  schreibt `postinst` keine Datei (Journalzeile `werkbank: halter.json nicht geschrieben`). Name des Ziels ist der `host`
  des Netzziels, wenn er die Kennungsform hat; sonst fällt das Netzziel weg, und eine Zeile geht ins Journal. Felder: `host` und
  `port` des Netzziels, `tls` `true`, `praefix` `""`, `kopf` `authorization`, `vorsatz` `"Bearer "`, `schluessel`
  `anmeldung.<schluessel der Route>` (für die Route `copilot` also `anmeldung.copilot`, das Token der Geräte-Anmeldung; mit dem
  Admin-Schalter `eingeschraenktes_token: true` der Route `anmeldung.<schluessel der Route>.eingeschraenkt`, das eingeschränkte Token,
  Nachtrag Business), `pfade` je Pfad des Netzziels mit Methode
  `GET` oder `POST` die Regel `<METHODE> <pfad>` (andere Methoden fallen weg; ein Ziel ohne Regel fällt weg). Kopfname und Vorsatz
  sind hergeleitet, nicht gemessen (OPEN 31);
- `erststart_antwortform` (Nachtrag Erststart): nur an den Modell-Hosts `api.individual.githubcopilot.com` und (Nachtrag Business)
  `api.business.githubcopilot.com`, die Regeln `GET /models` und `POST /chat/completions` in dieser Reihenfolge, soweit `pfade` des Ziels
  sie genau enthält; enthält `pfade` keine, fehlt das Feld. Der Schalter öffnet nie einen Pfad (Halter-Vertrag 2.3a). Aus dem
  gelieferten Profil entstehen an jedem Modell-Host genau die zwei Pfade und der Schalter für genau diese zwei; kein anderes Ziel trägt
  das Feld;
- `server` `{}` (insbesondere kein lokaler Server mit Token, Halter-Vertrag 2.1) und `kids` `{}`.

**Nachtrag Business (26.11.2026).** Aus dem gelieferten Profil entstehen vier Ziele: `api.github.com`, `api.githubcopilot.com` und die
Modell-Hosts beider Pläne, `api.individual.githubcopilot.com` und `api.business.githubcopilot.com`. Telemetrie und `exp` beider Pläne
tragen keinen Schlüssel und werden darum kein Ziel; eine Unterdomänen-Form hat keine Kennungsform und fällt mit einer Journalzeile weg.
Der Admin-Schalter `eingeschraenktes_token` der Route (Vertrag 2) bestimmt nur den Namen des Geheimnisses. Ein Wert, der kein
Wahrheitswert ist, lässt jedes Netzziel der Route weg (je eine Zählung in der Journalzeile): kein Ziel, kein Token, nie ein Rückfall auf
das Token der Geräte-Anmeldung. Fehlt das eingeschränkte Token in der Ablage, endet der Start fail closed: `halter.kein_schluessel`,
im Modellweg `mw.halter_kein_schluessel` (Exit 78). Fälle in `test_bau.py`, Klasse `HalterJson`:
`test_both_plans_bind_the_device_token_without_the_switch`, `test_the_admin_switch_names_only_the_restricted_token`,
`test_a_switch_that_is_no_boolean_never_falls_back_to_the_device_token`; `test_the_file_passes_the_contract_of_the_holder` prüft auch die
Datei mit gesetztem Schalter gegen `$defs/konfig`. Gegenprobe gemessen (26.11.2026):
Die Mutationen „Schalter ignoriert“, „Rückfall auf das Token der Geräte-Anmeldung“, „Schlüssel an Telemetrie“ und „ein Modell-Host fehlt“
machen je einen dieser Fälle rot.

Der Name des Ziels muss gleich `halter_ziel` der Zielliste des Modellwegs sein (Halter-Vertrag 13; Vertrag 8, Abschnitt 12). Das
Beispiel in Vertrag 8 nennt andere Namen (`copilot-plan`); wer die Zielliste baut, nimmt dieselbe Regel (OPEN 31). Referenz:
`referenz.halter_konfig(vorlage, uid)`; `test_bau.py` vergleicht den Block `# --- halter.json begin/end` von `postinst` mit ihr und
prüft das Ergebnis gegen `$defs/konfig`. Fall des Nachtrags Erststart (`test_bau.py`, Klasse `HalterJson`,
`test_the_first_start_profile_gives_two_paths_and_the_switch`): aus dem Profil zwei Pfade und der Schalter am Modell-Host;
Gegenprobe gemessen: Mit `vorfuehrung.json` als Vorlage stehen am Modell-Host die sieben Pfade, und die Prüfung des Falls meldet
`pfade am Modell-Host: 7`. `test_step_2c_reads_only_the_first_start_profile`: liegt nur `einstellungen.json` in der Vorlage, entsteht
keine Datei.

Fehlt der Nutzer (kein `SUDO_USER`), schreibt `postinst` keine Datei und eine Zeile ins Journal; der Halter startet dann nicht
(`pruef.dienst_fehlgeschlagen` für `wb-halter.service`). `remove` lässt die Datei liegen, `purge` löscht sie (und `/etc/werkbank`,
wenn leer). Fälle: `entfernen-halter-json-bleibt`; der Inhalt ist über die Referenz und `test_bau.py` geprüft (Nachtrag Rolle aus der
Gruppe; alt: „Fälle zum Inhalt folgen mit dem Schema (OPEN 28)“).

### vorgabe.json (Nachtrag Vorgabe der Firma, 26.11.2026)

`postinst` schreibt in Schritt 2d `/etc/werkbank/vorgabe.json`, die Vorgabe der Firma für die Routentabelle mit ihren Netzzielen (Vertrag 2,
Abschnitt „Vorgabe der Firma für die Netzziele“; A45). Inhalt: eine Kopie des Erststart-Profils
`/usr/share/werkbank/probelauf/erststart.json`, nie der Arbeitskopie der Person. Eigentümer `root:root`, Modus 0644, atomar (temporäre
Datei im selben Ordner, `rename`), im Ordner `/etc/werkbank` (`root:root`, 0755). Die Datei ist kein `conffile`; ein Update schreibt sie
neu. Grund für root: „Lauf starten“ nimmt nur eine root-eigene Vorgabe in einem sicheren Ordner an (Schritt 4b), sonst startet keine
Sitzung, auch keine lokale (`lauf.vorgabe_fehlt`, `lauf.vorgabe_unsicher`). Fehlt das Profil, schreibt `postinst` keine Datei und die Journalzeile
`werkbank: vorgabe.json nicht geschrieben`. `remove` lässt die Datei liegen, `purge` löscht sie (und `/etc/werkbank`, wenn leer). Fälle:
`test_bau.py`, Klasse `VorgabeJson`.

### Rollengruppe und Rollenprogramm (Nachtrag Rolle aus der Gruppe)

`postinst` Schritt 1b (nach 1a, vor 2): **Gruppe.** Fehlt `wb-bruecke` in `/etc/group`, legt `postinst` sie mit `addgroup --system
wb-bruecke` an und schreibt die Marke `/var/lib/werkbank/rollengruppe-angelegt` (`root:root`, 0644, Inhalt `wb-bruecke` und LF). Besteht
sie schon, übernimmt `postinst` sie nur in dieser Form: genau ein Eintrag, gid von 100 bis 999, keine Mitglieder, Passwortfeld `x`, `!`,
`*` oder leer, und keine Hauptgruppe eines Kontos in `/etc/passwd`; sonst ändert es nichts an der Gruppe und am Programm, schreibt die
Journalzeile `werkbank: Rollengruppe wb-bruecke abgelehnt, Code 25` und fährt fort (Entscheidung 7: kein Abbruch; der Halter gibt dann
keine Rolle, `werkbank-pruefen` meldet es). **Programm.** Danach `dpkg-statoverride --update --add root wb-bruecke 2755
/usr/lib/werkbank/werkbank-modellweg`, wenn noch kein Eintrag besteht; ein Eintrag genau dieser Form bleibt, ein anderer wird ersetzt
(`--remove`, dann `--add`). Ein Fehler schreibt `werkbank: Rollenprogramm nicht gesetzt, Code 26` und bricht nicht ab. Kein Konto wird
Mitglied der Gruppe; `postinst` ruft nie `adduser <nutzer> wb-bruecke` und nie `usermod -aG`.

`prerm` ändert nichts an Gruppe und Eintrag. `postrm remove` lässt beide liegen (wie `halter.json`). `postrm purge`: zuerst
`dpkg-statoverride --remove /usr/lib/werkbank/werkbank-modellweg`, nur wenn der Eintrag genau `root wb-bruecke 2755` ist; danach, nur wenn
die Marke da ist, `delgroup --system --only-if-empty wb-bruecke` (bleibt die Gruppe, weil sie ein Mitglied hat, geht die Zeile
`werkbank: Rollengruppe wb-bruecke bleibt, Code 27` ins Journal). Eine Gruppe, die vor der Installation da war (keine Marke), bleibt. Die
Marke liegt unter `/var/lib/werkbank` und geht mit dem Ordner. Fälle `entfernen-rolle-*`. Hergeleitet, nicht gemessen: Verhalten von
`dpkg-statoverride` beim Update und beim Entfernen der Datei durch `dpkg` (AL-16).

**Kein `NoNewPrivileges` vor dem Rollenprogramm.** Unter `PR_SET_NO_NEW_PRIVS` wirkt setgid bei `execve` nicht (execve(2)); dann hätte
der Modellweg die gid der Person und bekäme beim Halter `halter.peer`. Darum darf keine Stelle der Kette Kern-Einheit → `werkbank-lauf` →
`werkbank-modellweg` das Flag setzen (die Kern-Einheit hat es schon heute nicht, Abschnitt „Dienste“; `kernsperre` setzt es nur im
Einschluss). Hergeleitet, Messung AL-17.

Warum `dpkg-statoverride` und nicht das Archiv: Beim Entpacken muss eine Gruppe im Archiv schon bestehen (Teil A2, Annahme A2-1); in Schritt
I gibt es kein `preinst`. Mit dem Eintrag bleibt das Archiv bei `root:root` 0755, und Stufe 9 der Gegenprobe bleibt für Teil A unverändert.


### Der Läufer (Schnittstelle, Vertrag Vorführung)

Den Läufer baut der Baustein, der das Drehbuch führt (`rust/spec/vorfuehrung/laeufer/`, Python 3, nur Standardbibliothek). Dieser Vertrag kopiert dessen Vertrag nicht; es gilt dort der Stand. Das Paket garantiert nur:

- Befehl `werkbank-probelauf --ergebnis <datei>`, ruft `drehbuch.py probelauf --ergebnis <datei> --konfig /usr/share/werkbank/probelauf/konfig.json`.
- Exitcodes (am 20.11.2026): 0 alle Schritte bestanden; 1 mindestens ein Schritt nicht bestanden (Datei vollständig); 2 Aufruf falsch, keine Datei; 3 Voraussetzung fehlt (Copilot nicht angemeldet, Gateway nicht erreichbar, kein Schlüssel; die Datei nennt den Code).
- Die Ergebnisdatei ist ein geschlossenes JSON ohne Inhalte, Pfade, Adressen, Namen und Schlüssel (Schritte S1 bis S5, E1, E2, E3, E4-lokal, E4-cloud, E5 und E6 mit `bestanden`, `fehlercode`, `dauer_ms`, dazu Versionen). Form und sichtbare Fälle stehen im Vertrag Vorführung; die Prüfung dort heißt `drehbuch.py pruefe-ergebnis <datei>`.
- Das Paket hängt von `python3` ab (Ubuntu 26.04 liefert es mit). Die Ausschlussliste des deb führt deshalb kein `*.py`; der Läufer ist die einzige Python-Datei im Paket (Fall `liste-gueltig-linux`; mit der Ausschlussliste von Windows wäre er rot: `liste-linux-python-im-windows-paket`).

**Was `werkbank-pruefen` auf einer laufenden Installation meldet (gemessen in VM B, 21.11.2026, Ubuntu 26.04.1; Kern und Brücke mit `systemctl --user start` gestartet).** Frisch installiert, ohne Schlüsseldatei und ohne Anmeldung der Copilot CLI, gibt der Befehl **rc 1** mit den Befunden `pruef.schluessel_fehlt` und `pruef.copilot_fehlt` (mit `sudo`; ohne root kommt `pruef.apparmor_nicht_pruefbar` hinzu, zusammen mit dem Hinweis `pruef.hinweis_sudo`, der nicht zählt). Das ist gewollt: Beide Dateien legt der Betreiber an (Schlüssel des Modellwegs, Anmeldung der Route `copilot`); ohne sie kann keine Sitzung mit dieser Route laufen, und der Befehl soll das sagen, nie still grün sein. Es gibt keinen Befund zu Diensten, Nutzerordner, Einstellungen, Paketstand oder AppArmor. Mit beiden Dateien (Modus 0600, nicht leer, `copilot.schluessel` in der Form einer Zeile) gibt der Befehl mit `sudo` **rc 0** (`pruef.ok: Alle 8 Pruefungen bestanden.`). Ohne root bleibt auch dann `pruef.apparmor_nicht_pruefbar` und damit rc 1; die Liste der geladenen Profile ist nur mit root lesbar (Abschnitt „Der Nutzer“). Der Modellweg ist keine Einheit des Pakets und nie ein Befund. Fälle: `pruefung-linux-schluessel-fehlt-dienste-ok`, `pruefung-linux-nur-copilot-anmeldung-fehlt`, `pruefung-linux-dienst-*`.

**Prüfung `copilot` (Linux; die achte Prüfung, nach dem Schlüssel des Modellwegs, vor dem Vault).** Sie prüft `<heim>/.local/state/agent-workbench/halter/geheim/anmeldung.copilot` (Heim aus `passwd`; Nachtrag Halter, alt: `/var/lib/werkbank/<nutzer>/copilot.schluessel`) nach den Ebenen der Ablage, Art, Eigentümer, Modus 0600, Leere und Form, in dieser Reihenfolge der Zustände `ablage` (eine Ebene ab `.local` ist ein Verweis, kein Ordner, nicht die der Person, für Gruppe oder andere schreibbar, oder `geheim` nicht 0700; neu), `fehlt` (auch: die Ablage fehlt ganz), `art`, `eigentuemer`, `modus`, `leer`, `form`; ohne Nutzer ist der Zustand `nicht_pruefbar`. Die Ebenen öffnet die Prüfung ohne Verweis und als der Nutzer; sie legt keine an. **Alter Ort (neu):** Liegt `/var/lib/werkbank/<nutzer>/copilot.schluessel` noch da (in jedem Zustand), steht nach der Zeile der Prüfung `copilot` der Befund `pruef.copilot_alter_ort`; die Datei trägt vielleicht ein Token, das niemand mehr liest. Lagefeld `copilot_alt` (`true`; fehlt es, gilt `false`, damit bleiben alle früheren Fälle). Die Form ist die des Halters (Halter-Vertrag 2.3, Fund F11, Projektentscheid, 25.11.2026): ein Wert von 1 bis 12288 druckbaren ASCII-Zeichen ohne Leerzeichen (`\x21` bis `\x7e`) und genau ein LF am Ende; `werkbank-anmelden` schreibt sie so. alt: „genau eine Zeile von 1 bis 4096 druckbaren ASCII-Zeichen ohne Leerzeichen, mit oder ohne LF“. Grund: Eine Datei ohne LF oder mit mehr als 4096 Zeichen meldete der Prüfbefehl anders als der Halter (`pruef.ok` gegen `halter.kein_schluessel`). Die Sätze sind fest, nennen nie Inhalt, Länge oder Pfad. Die Zahl der Prüfungen unter Ubuntu ist damit **8** (`pruef.ok: Alle 8 Pruefungen bestanden.`). Beim `remove` bleibt die Datei (wie `modellweg.schluessel`) und `--vor-entfernen` sagt es mit `pruef.copilot_rest`; `purge` löscht sie (Dateien überschreiben, dann entfernen; als die Person, Entscheidung 5). `--vor-entfernen` sagt zum alten Ort nichts, denn `purge` löscht ihn mit `/var/lib/werkbank`. Fälle: `pruefung-linux-copilot-*`, `pruefung-linux-vor-entfernen-*`.

### Skripte des Pakets

Jeder Schritt ist wiederholbar und bricht die Installation nicht ab. Eine Installation fragt nichts; die einzige Frage des Pakets (`werkbank/ca_zulassung`, Abschnitt „Zulassung der CA“) stellt nur `dpkg-reconfigure` (Priorität `low`). Ein Fehler eines Schritts schreibt eine Zeile ins Journal (`logger -t werkbank-installation`, fester Text, ohne Pfad und Inhalt). Nur ein Fehler beim Entpacken lässt `apt` scheitern.

| Skript | Schritte in dieser Reihenfolge |
|---|---|
| `postinst` (`configure`) | 1. Nutzer bestimmen (`SUDO_USER`). 2. `/var/lib/werkbank/<nutzer>/` mit den Unterordnern (`protokoll/`, `sitzungen/`) anlegen (Modi und Eigentümer wie in der Tabelle), Vorlage des Vaults und der Einstellungen kopieren (Nachtrag Erststart: die Einstellungen aus `erststart.json`, nur wenn die Arbeitskopie fehlt). 2a. `/etc/werkbank/lauf.json` für diesen Nutzer schreiben (Abschnitt „lauf.json“); `ca_zulassung` kommt aus der wartenden Antwort von debconf oder aus der Datei (Abschnitt „Zulassung der CA“); ein Fehler dieses Schritts bricht die Installation nicht ab; ein Programmordner, den der Einschluss ablehnt, lässt nur dieses Harness aus (Journalzeile mit dem Code 19, Abschnitt „lauf.json“, „Prüfung beim Eintragen“). **2c (Nachtrag Halter):** `/etc/werkbank/halter.json` für diesen Nutzer schreiben (Abschnitt „halter.json“); ein Fehler bricht die Installation nicht ab (Journalzeile `werkbank: halter.json nicht geschrieben`, ohne Pfad und Inhalt). Im Heim legt `postinst` nichts an, auch nicht die Ablage. **1b (Nachtrag Rolle aus der Gruppe):** Systemgruppe `wb-bruecke` und `dpkg-statoverride` für `werkbank-modellweg` (Abschnitt „Rollengruppe und Rollenprogramm“; Journalcodes 25 und 26, kein Abbruch). **1a (vor Schritt 2): die Sitzungsordner.** `/var/lib/werkbank/s` und `/var/lib/werkbank/l` gehören `root:root` mit Modus 0755; `/var/lib/werkbank/s/<uid>` und `/var/lib/werkbank/l/<uid>` gehören dem Nutzer mit Modus 0700 (Zustandsordner der Sitzungen und Socket-Ordner; dazu `/var/lib/werkbank/z` und `z/<uid>`, der Zählordner des Probelaufs, siehe `lauf.marker_ordner` unten; siehe `sitzungen` und `laufzeit` in „lauf.json“). Fehlt ein Ordner, legt `postinst` ihn an und prüft ihn danach noch einmal. Gibt es einen der Ordner (auch `/var/lib/werkbank`, `s` und `l` selbst) schon mit fremdem Eigentümer, falschem Modus, als Symlink oder als Nicht-Ordner, endet `postinst` mit dem festen Code 17 (`/var/lib/werkbank`, `s`, `s/<uid>`, `z`, `z/<uid>`) oder 18 (`l`, `l/<uid>`); kein Symlink wird verfolgt, nichts wird geändert (fail closed). **2 (Ende): der Ordner `dienst`.** Nach dem Zustandsordner des Nutzers legt `postinst` `/var/lib/werkbank/<nutzer>/dienst/` an (Eigentümer und Gruppe des Nutzers, Modus genau 0700) und prüft ihn danach noch einmal, wie die Sitzungsordner. Gibt es ihn schon mit fremdem Eigentümer, anderem Modus, als Symlink oder als Nicht-Ordner, endet `postinst` mit dem festen Code 29; kein Symlink wird verfolgt, nichts wird geändert (sonst startet der Dienst mit Exit 3). `postinst` legt `bestand.json` darin nie an und fasst sie nie an; ein Update lässt sie byte-gleich, und `purge` entfernt sie mit dem Zustandsordner. Herkunft: Projektentscheid, 27.11.2026; Vertrag `dienst` DI-L5. **2 (Ende): der Ordner `gespraeche`.** Gleich danach legt `postinst` `/var/lib/werkbank/<nutzer>/gespraeche/` an (Eigentümer und Gruppe des Nutzers, Modus genau 0700) und prüft ihn danach noch einmal, in derselben Weise. Gibt es ihn schon mit fremdem Eigentümer, anderem Modus, als Symlink oder als Nicht-Ordner, endet `postinst` mit dem festen Code 30; kein Symlink wird verfolgt, nichts wird geändert (sonst endet jeder Start in „Lauf starten“ mit `lauf.fortsetzung_kaputt` oder `lauf.intern`). `postinst` legt nichts darin an und fasst nichts darin an; ein Update lässt die Gespräche byte-gleich, und `purge` entfernt sie mit dem Zustandsordner. Herkunft: Projektentscheid, 27.11.2026; Vertrag `start`, Fassung 2. 3. Bei Form B die Kopie `/usr/lib/werkbank/bwrap` aus `/usr/bin/bwrap` anlegen; die Profile des Produkts laden, **nur wenn die Sperre aktiv ist** (nie ein Profil mit Anhang `/usr/bin/bwrap`); danach der Probelauf `werkbank-einschluss selbsttest` als Nutzer ohne Vorrechte. Schlägt eine Voraussetzung dieses Schritts fehl, endet `postinst` mit einem der festen Codes 11 bis 16 (Tabelle unten); das ist der **einzige** Schritt, der die Installation scheitern lässt, weil ein Paket ohne tragenden Einschluss nicht laufen darf. 4. `systemctl --global enable werkbank-kern.service werkbank-bruecke.service wb-halter.service` (alt: ohne `wb-halter.service`). 4a. Die Einheit `werkbank-modellweg.service` eines früheren Pakets: global abschalten, für den Nutzer anhalten und `reset-failed`, die Verknüpfung unter `/etc/systemd/user/default.target.wants/` löschen (keine Reste nach einer Aktualisierung; Fall des Außentests: die alte Einheit ist nach dem Upgrade weg). 4b. Die Einheiten, die `prerm` bei einem `upgrade` angehalten hat (Notiz `/run/werkbank-paket-neustart`), starten wieder: nur der Nutzer von Schritt 1 und nur die drei festen Einheiten (zuerst Kern, dann Brücke, dann Halter; alt: die beiden festen Einheiten Kern und Brücke); die Notiz wird in jedem Fall gelöscht. Nach `dpkg-reconfigure` gibt es keine Notiz, die Einheiten liefen ohnehin weiter. Laufen Kern und Brücke vor dem Schritt, laufen sie danach (Messung im Abschnitt „Zulassung der CA“). 5. `werkbank-pruefen` ins Journal (der Rückgabewert zählt nicht). |
| `config` (debconf, vor `postinst`) | Die Frage `werkbank/ca_zulassung` stellen (Priorität `low`: nur `dpkg-reconfigure` fragt). Vorher wird der Wert der Datei `lauf.json` zur Vorgabe der Frage, wenn keine Antwort wartet. Mit `DEBIAN_FRONTEND=noninteractive` bleibt der bisherige Wert. Bricht nie ab. Dazu die Datei `templates` (Wortlaut der Frage). |
| `prerm` (`remove`, `upgrade`, `deconfigure`) | bei `remove`: zuerst `werkbank-pruefen --vor-entfernen` (der Befehl scheitert nie, rc 0; der Satz `pruef.schluessel_rest` erscheint auf der Ausgabe von `apt` und im Journal, wenn eine Schlüsseldatei da ist, ebenso `pruef.copilot_rest` für `anmeldung.copilot`); dann die Benutzereinheiten des Nutzers anhalten (Kern, Brücke und Halter). **`dpkg-reconfigure` ruft `prerm upgrade <Version>` mit `DEBCONF_RECONFIGURE=1` auf (gemessen) und ersetzt keine Datei: Dann endet `prerm` sofort, ohne eine Einheit anzuhalten.** Bei einem echten `upgrade` (auch bei derselben Version, `apt install --reinstall`) merkt `prerm` die laufenden der drei Einheiten (Kern, Brücke, Halter; alt: der beiden) in `/run/werkbank-paket-neustart` (eine Zeile: Nutzer und Einheiten, `root`, 0600) und hält sie an; `postinst` Schritt 4b startet genau diese wieder. Bei `remove` und `deconfigure` gibt es keine Notiz |
| `postrm` (`remove`) | (Nachtrag Halter: auch `wb-halter.service` global abschalten; `halter.json` und die Ablage bleiben) jedes Profil des Produkts entladen (`apparmor_parser -R` auf die Datei; das entlädt alle drei Profile der Datei) und seine übersetzte Zwischendatei `/var/cache/apparmor/<kennung>/usr.lib.werkbank.*` löschen (gemessen auf 26.04.1: `apparmor_parser -R` lässt sie liegen; Fall `entfernen-rueckstand-apparmor-cache`), Einheiten global abschalten. Der Ordner `/var/lib/werkbank` bleibt (Konfiguration). |
| `postrm` (`purge`) | zusätzlich die Antwort auf die Frage `werkbank/ca_zulassung` aus debconf entfernen (`db_purge`; eine neue Installation beginnt wieder mit `offen`), bei Form B die Kopie `/usr/lib/werkbank/bwrap` löschen und `/var/lib/werkbank` samt Schlüsseldatei **löschen** (Dateien überschreiben, dann entfernen), `/etc/werkbank/lauf.json` löschen und den Ordner `/etc/werkbank`, wenn er leer ist; `remove` lässt `lauf.json` liegen. **Nachtrag Halter:** vor dem Löschen von `lauf.json` Nutzer, uid und Heim aus ihr lesen (`nutzer` aus `zustand`, `heim`; sonst aus `SUDO_USER` und `passwd`); dann als dieser Nutzer (`setpriv --reuid=<uid> --regid=<gid> --init-groups`) die Ablage räumen: das Heim öffnen, jede Ebene `.local`, `.local/state`, `agent-workbench`, `halter`, `geheim` relativ und mit `O_NOFOLLOW`; ist eine Ebene ein Verweis oder nicht die der Person, dort aufhören; in `geheim/` jede reguläre Datei überschreiben und entfernen, jeden anderen Eintrag nur entfernen, wenn er ein Verweis ist (nie verfolgt); ebenso in `halter/protokoll/` (die Kette des Halters, Halter-Vertrag 2.3; Fund F16, alt: `protokoll/` blieb, und mit ihm `halter/` und `agent-workbench/`), dann `protokoll` mit `rmdir`; danach `geheim`, `halter` und `agent-workbench` mit `rmdir`, nur wenn leer. Fehlen Nutzer und Heim, bleibt die Ablage, und eine Zeile geht ins Journal (`werkbank: Ablage des Halters nicht geraeumt`, ohne Pfad). Dazu `/etc/werkbank/halter.json` löschen. **Nachtrag Rolle aus der Gruppe:** vor dem Löschen von `/var/lib/werkbank` den Eintrag von `dpkg-statoverride` entfernen und die Gruppe `wb-bruecke` löschen, wenn die Marke es erlaubt (Abschnitt „Rollengruppe und Rollenprogramm“, Journalcode 27). Ein Fehler bricht `purge` nicht ab |

**Feste Fehlercodes des `postinst` (Schritt 3; `dpkg` meldet `postinst … returned error exit status N`; gemessen auf 26.04.1):**

| Code | Ursache | Meldung beginnt mit |
|---|---|---|
| 11 | `/usr/bin/bwrap` fehlt (Paket `bubblewrap`) | `werkbank-einschluss:` |
| 12 | `apparmor_parser` fehlt (Paket `apparmor`) | `werkbank-einschluss:` |
| 13 | die Profildatei `/etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss` fehlt oder ist nicht lesbar | `werkbank-einschluss:` |
| 14 | der Parser lehnt das Profil ab (seine Meldung wird durchgereicht) | `werkbank-einschluss:` |
| 15 | ein **fremdes Profil** hängt schon am **Pfad des Starters** `/usr/lib/werkbank/werkbank-einschluss`; nichts wird geändert | `werkbank-einschluss:` |
| 16 | der Probelauf scheitert nach dem Laden | `werkbank-einschluss:` |
| 17 | `/var/lib/werkbank`, `/var/lib/werkbank/s`, `s/<uid>`, `/var/lib/werkbank/z` oder `z/<uid>` hat fremden Eigentümer oder falschen Modus oder ist ein Symlink oder kein Ordner (Schritt 1a); nichts wird geändert | `werkbank-einschluss:` |
| 18 | `/var/lib/werkbank/l` oder `l/<uid>` aus demselben Grund | `werkbank-einschluss:` |
| 19 | **kein Abbruch** (Projektentscheid): ein Programmordner eines Harness (Schritt 2a) berührt den Anmeldespeicher oder enthält ein Geheimnissegment nach der Regel des Einschlusses; `postinst` endet mit **Exit 0**, trägt dieses Harness nicht ein und schreibt die Journalzeile `werkbank: Programmordner abgelehnt, Code 19, Harness <name>` (ohne Pfad, ohne Inhalt) und die Meldung `werkbank: ein Programmordner des Harness <name> …; das Harness ist nicht eingetragen (Code 19)` | `werkbank:` |
| 29 | `/var/lib/werkbank/<nutzer>/dienst` hat fremden Eigentümer oder falschen Modus oder ist ein Symlink oder kein Ordner (Schritt 2, Ende); nichts wird geändert. Herkunft: Projektentscheid, 27.11.2026; Vertrag `dienst` DI-L5 | `werkbank-einschluss:` |
| 30 | `/var/lib/werkbank/<nutzer>/gespraeche` hat fremden Eigentümer oder falschen Modus oder ist ein Symlink oder kein Ordner (Schritt 2, Ende); nichts wird geändert. Herkunft: Projektentscheid, 27.11.2026; Vertrag `start`, Fassung 2 | `werkbank-einschluss:` |

Ohne aktive Sperre gelten 11 und 16 (der Probelauf muss auch ohne Profil gelingen), 12 bis 15 entfallen. Code 0 bei sauberem Lauf und beim erneuten Lauf (`upgrade`). Fehlt der Starter (nur im Gerüstbau), lädt `postinst` das Profil und überspringt den Probelauf; der Prüfbefehl meldet die fehlende Datei.

### lauf.json (Nachtrag 21.11.2026: `werkbank-lauf` im Paket lauffähig)

`werkbank-lauf` liest im ausgelieferten Bau `/etc/werkbank/lauf.json` (Crate `lauf`, `src/konfig.rs`, geschlossene Form, Fassung 1) und prüft Eigentümer und Modus. `postinst` schreibt die Datei in Schritt 2a für den Nutzer aus Schritt 1 (`SUDO_USER`; bei mehreren Nutzern gilt der zuletzt installierende, Fassung 1 kennt einen Nutzer je Rechner). Eigentümer `root:root`, Modus 0644, atomar (temporäre Datei im selben Ordner, dann `rename`). Jeder Wert stammt aus dem Skript, aus `passwd` (`getent`, nie aus der Umgebung des Aufrufers) oder aus der Suche in festen Ordnern; ist ein Wert keine saubere Ordnerform (`referenz.lauf_ordner_form`: absolut, ohne `//`, ohne `.`- und `..`-Segment, nur `A-Za-z0-9._/+@-`), schreibt `postinst` keine Datei und die Journalzeile `werkbank: lauf.json nicht geschrieben`. Ein erneutes Schreiben nach einer späteren Installation eines Harness: `sudo dpkg-reconfigure werkbank`.

| Feld | Wert | Grund |
|---|---|---|
| `einstellungen`, `vault`, `protokoll`, `zustand` | `/var/lib/werkbank/<nutzer>/einstellungen.json`, `…/vorfuehr-vault`, `…/protokoll`, `/var/lib/werkbank/<nutzer>` | Zustandsordner der Installation (Vertrag 1) |
| `sitzungen` | `/var/lib/werkbank/s/<uid>`, 0700, Eigentümer der Nutzer, von `postinst` angelegt (`/var/lib/werkbank/s` 0755 `root`) | Hier liegt je Sitzung der Zustandsordner des Einschlusses (`<sitzung>/zustand`). Er darf **nicht** unter `zustand` (`/var/lib/werkbank/<nutzer>`) oder `laufzeit` liegen: `lauf` schützt beide samt dem Ordner der Schlüsseldateien, und der Starter lehnt einen eingehängten Ordner in einem geschützten Ordner ab (Exit 5, `lauf.einschluss_fehlt`). Gemessen in einer frischen VM: mit `sitzungen` unter `zustand` endete jeder Lauf mit diesem Code (Beschreibung mitgeschnitten). Das Beispiel `/var/lib/werkbank/<nutzer>/sitzungen` des Einschluss-Vertrags gilt für die Ziellisten (`zustand/sitzungen/<sitzung>.zielliste.json`), nicht für den Zustandsordner |
| `gespraeche` | `/var/lib/werkbank/<nutzer>/gespraeche` (der Zustandsordner plus `/gespraeche`, nach dem Namen wie `einstellungen` und `protokoll`; nicht nach der uid), 0700, Eigentümer der Nutzer, von `postinst` angelegt und geprüft (Code 30) | Pflichtschlüssel von „Lauf starten“ (Fassung 2; ohne ihn endet jeder Start mit `lauf.konfig_kaputt`). Hier liegt je Sitzung `<sitzung>/` mit `gespraech.json` und dem Zustandsordner des Harness. Der Ort liegt im Zustandsordner der Installation, damit `purge` ihn mit löscht. Anders als `sitzungen` darf er unter `zustand` liegen: Der Einschluss bindet nur den eigenen Baum einer Sitzung ein und lässt einen Zustandsordner unter einem geschützten Ordner zu, nie einen geschützten Ordner selbst (Vertrag `start`, Abschnitt „Ablage des Gesprächs“). `lauf` läuft als der Nutzer und dürfte den Ordner im Elternordner (`/var/lib/werkbank/<nutzer>`, Eigentümer der Nutzer, 0700) selbst anlegen; `postinst` legt ihn trotzdem an und prüft ihn, damit eine fremde Form bei der Installation auffällt und nicht bei jedem Start. `kern.json` trägt denselben Wert als `lauf.gespraeche`. Herkunft: Projektentscheid, 27.11.2026; Vertrag `start`, Fassung 2 |
| `heim` | Heimordner aus `getent passwd <nutzer>` | nie aus der Umgebung |
| `programme` | `rangkern`, `werkbank-modellweg`, `werkbank-lauf-kind`, `such-mcp`, `einschluss-weiter` unter `/usr/lib/werkbank/` | Positivliste |
| `schluessel` | `lokal`: `…/modellweg.schluessel`, `copilot`: `…/copilot.schluessel` | die Route nennt ihren Schlüssel in der Einstellungsdatei (`schluessel`: `copilot` für die Cloud-Route, `lokal` für die lokale); die Dateien nennt der Paketvertrag. **Nachtrag Halter:** Der Eintrag `copilot` bleibt in diesem Zug unverändert, weil `werkbank-lauf` (Vertrag 1) und der Modellweg (Vertrag 8) ihn heute lesen; an diesem Ort liegt nach dem Nachtrag kein Token mehr. Er entfällt mit dem Zug, der den Modellweg auf den Halterweg stellt (OPEN 28); bis dahin findet die Cloud-Route kein Token und scheitert sichtbar |
| `laufzeit` | `/var/lib/werkbank/l/<uid>`, 0700, Eigentümer der Nutzer, von `postinst` angelegt | Der längste Socketpfad ist `laufzeit` + `/` + Sitzungskennung (höchstens 64 Byte) + `/modell.sock` (11 Byte). Mit der größten uid (10 Stellen) sind das 30 + 1 + 64 + 12 = 107 Byte, die Grenze von `lauf` (`SOCKET_MAX` 107; `sun_path` 108 mit dem Nullbyte). Die Länge hängt nur an der uid, nie am Nutzernamen (32 Zeichen wären zu lang); `/run` fiele nach dem Neustart weg |
| `ca_zulassung` | `offen` (Vorgabe, fail closed). Er ändert sich nur durch einen ausdrücklichen Schritt eines Admins mit root: die wartende Antwort der Frage von `dpkg-reconfigure` oder der Befehl `werkbank-ca-zulassung` (Abschnitt „Zulassung der CA“). Ohne diesen Schritt bleibt der Wert der Datei, auch einer, den ein Admin von Hand eingetragen hat; ein anderer früherer Wert als `zugelassen`, `offen` oder `abgelehnt` wird nie übernommen | Die Antwort der Organisation auf die lokale CA (Vertrag 8, Abschnitt 12) liegt nicht vor; `offen` ist fail closed: die Cloud-Route startet nicht (der Modellweg endet mit `mw.ca_nicht_zugelassen`), die lokale Route ist nicht betroffen. Der Läufer liest die Datei bei jedem Start einer Sitzung (`rust/crates/lauf/src/ablauf.rs:145`): Ein neuer Wert gilt für die nächste Sitzung, ohne Neustart des Dienstes |
| `harness`, `programmordner`, `path` | je gefundenem Harness (`copilot`, `pi`, `opencode`) ein Eintrag, in dieser Reihenfolge; sonst fehlt er und `lauf` endet mit seinem festen Code | siehe unten |

**opencode nur in der festen Fassung (Entscheid des Betreibers, 27.11.2026).** `lk_suche opencode` sucht `opencode` an denselben Orten wie die anderen Harness. Eingetragen wird der echte Pfad nur, wenn seine sha256 gleich `LK_OPENCODE_SHA256` ist: `7df4e4136301f16589b0f30bf91a61380fca5f8ef22dd36570f5ebe1d4ec802c`, das Programm `package/bin/opencode` von npm `opencode-linux-x64@1.19.5` (das `postinstall` von `opencode-ai@1.19.5` verlinkt es als `bin/opencode.exe`; gemessen in einer Wegwerf-VM, harter Link). Jede andere Datei, auch die Fassung `-baseline` für Rechner ohne AVX2, wird nicht eingetragen; dazu die feste Journalzeile `werkbank: opencode nicht in der festen Fassung, nicht eingetragen` ohne Pfad. opencode ist ein eigenes Programm ohne `node`; sein Programmordner ist die Paketwurzel `<präfix>/lib/node_modules/opencode-ai`. Fälle `laufkonfig-opencode-npm` und `laufkonfig-drei-harness`, Test `test_opencode_is_entered_only_in_the_fixed_version` in `rust/paket/test_bau.py`.

**Suche der Harness.** In dieser Reihenfolge, der erste Treffer zählt: `<heim>/.nvm/versions/node/v<a>.<b>.<c>/bin` (**die höchste Version zuerst**, Zahlenvergleich von Haupt-, Neben- und Patchstand: `v22.10.0` vor `v22.1.0` vor `v9.0.0`; Ordner, die kein Versionsname dieser Form sind, zählen nicht; hat die höchste Version das Programm nicht, gilt die nächste), `<heim>/.local/bin`, `<heim>/.npm-global/bin`, `<heim>/.volta/bin`, `/usr/local/bin`, `/usr/bin`. Es läuft kein Programm des Nutzers und keine Shell. Eingetragen wird der echte Pfad (`readlink -f`), und nur, wenn er unter `/usr/`, `/opt/` oder unter `<heim>/.nvm`, `.local`, `.npm-global`, `.volta` liegt, nie unter einem Arbeitsordner (Fall: ein Verweis nach `<heim>/projekt/pi` wird nicht genommen). `node` kommt aus demselben Ordner, sonst aus `/usr/local/bin` oder `/usr/bin`.
**Programmordner (Nachtrag 21.11.2026).** Je Harness die **Paketwurzel**, nie der npm-Präfix: der Ordner `<präfix>/lib/node_modules/<paket>` (bei `@scope/paket` zwei Ebenen unter `lib/node_modules`) oder, wenn das Programm keine Datei eines solchen Pakets ist, der Ordner des Programms; dazu der Ordner von `node` (der Ordner, in dem die Datei liegt, also dessen `bin`, nicht zwei Ebenen darüber). Grund: Ein Präfix wie `<heim>/.local` (so installiert `npm --prefix ~/.local`) enthält den Anmeldespeicher `.local/share/keyrings`; der Einschluss lehnt ihn ab, und jede Sitzung endet mit `lauf.einschluss_teil: 5 verboten`, auch die mit der Copilot CLI (gemessen 21.11.; in der Wegwerf-VM unten wiederholt). Die Paketwurzel ist der engste Ordner, mit dem eine Sitzung bis zum Eingabefeld läuft (Messung im Abschnitt „Messung Programmordner“); `<präfix>/bin` hält nur den Verweis, eingetragen wird der echte Pfad (`readlink -f`) im Paketordner. Nichts unter `/usr` (der Einschluss bindet `/usr` ganz); Doppel und Ordner in einem anderen Programmordner entfallen (der Einschluss lehnt sich überlappende Ordner ab). `path` ist `/usr/local/bin:/usr/bin:/bin`, mit dem `bin`-Ordner von `node` davor, wenn er nicht unter `/usr` liegt.
**Prüfung beim Eintragen (Code 19).** `postinst` und `dpkg-reconfigure` prüfen **jeden** Programmordner vor dem Schreiben mit `werkbank-pruefen --programmordner <heim> <ordner>` (Exit 0 erlaubt und keine Ausgabe; Exit 1 verboten, eine feste Zeile `pruef.programmordner_verboten: …`; jeder andere Ausgang zählt als verboten). Der Befehl ruft die zwei Funktionen des Einschlusses (`hat_geheimnisteil` und `beruehrt_anmeldespeicher` in `crates/einschluss/src/verbote.rs`, dieselben wie `kopf_pruefen` des Profils) mit einem Harnessnamen, der keine eigene Anmeldedatei hat, also strenger als jede Sitzung ist. **Es gibt keine zweite Fassung der Regel**, weder in Shell noch hier im Text; der Entwurf mit einem Prüfmodus des vorhandenen Crates `werkbank-pruefen` (er hängt nur von `einschluss` ab) wurde gewählt, weil der Befehl ohnehin im Paket liegt, vor `postinst` entpackt ist und ohnehin dieselbe Lage prüft; ein Modus im Starter `werkbank-einschluss` ist ausgeschlossen (der Starter nimmt keine freien Argumente). Ein verbotener Ordner **beendet `postinst` nicht** (Projektentscheid ersetzt den Abbruch mit Code 19 aus dem Stand vom 21.11.2026): Das Harness mit dem verbotenen Ordner wird nicht eingetragen, weder `harness.<name>` noch seine Ordner stehen in `lauf.json` (so startet keine Sitzung dieses Harness, fail closed vor dem Einschluss); die Prüfung geht je Harness der Reihe nach über Paketwurzel und Ordner von `node` (nichts unter `/usr`), der erste verbotene Ordner zählt. `postinst` schreibt die feste Journalzeile `werkbank: Programmordner abgelehnt, Code 19, Harness <name>` und endet mit Exit 0; die Dienste, das andere Harness und Schritt 4b (Neustart nach einem Update) laufen weiter. Die Codes 11 bis 18 bleiben hart. **Zustandsdatei `/var/lib/werkbank/programmordner-abgelehnt`** (`root:root` 0644, atomar geschrieben, unter dem Zustandsordner, den `purge` mit löscht): je abgelehntes Harness eine Zeile `<harness> <ordner>` (Harness `copilot` oder `pi`, Ordner absolut, ASCII, ohne Leerzeichen); sie wird bei jeder Konfiguration neu geschrieben und **entfernt, wenn die Konfiguration keinen verbotenen Ordner findet** (fehlt der Befehl im Gerüstbau, bleibt sie, wie sie ist). Sie ist der einzige Weg, auf dem `werkbank-pruefen` den abgelehnten Ordner erfährt: es gibt keine zweite Suche mit eigener Logik. Ein Gerüstbau ohne den Befehl überspringt die Prüfung mit einer Journalzeile; ein ausgelieferter Bau hat ihn immer (Pflichtdatei des Manifests).
**Prüfbefehl.** `werkbank-pruefen` liest `lauf.json` (`heim`, `programmordner`, `harness`) und prüft jeden eingetragenen Programmordner gegen dieselbe Regel: ein verbotener Ordner ist ein **Befund** mit dem Code `pruef.programmordner_verboten` (rc 1, zählt in `pruef.rot`), auch in einer Datei aus einer früheren Installation. Dazu liest der Befehl die Zustandsdatei `/var/lib/werkbank/programmordner-abgelehnt` (höchstens 4096 Byte, nur eine gewöhnliche Datei, kein Verweis; eine Zeile anderer Form zählt nicht, jedes Harness einmal, die erste Zeile gilt) und zeigt je abgelehntes Harness, in der Reihenfolge `copilot`, `pi`, **eine Zeile** mit dem festen Code und dem festen Aufbau `pruef.programmordner_verboten: Harness <name> nicht eingetragen. Abhilfe: Harness unter einem anderen Praefix installieren, dann dpkg-reconfigure werkbank. Ordner: <ordner>` (rc 1, zählt in `pruef.rot`; der Ordner steht zuletzt, damit eine Kürzung bei 200 Byte nur sein Ende nimmt und die Abhilfe ganz bleibt; die Zeile von `lauf.json` oben beginnt mit `pruef.programmordner_verboten: Ein Programmordner`, die Zeile des Harness mit `pruef.programmordner_verboten: Harness `). Ein abgelehntes Harness bekommt nicht zusätzlich den Hinweis „nicht eingetragen“; die Lage der Fälle trägt die Datei als Feld `programmordner_abgelehnt` (Liste von `{harness, ordner}`, optional). Ein Harness, das `lauf.json` nicht nennt, ist **kein Befund**, sondern ein Hinweis mit Grund: `pruef.hinweis_harness_nicht_gefunden` (nennt die abgesuchten Orte; ein anderer npm-Präfix wird nicht gesucht), `pruef.hinweis_harness_ausserhalb` (er liegt an einem gesuchten Ort, der echte Pfad aber nicht unter den erlaubten Wurzeln) oder `pruef.hinweis_harness_nicht_eingetragen` (er liegt an einem gesuchten Ort und fehlt in der Datei; `sudo dpkg-reconfigure werkbank`). Der Befehl sieht dafür nur nach, ob eine ausführbare Datei an den festen Orten liegt und wohin ihr echter Pfad zeigt; es läuft kein Programm und keine Shell des Nutzers.

**Messung Programmordner (21.11.2026, Wegwerf-VM Ubuntu 26.04.1 aus dem Cloud-Abbild, Kernel 7.0, Eingang auf 20 Mbit gedrosselt und belegt, Wirt gesperrt; Node 22.22.1 und npm 9.2.0 aus `apt`, pi 1.5.2 mit `npm install -g --prefix <heim>/.local`; `werkbank-probelauf`-Läufer nicht gestartet, die Sitzung wurde über die Brücke gestartet, ohne Eingabe, ohne Modell, ohne Token).** pi liegt dann als `<heim>/.local/bin/pi` (Verweis) auf `<heim>/.local/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js`.

| Stand | Programmordner in `lauf.json` | Ergebnis |
|---|---|---|
| (a) alt (Stand vom 21.11.2026) | `<heim>/.local` | Sitzung endet sofort mit Code 5, im Bild nur `lauf.einschluss_fehlt: Einschluss nicht verfuegbar` und `lauf.einschluss_teil: 5 verboten`; der Fehler zeigt sich erst bei der Sitzung, die Einrichtung und `werkbank-pruefen` melden nichts zum Ordner |
| (b) neu | `<heim>/.local/lib/node_modules/@earendil-works/pi-coding-agent` | Sitzung läuft; das Eingabefeld (Muster `%/.*qwen3:8b` der Konfiguration) nach 0,82 s erkannt; Kopfzeile `v1.5.2`, Fußzeile `0.0%/128k (auto)`, Zustand `laeuft` nach 20 s und nach 60 s |
| engere Kandidaten (von Hand in `lauf.json`, danach zurückgestellt) | `…/pi-coding-agent/dist/bundle` und `…/pi-coding-agent/dist` | Sitzung läuft und erreicht das Eingabefeld (nach 1,42 s), aber pi zeigt `v0.0.0`: die `package.json` der Paketwurzel ist nicht sichtbar (pi liest Name und Version aus ihr). Die Paketwurzel ist darum der engste Ordner, der das Paket ganz lässt; gewählt ist sie, nicht ein Unterordner |
| (c) neu, Gegenprobe | `werkbank-pruefen --programmordner <heim> <ordner>` | Exit 1 und die feste Zeile für `<heim>/.local`, `<heim>`, `<heim>/.local/share/keyrings` und `<heim>/.config/x/node_modules/p`; Exit 0 ohne Ausgabe für `<heim>/.local/bin`, die Paketwurzel von pi und `/opt/pi`; Exit 64 für einen relativen Ordner |
| (c) neu, Einrichtung | `dpkg-reconfigure` mit einem Harness, dessen echter Pfad im Anmeldespeicher liegt (`<heim>/.local/bin/copilot` → `<heim>/.local/share/keyrings/copilot`) | `dpkg-reconfigure` endet mit Exit 19 und der Meldung `werkbank-einschluss: ein Programmordner der Harness beruehrt den Anmeldespeicher oder ein Geheimnis; …; nichts geaendert (Code 19)`, die Journalzeile `werkbank: Einschluss, Code 19`, `lauf.json` byte gleich wie vorher; nach dem Entfernen des Harness gelingt die Einrichtung wieder (Exit 0) |
| (c) neu, `werkbank-pruefen` | `lauf.json` von Hand mit `programmordner: ["<heim>/.local"]` | Exit 1, eigene Zeile `pruef.programmordner_verboten: …` und `pruef.rot` mit diesem Befund |
| Suche | ein Programm an einem gesuchten Ort mit echtem Pfad in `<heim>/projekt`; keines an einem gesuchten Ort | `pruef.hinweis_harness_ausserhalb: copilot liegt an einem gesuchten Ort, …`; `pruef.hinweis_harness_nicht_gefunden: copilot nicht gefunden in ~/.nvm/versions/node/v*/bin, …` |

**Geändert nach dieser Messung (Folgeschritt „weich auslassen“, Projektentscheid):** Die Zeile „(c) neu, Einrichtung“ gilt für den Stand vom 21.11.2026 (Exit 19). Im Stand des Folgeschritts endet `postinst` mit Exit 0, trägt das Harness nicht ein, schreibt die Journalzeile und die Zustandsdatei, und `werkbank-pruefen` zeigt die Zeile des Harness. Gemessen ist das bisher in `test_bau.py` und den Fällen; die Messung in der Wegwerf-VM (Neuinstallation und Update mit laufenden Diensten) steht aus.

Admin-Felder (gleiche VM): Mit `lauf.groesse` `[150, 45]` und `port` `8765` (mit `json.dumps(indent=1)` geschrieben) und von Hand geändertem `deflate` bleiben nach `dpkg -i` des neuen deb und `dpkg-reconfigure` beide Werte, `deflate` ist wieder `true`, die Dateien `root:root` 0644; die Brücke schreibt nach dem Start den Port `8765` in `port_datei`. Ungültige Werte (`[10, 40]`, `70000`) ergeben die Vorgabe und je eine der festen Journalzeilen. Gegenprobe: `dpkg -i` des deb mit dem `postinst` des Stands vom 21.11.2026 setzt beide Werte auf `[120, 40]` und `0` zurück. Doppelter Schlüssel `ca_zulassung`: `werkbank-pruefen` zeigt `pruef.hinweis_ca_doppelt`.

Fälle `laufkonfig-*` (Form `laufkonfig`, Referenz `laufkonfig`; neu am 21.11.: `prefix-local`, `prefix-npm-global`, `paket-mit-scope`, `node-unter-lokal-bin`, `ordner-genestet-bin` und die acht Fälle `verboten-*`, deren Feld `pruefer_verboten` die Antwort des Prüfmodus nennt (erwartet: das Harness fehlt in der Datei, Feld `abgelehnt_datei` mit dem Inhalt der Zustandsdatei und `journal` mit den festen Zeilen; vor dem Folgeschritt „weich auslassen“ war die Erwartung `postinst_code` 19 ohne Datei); der Rust-Test `tests/programmordner_faelle.rs` hält die echte Regel an diese Liste und prüft, dass jeder erwartete Programmordner sie besteht; geänderte Erwartung von `pi-nvm`, `beide-harness`, `ordner-genestet`: statt des Präfixes die Paketwurzel und der bin-Ordner von node): ohne Harness, pi unter nvm, Copilot unter `/usr/local`, beide, einzelnes Programm unter `/opt`, verschachtelte Ordner, anderer Nutzer, längster Nutzername mit größter uid, drei Fälle der Zulassung, kein Nutzer, Heimordner mit Leerzeichen, relativer Heimordner, Harnesspfad mit Anführungszeichen. Fälle `entfernen-lauf-json-bleibt`, `entfernen-etc-ordner-bleibt` und `entfernen-laufzeitordner-bleibt` belegen, dass `purge` nichts davon zurücklässt.

### Zulassung der CA (Nachtrag 21.11.2026, Projektentscheid)

Nach der Installation steht in `/etc/werkbank/lauf.json` das Feld `ca_zulassung` auf `offen`. Dann endet der Modellweg der Cloud-Route mit `mw.ca_nicht_zugelassen` und die Cloud-Sitzung startet nicht. Das ist gewollt (fail closed). Das Eintragen von `zugelassen` ist **ein ausdrücklicher Schritt eines Admins mit root**: nie still, nie durch den Läufer, nie durch `postinst` aus eigenem Antrieb. Es gibt genau zwei Wege, die dieselbe Datei und dieselbe Protokollzeile erzeugen.

**(a) Die Frage bei `sudo dpkg-reconfigure werkbank` (debconf).** Die Dateien `DEBIAN/templates` und `DEBIAN/config` stellen die Frage `werkbank/ca_zulassung`, Art `select`, Werte `zugelassen`, `offen`, `abgelehnt`, Vorgabe `offen`, Priorität `low`: `dpkg-reconfigure` fragt (sein Standard ist `low`), eine Installation fragt nicht. Mit `DEBIAN_FRONTEND=noninteractive` wird nie gefragt: Der bisherige Wert bleibt, frisch ist er `offen`. Wortlaut der Frage (genau der Text der Datei `templates`, ASCII):

```
Zulassung der lokalen Zertifizierungsstelle fuer die Cloud-Route
 Mit "zugelassen" gibt der Administrator der Cloud-Route frei, je Sitzung ein
 eigenes Zertifikat zu erzeugen. Das Zertifikat gilt hoechstens 23 Stunden, nur
 fuer die Ziele der Zielliste der Sitzung, und sein Schluessel liegt ausserhalb
 des Einschlusses.

 Mit "offen" (Vorgabe) startet die Cloud-Route nicht, bis ein Administrator
 zulaesst. Mit "abgelehnt" ist die Cloud-Route ebenfalls gesperrt, als
 ausdrueckliche Entscheidung. Die lokale Route ist in beiden Faellen nicht
 betroffen.

 Ohne Rueckfrage geht dasselbe mit: sudo werkbank-ca-zulassung <Wert>
```

Für ein Skript ohne Dialog: `echo 'werkbank werkbank/ca_zulassung select zugelassen' | sudo debconf-set-selections`, dann `sudo dpkg-reconfigure -f noninteractive werkbank`.

**Leere Eingabe im Frontend `teletype`:** Eine leere Eingabe (nur Eingabetaste) setzt die Vorgabe der Vorlage, also `offen` (die sichere Richtung, Projektentscheid: so lassen), auch wenn vorher `zugelassen` galt. Wer `zugelassen` behalten will, tippt den Wert ein oder lässt die Frage mit `-f noninteractive` aus.

**`dpkg-reconfigure` lässt Kern und Brücke laufen.** Der Admin-Schritt hält keine Einheit an: `prerm` erkennt `dpkg-reconfigure` an `DEBCONF_RECONFIGURE=1` und tut nichts; `postinst` startet und stoppt keine Einheit. Das Gleiche gilt für `werkbank-ca-zulassung <Wert>` (schreibt nur die Datei).

**(b) Der Befehl ohne Rückfrage für die Einrichtung per Skript:** `sudo werkbank-ca-zulassung <zugelassen|offen|abgelehnt>` (Datei `/usr/bin/werkbank-ca-zulassung`, Shell-Skript, `root:root` 0755, Positivliste Art `programm`). Er ändert genau ein Feld der Datei `lauf.json`, schreibt sie atomar wie `postinst` (temporäre Datei im selben Ordner, `root:root` 0644, dann `rename`; die Datei ist nie halb geschrieben) und lässt jedes andere Byte unverändert. Gleicher Wert: nichts wird geschrieben, keine Protokollzeile, rc 0. Danach setzt er die Antwort von debconf auf denselben Wert und die Marke `seen` auf `false` (`debconf-communicate werkbank`), damit die nächste Frage den Wert als Vorgabe zeigt; scheitert nur dieser Abgleich, meldet er eine Warnung auf stderr und rc bleibt 0 (die Datei gilt). Feste Exitcodes, jeweils ohne Änderung der Datei bei einem Fehler:

| rc | Bedeutung |
|---|---|
| 0 | Wert gesetzt oder schon gesetzt |
| 64 | Aufruf falsch: kein oder mehr als ein Argument, oder ein Wert außer `zugelassen`, `offen`, `abgelehnt` (genau, klein) |
| 66 | `/etc/werkbank/lauf.json` fehlt, ist ein Verweis oder keine gewöhnliche Datei, oder hat nicht genau ein Feld `ca_zulassung` der Form `"ca_zulassung"` `:` `"<kleinbuchstaben>"` (JSON-Leerraum um den Doppelpunkt und Zeilenumbrüche sind erlaubt; ein zweiter Schlüssel `"ca_zulassung"` irgendwo in der Datei macht die Datei ungültig) |
| 74 | die Datei konnte nicht geschrieben werden (die temporäre Datei wird entfernt) |
| 77 | nicht root |

**(c) Die Protokollzeile.** Jede Änderung des Wertes (durch `postinst` oder durch den Befehl) schreibt genau eine Zeile ins Journal, mit demselben Mittel wie das bestehende Protokoll des Pakets (`logger -t werkbank-installation`; `journalctl -t werkbank-installation`):

`werkbank: ca_zulassung <neu> (vorher <alt>), Weg <debconf|befehl|paket>, uid <n>[, sudo_uid <n>], Zeit <JJJJ-MM-TTThh:mm:ssZ>`

`<alt>` ist `zugelassen`, `offen`, `abgelehnt`, `keiner` (die Datei gab es nicht) oder `ungueltig` (die Datei trug einen Wert außerhalb der drei; der rohe Wert erscheint nie). `Weg debconf` heißt: die wartende Antwort der Frage; `Weg befehl`: `werkbank-ca-zulassung`; `Weg paket`: die erste Anlage der Datei oder das Zurücksetzen eines ungültigen Wertes ohne Antwort (`offen`). `uid` ist die tatsächliche Kennung des ausführenden Prozesses (`id -u`), `sudo_uid` nur, wenn `SUDO_UID` eine Zahl ist. Die Zeile nennt keinen Pfad, keinen Namen und kein Geheimnis. Gleicher Wert: keine Zeile.

**(d) `werkbank-pruefen`.** Unter Ubuntu steht der Zustand als eigene Hinweiszeile (kein Befund, rc bleibt 0, kein Teil von `pruef.rot`): `offen` und `abgelehnt` heißen „Cloud-Route gesperrt“ und sind **kein Fehler der Installation**; `zugelassen` ist die grüne Zeile; ist die Datei da, aber nicht lesbar (Verweis, kein JSON, einer der drei Werte fehlt), steht `pruef.hinweis_ca_unbekannt` (die Cloud-Route gilt als gesperrt). Fehlt die Datei ganz, gibt es keine Zeile (das Paket hat sie nicht geschrieben, zum Beispiel ohne `sudo`-Nutzer). Die Sätze stehen in der Tabelle der Codes; die Zeile steht nach den Zeilen zu Ubuntus Profil und zum System-`bwrap` und vor `pruef.hinweis_sudo`. Die Lage eines Falls trägt das Feld `ca_zulassung` (`zugelassen`, `offen`, `abgelehnt`, `unbekannt`, `doppelt`); fehlt es, gibt es keine Zeile (alle früheren Fälle bleiben, wie sie sind). Fälle `pruefung-linux-ca-*`. **Doppelter Schlüssel (Nachtrag 21.11.2026):** Steht `"ca_zulassung"` mehr als einmal in der Datei (das Paket zählt den Text, auch als Wert eines anderen Feldes), gilt das wie im Paket als `offen`; `werkbank-pruefen` zeigt dafür die eigene Zeile `pruef.hinweis_ca_doppelt` (Hinweis, kein Befund) und nicht mehr `pruef.hinweis_ca_unbekannt`. Fall `pruefung-linux-ca-doppelter-schluessel`; Test `ein_doppelter_schluessel_ca_zulassung_gilt_wie_im_paket_als_offen_mit_eigener_zeile` (alt: `unbekannt`, die Zeile `pruef.hinweis_ca_unbekannt`; neu: `doppelt`, die Zeile `pruef.hinweis_ca_doppelt`).

**Welche Felder der Admin setzt, und was ein Update davon behält (Projektentscheid: ein Update überschreibt keine Einstellung des Admins).** Weg: `postinst` liest den vorhandenen Wert und schreibt ihn wieder; `lauf.json` ist **kein** `conffile`, weil sie je Nutzer von `postinst` erzeugt wird und nicht im Paket liegt (dpkg fragt nur bei Dateien, die das Paket liefert).

| Datei und Feld | Wer setzt es | Was `postinst` bei Installation, Update und `dpkg-reconfigure` tut |
|---|---|---|
| `/etc/werkbank/lauf.json`, Feld `ca_zulassung` | der Admin (Frage, Befehl oder von Hand) | liest den Wert **JSON-tolerant** (siehe unten) und schreibt ihn in die feste Form zurück; ein anderes Werkzeug, das die Datei neu schrieb, verliert den Wert nicht |
| `/var/lib/werkbank/<nutzer>/einstellungen.json` (darin die Gateway-Adresse) | der Betreiber | wird nur kopiert, wenn sie fehlt; nie überschrieben (Schritt 2) |
| `…/modellweg.schluessel`; `<heim>/.local/state/agent-workbench/halter/geheim/anmeldung.copilot` (Nachtrag Halter; alt: `…/copilot.schluessel`) | der Betreiber (`werkbank-anmelden`) | `postinst` legt sie nie an und fasst sie nie an |
| `/etc/werkbank/halter.json` (Nachtrag Halter) | **niemand** in Schritt I | wird bei jeder Installation neu aus `passwd` und der Vorlage geschrieben (Nachtrag Erststart: Vorlage ist das Erststart-Profil `erststart.json`, samt Schalter `erststart_antwortform`) |
| `/etc/werkbank/kern.json`, Feld `lauf.groesse` | der Admin (von Hand) | liest den Wert JSON-tolerant (`tr`, `grep`, `sed`, wie bei `ca_zulassung`), prüft ihn und schreibt ihn wieder; ungültig oder doppelt: die Vorgabe `[120, 40]` und die Journalzeile `werkbank: kern.json: groesse ungueltig, die Vorgabe gilt` |
| `/etc/werkbank/bruecke.json`, Feld `port` | der Admin (von Hand) | ebenso; ungültig oder doppelt: die Vorgabe `0` und die Journalzeile `werkbank: bruecke.json: port ungueltig, die Vorgabe gilt` |
| `lauf.json`, alle übrigen Felder; `kern.json` und `bruecke.json`, alle übrigen Felder | **niemand**: das Paket erzeugt sie bei jeder Installation neu aus `passwd`, der Suche und der festen Vorlage | keine Admin-Felder; eine Änderung von Hand geht bei einem Update verloren (entschieden: Änderungen gehören in den Befehl, die Frage oder `einstellungen.json`) |

**Admin-Felder von `kern.json` und `bruecke.json` (Nachtrag 1 Projektentscheid, 21.11.2026, Projektentscheid), je Feld mit Grund.**
- `kern.json`, `lauf.groesse` (`[<Spalten>, <Zeilen>]`): Die Größe des Terminals jeder Sitzung. Ein Admin kann sie sinnvoll ändern (große Bildschirme, kleine Fenster, Vorgabe `[120, 40]`); der Dienst prüft sie ohnehin (Spalten ab `SPALTEN_MIN`, höchstens 1000). Gültig im Paket: Spalten 20 bis 1000, Zeilen 5 bis 1000, ganze Zahlen ohne führende Null. Grund der engeren Grenze: ein Terminal unter 20 Spalten oder 5 Zeilen lässt keine Sitzung bis zum Eingabefeld laufen; der Dienst würde es annehmen, und die Sitzung bliebe ohne sichtbaren Grund leer.
- `bruecke.json`, `port` (0 bis 65535, ganze Zahl ohne führende Null): Der Port der Brücke auf `127.0.0.1`. `0` (Vorgabe) lässt das System wählen, die Brücke schreibt ihn in `port_datei`; ein Admin kann einen festen Port brauchen (Firewall, Weiterleitung des Browsers). Der Wert wird so gelesen, wie die Brücke ihn liest (`ganzzahl` 0 bis 65535).
- **Keine Admin-Felder**, mit Grund: `deflate` (beide Dateien müssen denselben Wert tragen, die Brücke sendet ihn als `X-Deflate`; ein einzelner Schalter in einer Datei trennt die beiden und bricht die Verbindung); `socket`, `dienst.socket`, `token_datei`, `port_datei` (aus der uid abgeleitet und mit `RuntimeDirectory` abgestimmt; ein anderer Pfad bricht den Start oder die Grenze von 100 Byte); `betrachter`, `anfang` (die Person kommt aus `SUDO_USER`); `lauf.programm`, `lauf.env` (fester Vertrag mit dem Läufer: die Umgebung ist genau `TERM`), `lauf.einstellungen`, `lauf.sitzungen`, `lauf.gespraeche`, `lauf.marker_datei`, `lauf.marker_ordner` (Ordner, die `postinst` anlegt und prüft; ein anderer Ort umginge die Prüfungen von Schritt 1a).
- **Lesen:** wie bei `ca_zulassung`: `tr` setzt Tabulator, CR und LF auf Leerzeichen; Leerzeichen um den Doppelpunkt und im Paar sind erlaubt; der Schlüssel (`"groesse"`, `"port"`) muss genau einmal in der Datei stehen und der Wert die gültige Form haben, sonst gilt die Vorgabe mit der festen Journalzeile (ohne Pfad und Inhalt). Fehlt der Schlüssel in der alten Datei (früherer Stand des Pakets) oder fehlt die Datei, gilt die Vorgabe ohne Zeile. Der alte Wert wird gelesen, **bevor** die Datei neu geschrieben wird; die Datei bleibt `root:root` 0644 und wird atomar ersetzt wie bisher. Referenz `referenz.dienst_admin`. Fälle `dienstkonfig-admin-*` (der erwartete Inhalt der Dateien und, wo es sie gibt, die Journalzeilen im Feld `journal`); Tests `DienstJson.test_every_case_with_an_old_file_gives_the_reference_and_the_journal_lines` und `…test_the_admin_fields_survive_an_update_and_a_new_file_has_the_defaults`.
- **Upgrade von einem Paket ohne den Fix:** Das Upgrade von einem deb ohne diesen Fix lässt Kern und Brücke einmal aus, denn das alte `prerm` schreibt keine Notiz in `/run/werkbank-paket-neustart`, und `postinst` startet nur, was die Notiz nennt. Das wird hingenommen (Projektentscheid, 21.11.2026): Die Organisation installiert frisch. Der Nutzer startet beide Einheiten einmal von Hand (`systemctl --user start werkbank-kern werkbank-bruecke`) oder meldet sich neu an.

**Lesen von `ca_zulassung` (JSON-tolerant, nur POSIX-Werkzeuge, keine neue Abhängigkeit):** `postinst`, `config` und `werkbank-ca-zulassung` lesen die Datei mit `tr`, `grep` und `sed`. Beliebiger Leerraum (Leerzeichen, Tabulator, CR, LF) um den Doppelpunkt und nach `{` oder `,` ist erlaubt. Es zählt genau ein Schlüssel `"ca_zulassung"` in der Datei und nur einer der Werte `zugelassen`, `offen`, `abgelehnt` ohne Leerzeichen im Wert. Ein zweiter Schlüssel, ein anderer Wert oder ein Wert mit Leerzeichen (`"zugelassen "`) ergibt wie bisher `offen` mit der Protokollzeile `(vorher ungueltig), Weg paket`. Der Befehl ändert nur den Wert und lässt jedes andere Byte der Datei, auch Leerraum und Zeilenenden, unverändert. Fälle: `laufkonfig-vorher-umgeschrieben-indent1`, `…-zeilen`, `…-doppelter-schluessel`, `…-leerzeichen-im-wert`; Tests `test_a_value_rewritten_by_another_tool_is_read_as_json`, `test_a_doubled_key_or_a_value_with_a_blank_gives_offen_and_one_line`, `test_step_2_never_overwrites_the_settings_file_of_the_operator` und, für den Befehl, `test_a_file_rewritten_by_another_tool_is_read_and_only_the_value_changes`.

**Antwort und Datei laufen nie auseinander.** Die Datei `lauf.json` ist die Wahrheit. Eine Antwort von debconf zählt nur, **solange ihre Marke `seen` wahr ist** (der Dialog von `dpkg-reconfigure` oder `debconf-set-selections` setzt sie). `postinst` verbraucht die Antwort: Er schreibt den Wert in die Datei, setzt die Antwort auf den geschriebenen Wert und `seen` auf `false`. Daraus folgt:

- Ein Upgrade, eine erneute Installation desselben Pakets und `dpkg-reconfigure` ohne Antwort ändern den Wert nie (es wartet keine Antwort, die Datei bleibt, wie sie ist; auch ein von Hand eingetragener Wert bleibt, wie bisher).
- Hat ein Admin den Befehl benutzt, zeigt die nächste Frage diesen Wert als Vorgabe: Der Befehl setzt die Antwort mit, und `config` setzt vor der Frage den Wert der Datei als Vorgabe, wenn keine Antwort wartet.
- Eine wartende Antwort mit einem Wert außerhalb der drei wird nie übernommen; die Datei gilt.
- Schreibt `postinst` keine Datei (kein `sudo`-Nutzer, unsaubere Ordnerform), wartet die Antwort weiter und es gibt keine Protokollzeile.
- `purge` entfernt die Antwort aus debconf (`db_purge`); eine neue Installation beginnt mit `offen`.

Fälle: `laufkonfig-antwort-*` (die wartende Antwort gilt vor der Datei, auch zum Zurücknehmen; ein ungültiger Wert wird ignoriert), die Fälle `pruefung-linux-ca-*` und die Tests des Baus (`rust/paket/test_bau.py`: `LaufJson`, `CaBefehl`, `CaFrage`).

**Gemessen (Wegwerf-VM aus dem Cloud-Abbild, Ubuntu 26.04.1, 21.11.2026; deb aus dem Stand vom 21.11.2026, gebaut mit `bauhost.sh` in der VM, `paket.sh: built in 194 s`).**

| Schritt | Ergebnis |
|---|---|
| frische Installation, `DEBIAN_FRONTEND=noninteractive`, ohne Antwort | rc 0; `lauf.json` 644 `root:root` mit `"ca_zulassung":"offen"`; debconf-Antwort `offen`, Marke `seen` false; eine Protokollzeile `offen (vorher keiner), Weg paket` |
| `werkbank-pruefen` (Schlüsseldateien mit Fantasiewerten, Kern und Brücke gestartet) | `pruef.hinweis_ca_offen` und `pruef.ok: Alle 8 Pruefungen bestanden.`, **rc 0** |
| `sudo werkbank-ca-zulassung zugelassen`, danach `offen` | rc 0 je Aufruf; Datei, debconf-Antwort und `werkbank-pruefen` (`pruef.hinweis_ca_zugelassen`, rc 0) folgen; je eine Zeile mit `Weg befehl` |
| derselbe Wert noch einmal; Wert `ja`; kein Argument; als Nutzer ohne root | rc 0 ohne neue Zeile; rc 64; rc 64; rc 77; die Datei bleibt, wie sie war |
| `debconf-set-selections` `zugelassen`, dann `dpkg-reconfigure -f noninteractive` | die Marke `seen` ist nach `debconf-set-selections` **true**; danach Datei `zugelassen`, Zeile `zugelassen (vorher offen), Weg debconf`, Marke false |
| `dpkg-reconfigure -f noninteractive` ohne neue Antwort (Wert offen, Wert zugelassen) | Datei und Antwort unverändert, keine neue Zeile |
| Befehl setzt `offen`, danach `dpkg-reconfigure -f noninteractive` | bleibt `offen` |
| wartende Antwort `zugelassen`, dann Befehl `abgelehnt`, dann `dpkg-reconfigure -f noninteractive` | bleibt `abgelehnt` (der Befehl setzt die Marke zurück) |
| derselbe deb noch einmal: `apt-get install --reinstall` und `dpkg -i` | Wert `zugelassen` bleibt, in debconf und Datei, keine neue Zeile |
| Datei von Hand auf `offen`, dann `dpkg-reconfigure -f noninteractive` | Wert der Datei bleibt, die debconf-Antwort folgt |
| Pseudoterminal (`script`), Frontend `teletype`, Installation mit `--reinstall` | die Frage erscheint **nicht** (0 Treffer) |
| Pseudoterminal, `dpkg-reconfigure` mit `teletype` und mit `readline`, Standardpriorität | die Frage erscheint (1 Treffer, der Fragetext wie in `templates`); Antwort `abgelehnt` oder `zugelassen` gilt: Datei, Zeile `Weg debconf` |
| Pseudoterminal, `teletype`, **leere Eingabe** bei Wert `zugelassen` | Der Wert wird `offen`: `config` setzt die Antwort vor der Frage auf `zugelassen` (gemessen im Entwicklerprotokoll von debconf), die Eingabe ohne Auswahl nimmt aber die Vorgabe der Vorlage (`offen`). Das ist die sichere Richtung; ausdrücklich gilt nur eine gewählte Antwort |
| `apt-get purge` | die Antwort ist aus debconf weg (`debconf-show` leer, 0 Treffer in `config.dat` und `templates.dat`), `/etc/werkbank` weg, `find` nach `*werkbank*` ohne die Systemprotokolle 0 |
| neue Installation nach `debconf-set-selections zugelassen` | Datei `zugelassen` in einem Zug, Zeile `zugelassen (vorher keiner), Weg debconf` |

**Kern und Brücke nach dem Admin-Schritt (Wegwerf-VM, 21.11.2026, deb aus dem Stand vom 21.11.2026 gebaut; „fix“ = derselbe deb mit `prerm` und `postinst` dieses Stands). Ursache: `dpkg-reconfigure` ruft `prerm upgrade <Version>` mit `DEBCONF_RECONFIGURE=1` und danach `postinst configure <Version>` auf (`/usr/sbin/dpkg-reconfigure`, Zeilen 109 bis 112); `prerm` hielt dabei Brücke und Kern an (Journal: `Stopping werkbank-bruecke`, dann `werkbank-kern`, 0,25 s nach dem Start des Befehls), und `postinst` startete nichts. Eine Neuinstallation (`prerm upgrade`) hielt sie aus demselben Grund an.** Vor jedem Schritt laufen beide Einheiten:

| Schritt | ohne Fix: Kern, Brücke | mit Fix: Kern, Brücke |
|---|---|---|
| `dpkg-reconfigure -f noninteractive` | inaktiv, inaktiv | aktiv, aktiv (gleiche PID, nie angehalten) |
| `werkbank-ca-zulassung zugelassen` | aktiv, aktiv | aktiv, aktiv (gleiche PID) |
| `debconf-set-selections`, dann `dpkg-reconfigure -f noninteractive` | inaktiv, inaktiv | aktiv, aktiv (gleiche PID) |
| `dpkg-reconfigure` mit `teletype` (Eingabe `abgelehnt`) | inaktiv, inaktiv | aktiv, aktiv (gleiche PID) |
| `apt-get install --reinstall` desselben deb | inaktiv, inaktiv | aktiv, aktiv (neue PID: angehalten und wieder gestartet) |
| `dpkg -i` desselben deb | inaktiv, inaktiv | aktiv, aktiv (neue PID) |
| Einheiten vor dem Schritt aus, dann `--reinstall` und `dpkg-reconfigure` | aus, aus | aus, aus (nichts wird gestartet) |
| nur der Kern lief, dann `--reinstall` | inaktiv, inaktiv | aktiv, inaktiv |

Die Aktualisierung von einem deb ohne diesen Fix auf einen mit ihm lässt die Einheiten einmal aus (das alte `prerm` schreibt keine Notiz; hergeleitet, nicht gemessen).

Nicht gemessen: das Frontend `dialog` (die VM hat kein Terminal mit `dialog`-Sitzung; `teletype` und `readline` laufen im Pseudoterminal). Nach `dpkg-reconfigure` waren Kern und Brücke in der Messung nicht mehr aktiv (Ursache nicht untersucht); das ist unabhängig von der CA-Zeile, die nie als Befund zählt.

### Entfernen: nach `purge` ist nichts mehr da

Ein sichtbarer Fall belegt „nach purge nichts mehr da“. Form `entfernen`: `vorher` ist die Liste der Einträge des Systems vor der Installation, `nachher` die nach `apt purge`. Ein Eintrag hat die Form `datei:<pfad>`, `ordner:<pfad>` (absolute Pfade), `konto:<name>`, `gruppe:<name>`, `profil:<name>`, `einheit:<name>` oder `prozess:<name>`. Die Prüfung meldet genau einen Code:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.entfernen_form` | ein Eintrag hat keine bekannte Art oder keinen absoluten Pfad | `Eintrag hat falsche Form` |
| 2 | `paket.entfernen_rueckstand` | ein Eintrag steht nach `purge` da, stand nicht vorher da und ist keine ausdrückliche Ausnahme | `Rueckstand nach purge` |
| 3 | `paket.entfernen_verloren` | ein Eintrag stand vorher da und fehlt nach `purge` (das Paket hat Fremdes entfernt) | `Fremdes nach purge verloren` |

**Die Ausnahmen sind fest und werden ausdrücklich genannt**; jeder andere Eintrag ist ein Rückstand:

1. die Ergebnisdatei am Ort von `--ergebnis` (genau dieser Pfad, keine zweite Datei);
2. der Ordner `~/.copilot` des Nutzers, der `sudo` aufrief, und alles darunter (die Anmeldung der Copilot CLI; sie gehört nicht zum Paket; Annahme 12). Das Heim ist `<heimwurzel>/<nutzer>`; die Fälle tragen die Heimwurzel im Feld `heimwurzel` (Beispiel `/srv/beispiel/heim`, nicht `/home`, damit die Beispiele nicht in das Leckmuster der Pfade fallen; die Logik der Ausnahme ist dieselbe);
3. die Systemprotokolle: `/var/log/dpkg.log`, alles unter `/var/log/apt/`, `/var/log/journal/`, `/var/cache/apt/` und `/var/lib/apt/`.

**Ablage des Halters (Nachtrag Halter):** Sie ist **keine** Ausnahme. Nach `purge` steht weder `anmeldung.copilot` noch eine der Ebenen `geheim`, `halter` oder `agent-workbench` da, es sei denn, eine Ebene stand vorher da oder enthält Fremdes (dann bleibt sie, und das Fremde darin bleibt unverändert). `.local/state` und `.local` gehören nie dem Paket. Fälle `entfernen-ablage-*`, `entfernen-halter-*`, `remove-ablage-*`.

Die Fälle `entfernen-*` belegen: nach `purge` fehlt nichts Fremdes, bleibt nichts außer den drei Ausnahmen, und jede Art von Rückstand (Datei, Ordner, Programm, Profil, Einheit, Prozess, Konto, Gruppe) macht den Fall rot. **Eine Regel gilt für Profile unabhängig vom Zustand vorher:** Ein Profil mit dem Label `werkbank-*` oder einem Pfad unter `/usr/lib/werkbank/` steht nach `purge` nie mehr da, auch wenn es schon vor dieser Installation da war (Rest einer älteren). Das eigene Profil von Ubuntu für das System-`bwrap` bleibt und ist kein Rückstand. **`apt remove` (Form `remove`).** Der Fall nennt `nach_installation` und `nach_remove` (Listen der Einträge), die Ausgabe des Entfernens (`meldung`), das Journal und die Marke des Test-Schlüssels. Die Prüfung meldet genau einen Code:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.remove_schluessel_verloren` | die Schlüsseldatei war nach der Installation da und fehlt nach `remove` (das tut nur `purge`) | `remove hat die Schluesseldatei geloescht` |
| 2 | `paket.remove_hinweis_fehlt` | die Schlüsseldatei liegt nach `remove` da, aber die Ausgabe enthält den Satz `pruef.schluessel_rest` nicht | `Hinweis auf die Schluesseldatei fehlt` |
| 3 | `paket.remove_hinweis_inhalt` | die Marke des Schlüssels steht in der Ausgabe oder im Journal | `Schluessel im Hinweis` |

**Anmeldung in der Ablage (Nachtrag Halter):** Nennt ein Fall `heimwurzel`, gilt dasselbe für `<heimwurzel>/<nutzer>/.local/state/agent-workbench/halter/geheim/anmeldung.copilot`: fehlt sie nach `remove`, obwohl sie nach der Installation da war, gilt Stufe 1; liegt sie da und die Ausgabe enthält `pruef.copilot_rest` nicht, gilt Stufe 2. Ohne `heimwurzel` prüft ein Fall nur die Schlüsseldatei (alle früheren Fälle bleiben).

Die Außentests AL-5 und AL-6 erzeugen die Listen auf einer echten VM (Dateiliste vorher und nachher, `getent`, `apparmor_status`, `systemctl`, `ps`).

### Steuerdatei (`DEBIAN/control`)

Pflichtfelder: `Package`, `Version`, `Architecture`, `Depends`, `Description`. `Package` ist `werkbank`, `Architecture` ist `amd64`, `Version` gleicht der Version des Manifests; beim Gerüstbau (`--geruest`) lautet sie `<Version des Manifests>~geruest` (Fälle `steuerdatei-*geruest*`). `Depends` nennt mindestens `python3`, `bubblewrap`, `apparmor`, `ca-certificates`, `systemd` und `libssl3t64` (mit oder ohne Versionsangabe, auch mit Alternative) und dazu die Bibliotheken aus `dpkg-shlibdeps`. `libssl3t64` ist gemessen: In der Ubuntu-26.04.1-VM bindet `werkbank-modellweg` die Bibliotheken `libssl.so.3` und `libcrypto.so.3`, und `dpkg -S` nennt beide im Paket `libssl3t64` (Ergebnis). `dpkg-shlibdeps` trägt es selbst ein; die feste Liste macht es zur Pflicht, damit ein Bau ohne diese Zeile nie entsteht. Der Name ist genau: `libssl3` ist eine andere Zeile. Dazu trägt der Bau `debconf` ein (das Paket stellt eine Frage, und `postinst`, `config` und `postrm` laden den `confmodule`); die Referenz verlangt es nicht, der Test des Baus (`test_bau.py`) prüft es. Jede Abhängigkeit steht in Ubuntu `main`.

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.steuer_form` | Zeichen außerhalb ASCII oder Steuerzeichen, letzte Zeile ohne LF, Feld ohne Doppelpunkt, doppeltes Feld, Fortsetzungszeile ohne Feld | `Steuerdatei hat falsche Form` |
| 2 | `paket.steuer_feld_fehlt` | ein Pflichtfeld fehlt oder ist leer | `Pflichtfeld der Steuerdatei fehlt` |
| 3 | `paket.steuer_wert` | `Package` nicht `werkbank`, `Architecture` nicht `amd64`, `Version` ungleich der Manifest-Version (beim Gerüstbau: ungleich `<Manifest-Version>~geruest`; ein Auslieferungsbau trägt nie `~geruest`) | `Wert der Steuerdatei falsch` |
| 4 | `paket.steuer_abhaengigkeit` | eine der sechs festen Abhängigkeiten fehlt | `Abhaengigkeit fehlt` |

### Zusagen und Außentests des deb

| Nr | Zusage | Prüfung |
|---|---|---|
| ZL1 | Das Paket enthält die Attrappe nicht und kein Testwerkzeug; die einzige Python-Datei ist der Läufer. | Fälle `manifest-merkmal-verboten-linux`, `liste-*linux*`; AL-8 |
| ZL2 | Der Schlüssel steht nicht im Paket, in keiner Ausgabe, keinem Protokoll, keiner Befehlszeile und keiner Ergebnisdatei; die Datei hat Modus 0600 und gehört dem Nutzer. | Fälle `einrichten-*`, `pruefung-linux-schluessel-*`; AL-4 |
| ZL3 | Das Profil gilt nur für den festen Einschluss: Mit ihm gelingt der Probelauf unter aktiver Sperre, ohne es scheitert er. | AL-3 |
| ZL8 | **Unser Paket öffnet das System-`bwrap` nicht:** Kein Profil des Pakets hängt an `/usr/bin/bwrap`; ob ein Nutzer ohne Vorrechte `/usr/bin/bwrap --unshare-user` selbst nutzen kann, hängt von Ubuntus eigenem Profil ab (mitgeliefert: ja; entfernt: nein; gemessen, siehe Hinweis im Prüfbefehl). Nach `apt purge` ist kein Label `werkbank-*` mehr geladen. | Fälle `profil-*`, `pruefung-linux-apparmor-system-bwrap-frei`, `entfernen-profil-*`; AL-3 |
| ZL4 | Der System-Truststore gilt: ein Wurzelzertifikat, das die IT einträgt, wirkt; eines, das sie nicht einträgt, wirkt nicht; das Paket bringt kein Bündel mit. | Fälle `stueckliste-verboten-*`, `steuerdatei-abhaengigkeit-ca-certificates`; AL-7 |
| ZL5 | Die Installation braucht kein Netz und keine Eingabe. | AL-1 |
| ZL6 | Eine veränderte `.deb`-Datei erkennt der Prüfsummenvergleich. Ob `apt` und `dpkg` sie allein erkennen, ist gemessen, nicht vorausgesetzt. | AL-2 |
| ZL7 | Nach `apt purge` bleibt nichts außer den drei genannten Ausnahmen. | Fälle `entfernen-*`; AL-5, AL-6 |

| Nr | Ausgangslage | Handlung | Erwartet | Gemessen |
|---|---|---|---|---|
| AL-1 | frische VM Ubuntu 26.04.1 amd64 (auf der Linux-Messmaschine), kein Netz, die sechs Pakete installiert | die vier Zeilen der Anleitung bis einschließlich Schritt 3 mit Attrappen für Copilot CLI und Gateway | Rückgabe der Installation 0, Eingaben außer Adresse und Schlüssel 0; **Installation und Entfernen zusammen höchstens 15 Minuten** | Rückgaben, Dauer in Sekunden, Mitschnitt: Verbindungen beim Installieren 0 |
| AL-2 | echtes deb und ein Zweitpaket mit einem geänderten Byte | Prüfsummenvergleich je Paket; danach `apt install ./` und `dpkg -i` mit dem Zweitpaket | der Vergleich lehnt das Zweitpaket ab; was `apt` und `dpkg` allein tun, wird festgehalten | Rückgaben, Ausgaben |
| AL-3 | VM mit aktiver Sperre | Installation; Probelauf des Einschlusses als normaler Nutzer; `/usr/bin/bwrap --unshare-user true` als normaler Nutzer außerhalb des Läufers; Gegenprobe: Profil entladen und wiederholen | Profil geladen, Probelauf grün; ohne Profil rot mit der Sperrmeldung; **das System-`bwrap` ohne den Läufer scheitert weiter (gesperrt), mit dem Profil geladen** | `apparmor_status`, Rückgaben beider Aufrufe |
| AL-4 | installiertes Paket, Schlüssel mit der Marke eingegeben | Schlüssel in Ausgaben, Journal, `ps`, `/proc/<pid>/cmdline`, Umgebung, Protokoll und Ergebnisdatei suchen; im Einschluss `/var/lib/werkbank` suchen | Treffer außerhalb der Datei 0; Pfad im Einschluss nicht sichtbar; Datei 0600 | Trefferzahl, Modus, Eigentümer |
| AL-5 | VM mit Basiszustand | Listen vorher/nachher erzeugen (Dateien unter `/usr`, `/etc`, `/var`, `/home`; Konten; Gruppen; Profile; Einheiten; Prozesse), installieren, Läufer starten, `apt purge`; Listen vergleichen | Rückstände 0 außer den drei Ausnahmen; Verluste 0 | Zähler der Rückstände und Verluste, Liste der Ausnahmen |
| AL-6 | wie AL-5, aber `apt remove` statt `purge` | Listen vergleichen | `/var/lib/werkbank` bleibt (Beleg, warum `purge` verlangt wird) | Zähler |
| AL-7 | VM mit einem lokalen TLS-Server, dessen Wurzel nicht im System steht | Modellweg gegen den Server; danach die Wurzel in den System-Truststore einlesen (`update-ca-certificates`); danach wieder entfernen | abgelehnt; angenommen; abgelehnt | Rückgaben je Schritt |
| AL-8 | gebautes deb (jeder Linux-Host) | `dpkg-deb -c` und `-x`; Dateimenge mit Manifest und Positivliste vergleichen; die Attrappe suchen | Dateimenge deb = Manifest = Positivliste (außer `usr/share/werkbank/manifest.json`); keine Attrappe | Zähler abweichender Dateien: 0 |
| AL-9 | installiertes Paket | je Code der Linux-Tabelle den Zustand erzeugen (Einheit anhalten, Profil entladen, Datei ändern, Modus ändern, Nutzerordner löschen), `werkbank-pruefen` ausführen | Zeilen und rc wie in den Fällen `pruefung-linux-*` | Zeilen und rc je Zustand; Abweichungen 0 |
| AL-10 | VM mit zwei weiteren Konten, ohne Stelle | `werkbank-pruefen` (A108); Gegenproben: ein Konto; ein echter Fehler | rc 20 mit der Stellenzeile als letzter Zeile; rc 0; rc 1 | Rückgaben, letzte Zeile |
| AL-11 | VM mit aktiver Sperre, Paket installiert, ein **fremder Nutzer** ohne Vorrechte | der fremde Nutzer ruft `/usr/lib/werkbank/werkbank-einschluss` auf: mit `selbsttest`, mit einem fremden Argument (`roh`, `--help`, ein bwrap-Schalter), ohne Argument; Gegenprobe: ein Profil mit `unconfined` unter `werkbank-*` laden, Prüfbefehl; danach entladen | Der Starter führt nur den festen Einschluss aus; **jedes fremde Argument endet mit rc 2** und ohne Namensraum; der Prüfbefehl ist mit dem `unconfined`-Profil rot (`pruef.apparmor_unconfined`) und danach grün | Rückgaben je Aufruf, Zeilen des Prüfbefehls |

| AL-12 | VM mit aktiver Sperre, Paket installiert, **S1**: Ubuntus Profil `bwrap-userns-restrict` geladen (Auslieferung) | `werkbank-pruefen`; als Nutzer ohne Vorrechte `werkbank-einschluss selbsttest` und `/usr/bin/bwrap --unshare-user true`; `apparmor_status` | Der Einschluss läuft (`selbsttest` rc 0); der Prüfbefehl zeigt **zwei Hinweiszeilen**, erst `pruef.hinweis_ubuntu_profil_geladen`, dann `pruef.hinweis_system_bwrap_ubuntu`, und keine Zeile `pruef.apparmor_`; das System-`bwrap` ist frei (rc 0, Hinweis); `werkbank-einschluss`, `-bwrap` und `-kind` stehen in `apparmor_status` | Zeilen und rc des Prüfbefehls, Label des Kindes, Rückgaben beider Aufrufe |
| AL-13 | wie AL-12, aber **S0**: Ubuntus Profil entladen (`apparmor_parser -R /etc/apparmor.d/bwrap-userns-restrict`) | dieselben Aufrufe; danach das Profil wieder laden (`-r -W`) | Der Einschluss läuft **nur mit unserem Profil** (`selbsttest` rc 0); der Prüfbefehl zeigt **eine** Hinweiszeile, `pruef.hinweis_ubuntu_profil_entladen`, keine Zeile `pruef.hinweis_system_bwrap_ubuntu`; das System-`bwrap` scheitert (`uid map: Permission denied`, rc 1); nach dem Wiederladen wie AL-12 | Zeilen und rc, Rückgaben, Label des Kindes |
| AL-16 | VM mit Paket (Nachtrag Rolle aus der Gruppe), ein Nutzer | `getent group wb-bruecke`; `stat -c '%U:%G %a' /usr/lib/werkbank/werkbank-modellweg`; `dpkg-statoverride --list`; `apt install --reinstall`; dann `adduser <nutzer> wb-bruecke` und `werkbank-pruefen`, danach `deluser <nutzer> wb-bruecke`; `apt purge` | Gruppe mit gid 100 bis 999 ohne Mitglied; `root:wb-bruecke 2755`; ein Eintrag; nach dem Update unverändert; mit Mitglied `pruef.rollengruppe_mitglied` und rc 1, `wb-halter.service` mit Exit 78; nach `purge` weder Gruppe noch Eintrag | Ausgaben, Zeilen, Exitcode |
| AL-17 | VM mit Paket, Nutzer angemeldet (Nachtrag Rolle aus der Gruppe) | eine Sitzung der Cloud-Route; `/proc/<pid>/status` des Modellwegs (Zeile `Gid:`) als root; Gegenprobe: eine Kopie des Modellwegs ohne setgid unter dem Heim stellt `schluessel.vorhanden` an den Halter | effektive gid `wb-bruecke`, reale gid der Person; der Modellweg bekommt die Rolle (kein `halter.peer` im Journal des Halters); die Kopie bekommt `halter.peer` | gid-Felder, Antworten |
| AL-14 | VM mit Paket, ein Nutzer angemeldet (Nachtrag Halter) | `systemctl --user start wb-halter`; `werkbank-anmelden copilot --uebernehmen` mit einer Marke statt Token; `werkbank-pruefen`; dann `apt remove`, danach `apt purge`; Listen der Ablage je Schritt | Einheit aktiv, Socket 0600 im Ordner 0700, `/proc/<pid>` des Halters gehört root (Schritt 0); `pruef.ok`; nach `remove` liegt die Ablage und `pruef.copilot_rest` steht in der Ausgabe; nach `purge` fehlen `anmeldung.copilot` und die drei Ebenen, Fremdes unter `.local/state` bleibt | Zähler, Modi, Zeilen |

Die VM, die Buchung und die Maschine legt das Projekt fest; gemessen wird mit dem Bau. Der Vertrag startet keine VM.

## Teil A2: Linux-Paket, Schritt II: Halter unter dem Systemnutzer `wb-halter` (Fassung 2, Entwurf)

Dieser Teil ist eine eigene Fassung neben Teil A. **Teil A bleibt für Schritt I gültig und unverändert prüfbar:** Jeder Fall ohne das Feld `schritt` prüft nach Teil A, und keine Erwartung eines bestehenden Falls ändert sich. Ein Fall mit `"schritt": 2` prüft nach diesem Teil. Das deb trägt genau eine Fassung; welche, entscheidet der Bau (Folgezug Code, OPEN 30).

**Entscheid des Betreibers (23.11.2026, aus der Fragerunde):** Entscheidung 4 aus Teil A („kein Konto, keine Gruppe“) ist für den Systemnutzer `wb-halter` und drei Rollengruppen **aufgehoben**, angelegt mit `adduser --system`. `purge` löscht Konto und Gruppen nach Weg a: erst wenn `find -uid` und `find -gid` nichts mehr finden und kein Prozess des Nutzers läuft. Offen bleibt die Brücke (O1 oder O5), erst nach den Messungen L-1 bis L-3. Grund für den Systemnutzer: Ohne eigene uid trennt nichts die Ablage von einem Prozess der Person außerhalb des Einschlusses (Halter-Vertrag OPEN 18).

**Was gemessen ist:** nichts aus diesem Teil. Jede Aussage über Kernel, `dpkg`, `adduser` und systemd ist hergeleitet. Wo eine Regel an einer Messung hängt, steht sie als Annahme mit dem Namen der Messung (Liste am Ende). Die Messungen L-1 bis L-7 stammen aus dem Plan zum Systemnutzer des Halters (Abschnitt 6).

### Entscheidungen in Schritt II

1. **Ein Systemkonto und vier Systemgruppen.** Konto `wb-halter` mit eigener Hauptgruppe `wb-halter`, Heim `/nonexistent`, Shell `/usr/sbin/nologin`, uid im Bereich der Systemkonten. Dazu die drei Rollengruppen `wb-hub`, `wb-bruecke` und `wb-oberflaeche`. Die Hauptgruppe `wb-halter` ist keine Rollengruppe; sie entsteht durch `adduser --system --group` und gehört dem Konto (OPEN 30 c).
2. **Kein Konto ist Mitglied einer Rollengruppe**, weder die Person noch `wb-halter`. Eine Rolle bekommt ein Prozess nur durch den Start eines setgid-Rollenprogramms (Option O1 des Plans). Wäre die Person Mitglied, hätte jeder ihrer Prozesse die Rolle.
3. **Der Halter läuft als System-Einheit** `wb-halter.service` unter `User=wb-halter`. Die Benutzereinheit gleichen Namens aus Teil A entfällt.
4. **Die Ablage liegt unter `/var/lib/wb-halter/geheim/`** und gehört `wb-halter`. Die Ablage im Heim der Person aus Teil A zieht beim Update um und ist danach leer.
5. **Entfernen:** `remove` lässt Ablage, Konto und Gruppen liegen. `purge` löscht alles nach Weg a; mit Rest bleiben Konto und Gruppen, und das Journal nennt den Code 23.
6. Teil A, Entscheidungen 1 bis 3 und 5 bis 10, gilt weiter, soweit dieser Teil nichts anderes sagt. Die Dienste `werkbank-kern.service` und `werkbank-bruecke.service` bleiben Benutzereinheiten; Entscheidung 4 gilt für sie unverändert.

### Konto und Gruppen (`preinst`)

`preinst` (`install` und `upgrade`) legt Konto und Gruppen an, **bevor** `dpkg` die Dateien entpackt. Grund: Das Rollenprogramm mit setgid trägt eine Rollengruppe, die beim Entpacken bestehen muss (Annahme A2-1). Die Schritte:

1. Bestehen `wb-halter` oder eine der vier Gruppen schon, prüft `preinst` ihre Form: Konto mit uid von 100 bis 999, Hauptgruppe `wb-halter`, Heim `/nonexistent`, Shell `/usr/sbin/nologin`; Gruppe mit gid von 100 bis 999 und ohne Mitglieder. Weicht eine ab, endet `preinst` mit dem **festen Code 24** und ändert nichts (fail closed). Ein fremdes Konto oder eine fremde Gruppe dieses Namens wird nie übernommen. `dpkg` entpackt dann nichts.
2. Fehlendes legt `preinst` an: `adduser --system --group --no-create-home --home /nonexistent --shell /usr/sbin/nologin wb-halter`, danach `addgroup --system wb-hub`, `addgroup --system wb-bruecke`, `addgroup --system wb-oberflaeche`. Bestehendes in richtiger Form bleibt unverändert.
3. Kein anderes Konto und keine andere Gruppe ändert sich.

Der Bereich 100 bis 999 ist die Vorgabe von `adduser` (`FIRST_SYSTEM_UID`, `LAST_SYSTEM_UID`; hergeleitet, Außentest AL-15). Code 24 ist der einzige Abbruch dieses Teils; er fällt vor das Entpacken, darum widerspricht er Entscheidung 7 aus Teil A nicht. `preinst` braucht `adduser` vor dem Entpacken; die Steuerdatei bekommt dafür `Pre-Depends: adduser` (hergeleitet nach Debian Policy 7.2; die Prüfung der Steuerdatei ändert sich dadurch nicht, OPEN 30 d).

Form `systemnutzer`: `vorher` und `nachher` mit `konten` (je `name`, `uid`, `gruppe`, `heim`, `shell`) und `gruppen` (je `name`, `gid`, `mitglieder`), dazu `preinst_rc`. Die Prüfung meldet genau einen Code:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.systemnutzer_form` | ein Stand hat nicht die Form (Feld fehlt, Name doppelt, uid keine ganze Zahl) | `Stand der Konten hat falsche Form` |
| 2 | `paket.konto_fremd` | ein fremdes Konto oder eine fremde Gruppe eines der fünf Namen stand vorher da, und `preinst` endete nicht mit 24 oder hat etwas geändert | `Fremdes Konto uebernommen` |
| 3 | `paket.konto_fehlt` | `preinst` endete nicht mit 0, oder nach `preinst` fehlt das Konto oder eine der vier Gruppen | `Konto oder Gruppe fehlt` |
| 4 | `paket.konto_form` | das Konto oder eine der vier Gruppen hat nach `preinst` nicht die Form aus Schritt 1 | `Konto hat falsche Form` |
| 5 | `paket.rollengruppe_mitglied` | eine der vier Gruppen hat ein Mitglied | `Rollengruppe hat ein Mitglied` |
| 6 | `paket.konto_verloren` | ein anderes Konto oder eine andere Gruppe hat sich geändert oder fehlt | `Fremdes Konto veraendert` |

Fälle: `systemnutzer-*`.

### Die Einheit `wb-halter.service` (System-Einheit)

Datei `/usr/lib/systemd/system/wb-halter.service` (`daten`, root, 0644, Quelle `datei:paket/ubuntu-26.04-x64/wb-halter.system.service`). Die Datei `/usr/lib/systemd/user/wb-halter.service` aus Teil A ist in Schritt II nicht mehr im Paket. Inhalt (hergeleitet aus dem Plan, Abschnitt 3):

- `User=wb-halter`, `Group=wb-halter`, kein `SupplementaryGroups=`. Der Halter hält nie eine Rollengruppe.
- `ExecStart=/usr/lib/werkbank/werkbank-halter` ohne Argument; `halter.json` bleibt `/etc/werkbank/halter.json` (`root:root`, 0644, Teil A).
- `StateDirectory=wb-halter`, `StateDirectoryMode=0700`; die Ablage ist `/var/lib/wb-halter/geheim/` (0700), je Geheimnis eine Datei 0600 von `wb-halter`.
- `UMask=0077`, `Restart=on-failure`, `RestartPreventExitStatus=5 64 74 78`, `[Install] WantedBy=multi-user.target`.
- Härtung, weil der Halter nichts startet: `NoNewPrivileges=yes`, `ProtectHome=yes`, `ProtectSystem=strict`, `PrivateTmp=yes`, `CapabilityBoundingSet=` (leer), `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`. Der Halter braucht Netz für seine Ziele. `ProtectHome=yes` sperrt das Heim der Person; den Umzug macht darum `postinst`, nie der Halter.
- Keine Zeile `Environment`, `EnvironmentFile` oder `PassEnvironment`, keine Socket-Einheit (Teil A gilt).
- **Socket je Rolle (Annahme A2-2, Zug 3):** Ordner `/run/wb-halter/` (`root:root`, 0755) und je Rolle `/run/wb-halter/<rolle>/` mit `wb-halter:wb-<rolle>` und 0750. Sie legt `systemd-tmpfiles` an (`/usr/lib/tmpfiles.d/wb-halter.conf`, root, 0644), nicht `RuntimeDirectory`. Grund: `RuntimeDirectory` gehört nur dem Dienstnutzer, und der Halter darf die Gruppe nicht selbst setzen, weil er keine Rollengruppe hält. Die Form der Sockets legt Zug 3 fest (Halter 2.3 Fassung 2).

`postinst` (`configure`) ruft in Schritt 4 `systemctl enable --now wb-halter.service` statt `systemctl --global enable … wb-halter.service`. Die Notiz der Neustarts (Schritt 4b) führt nur Kern und Brücke; der Halter startet nach dem Umzug (unten). `werkbank-pruefen` liest die Einheit des Halters ohne `--user` (Folge im Prüfbefehl, OPEN 30 e).

### Stufe 9 mit setgid-Liste

In Schritt II hat Stufe 9 der Gegenprobe einen zweiten Teil. Der erste Teil bleibt: kein Eintrag ist für Gruppe oder Welt schreibbar (`paket.modus_schreibbar`). Danach gilt:

- setuid (04000) und das Sticky-Bit (01000) sind nie erlaubt.
- setgid (02000) ist nur an den Pfaden der **festen setgid-Liste** erlaubt, und dort genau mit Modus 2755 und der Gruppe der Liste. Ein Pfad der Liste im Baum ohne setgid ist ebenso falsch, denn dann hätte das Rollenprogramm keine Rolle.
- Im Fall der Form `liste` steht die Gruppe im optionalen Feld `gruppen` (Pfad zu Gruppe); ein setgid-Eintrag ohne Gruppe ist falsch.

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 9b | `paket.modus_setgid` | Schritt II: setuid oder Sticky-Bit; setgid außerhalb der Liste; ein Pfad der Liste ohne genau 2755 oder mit anderer Gruppe | `setgid ausserhalb der Liste` |

**Die setgid-Liste (Annahme A2-3, bis Zug 3):**

| Pfad | Gruppe | Rolle | Stand |
|---|---|---|---|
| `usr/lib/werkbank/werkbank-modellweg` | `wb-bruecke` | `bruecke` | Rollenprogramm der Rolle `bruecke` unter Linux (Halter-Vertrag 2.3) |
| Programm der Rolle `hub` | `wb-hub` | `hub` | folgt mit Zug 3; heute gibt es unter Linux kein Rollenprogramm `hub` |
| Programm der Rolle `oberflaeche` | `wb-oberflaeche` | `oberflaeche` | folgt mit Zug 3 und dem Weg der Anmeldung (Halter-Vertrag OPEN 18) |

Die Referenz führt die Liste als `referenz.SETGID_LISTE`. Kommt ein Rollenprogramm hinzu, ändert sich die Liste, nicht die Regel. Wie die Gruppe in das Archiv kommt, ist hergeleitet: Der Bau setzt `root:wb-<rolle>` mit 2755 in den gestuften Baum, und `dpkg` setzt beim Entpacken die Gruppe nach ihrem Namen (Annahme A2-1). Fälle: `liste-setgid-*`. Der Fall `liste-setgid-schritt1-unveraendert` belegt, dass Teil A setgid nicht prüft (OPEN 30 f).

### Update von Schritt I auf Schritt II: Umzug der Ablage

`postinst configure` zieht die Ablage der Person aus Teil A nach `/var/lib/wb-halter/geheim/` um. Die Person ist die aus `/etc/werkbank/lauf.json` (`nutzer` aus `zustand`, `heim`), sonst die aus `SUDO_USER` und `passwd`, wie beim `purge` in Teil A. Der Schritt läuft bei jedem `configure` und ist wiederholbar. Reihenfolge:

1. Die Benutzereinheit `wb-halter.service` der Person ist angehalten (`prerm upgrade` hat sie angehalten). `postinst` schaltet sie global ab (`systemctl --global disable wb-halter.service`) und löscht die Verknüpfung unter `/etc/systemd/user/default.target.wants/`, wie Schritt 4a in Teil A für den Modellweg.
2. **Lesen als die Person:** `setpriv --reuid=<uid> --regid=<gid> --init-groups`, jede Ebene `.local`, `.local/state`, `agent-workbench`, `halter`, `geheim` relativ und mit `O_NOFOLLOW`; ist eine Ebene ein Verweis oder nicht die der Person, gilt der Umzug als gescheitert. Gelesen werden nur reguläre Dateien, deren Name eine Kennung ist und nicht mit einem Punkt beginnt (Halter-Vertrag 2.3).
3. **Schreiben als root:** jede Datei in einen eigenen Ordner `/var/lib/wb-halter/geheim/.umzug/` (Name mit Punkt, also nie ein Geheimnis), `fsync`, `chown wb-halter:wb-halter`, `chmod 0600`, Inhalt mit der Quelle vergleichen. Erst wenn alle Dateien stimmen, benennt `postinst` sie nach `geheim/` um und löscht `.umzug/`.
4. **Konflikt:** Liegt ein Name schon in `geheim/` mit anderem Inhalt, gilt der Umzug als gescheitert. Liegt er dort mit gleichem Inhalt (ein früherer Lauf brach nach Schritt 3 ab), gilt er als umgezogen.
5. **Quelle räumen**, erst nach Schritt 3: als die Person jede umgezogene Datei überschreiben und entfernen, danach `geheim`, `halter` und `agent-workbench` mit `rmdir`, nur wenn leer; nie `.local/state` oder `.local` (wie `purge` in Teil A).
6. `systemctl enable --now wb-halter.service`, dann Kern und Brücke der Person nach Schritt 4b neu starten.

**Kein halber Zustand.** Scheitert ein Schritt vor Schritt 5, löscht `postinst` `.umzug/`, lässt Quelle und Ziel unverändert und schreibt die feste Journalzeile `werkbank: Umzug der Ablage gescheitert, Code 21` (ohne Pfad, Name und Inhalt). Die Installation bricht nicht ab (Entscheidung 7), `postinst` endet mit 0. Der Halter startet dann mit der Ablage, die er hat; `werkbank-pruefen` meldet die fehlende Anmeldung. Ein späteres `sudo dpkg-reconfigure werkbank` versucht den Umzug erneut. Code 21 ist kein Exitcode, sondern eine Journalzeile wie Code 19 in Teil A.

Form `umzug`: `nutzer`, `quelle_vorher`, `quelle_nachher`, `ziel_vorher`, `ziel_nachher` (Listen der Dateien je mit `name`, `sha256`, `modus`, `eigentuemer`), `journal` und `postinst_rc`. Die Prüfung meldet genau einen Code:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.umzug_form` | eine Liste hat nicht die Form (Name keine Kennung, Name doppelt, Prüfsumme falsch) | `Ablage hat falsche Form` |
| 2 | `paket.umzug_abbruch` | `postinst` endete nicht mit 0 | `Umzug bricht die Installation ab` |
| 3 | `paket.umzug_konflikt` | ein Name lag mit anderem Inhalt im Ziel, und Quelle oder Ziel haben sich geändert | `Umzug ueberschreibt ein Geheimnis` |
| 4 | `paket.umzug_inhalt` | eine umgezogene Datei hat anderen Inhalt, nicht 0600 oder nicht `wb-halter` | `Umgezogene Datei falsch` |
| 5 | `paket.umzug_halb` | das Ziel ist weder vollständig noch unverändert, oder die Quelle fehlt teilweise ohne vollständiges Ziel | `Umzug halb` |
| 6 | `paket.umzug_quelle_bleibt` | das Ziel ist vollständig, aber eine Datei der Quelle liegt noch da | `Quelle nach dem Umzug nicht leer` |
| 7 | `paket.umzug_code_fehlt` | der Umzug ist gescheitert, aber das Journal nennt Code 21 nicht | `Gescheiterter Umzug ohne Code` |
| 8 | `paket.umzug_code_falsch` | das Journal nennt Code 21, obwohl der Umzug gelang oder nichts umzuziehen war | `Code 21 ohne gescheiterten Umzug` |

Fälle: `umzug-*`, darunter `umzug-gueltig-zwei-geheimnisse` (n = 2), `umzug-gueltig-ein-geheimnis`, `umzug-gueltig-ohne-quelle` (n = 0) und `umzug-gescheitert-quelle-bleibt`.

### `remove` in Schritt II

`prerm remove` hält `wb-halter.service` an und ruft vorher `werkbank-pruefen --vor-entfernen` wie in Teil A. `postrm remove` schaltet die Einheit ab. Ablage, Konto und Gruppen bleiben, denn die Ablage gehört dem Konto. Die Meldung nennt `pruef.copilot_rest`, wenn `/var/lib/wb-halter/geheim/anmeldung.copilot` liegt. Form `remove` mit `"schritt": 2`: Jede Datei und jeder Ordner unter `/var/lib/wb-halter` aus `nach_installation` muss in `nach_remove` stehen (sonst Stufe 1), der Hinweis gilt für `anmeldung.copilot` an diesem Ort (Stufe 2), und zusätzlich:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 4 | `paket.remove_konto_verloren` | Schritt II: das Konto `wb-halter` oder eine der vier Gruppen war nach der Installation da und fehlt nach `remove` | `remove hat Konto oder Gruppe geloescht` |

Fälle: `remove-konto-*`.

### `purge` in Schritt II (Weg a)

`postrm purge` tut, was Teil A sagt, und zusätzlich in dieser Reihenfolge:

1. `wb-halter.service` anhalten und abschalten, die Einheit und `/usr/lib/tmpfiles.d/wb-halter.conf` gehen mit dem Paket; `/run/wb-halter` entfernen.
2. Jede Datei in `/var/lib/wb-halter/geheim/` überschreiben und entfernen, dann `/var/lib/wb-halter` entfernen. Liegt noch eine Ablage der Person aus Teil A (gescheiterter Umzug), räumt `postrm` sie als die Person wie in Teil A.
3. **Prüfung auf Rest, über jedes lokale Dateisystem.** `purge` liest zuerst die uid des Kontos und die gid jeder der vier Gruppen. Dann holt es die Liste der Einhängepunkte mit `findmnt --raw --noheadings --output TARGET,FSTYPE` (gleichwertig: `/proc/self/mountinfo`; die Ausgabe schreibt Leerzeichen und Sonderzeichen als `\xNN`, `purge` wandelt sie zurück). Je Einhängepunkt entscheidet der Typ:
   - **Typ in der Ausnahmeliste (unten):** keine Suche, auch nicht, wenn der Punkt nicht lesbar ist.
   - **Typ in der Liste der durchsuchten Typen (unten):** `find <punkt> -xdev \( -uid <uid> -o -gid <gid> -o -gid <gid2> … \)` mit der uid des Kontos und der gid jeder der vier Gruppen in einem Aufruf. Jede Ausgabe ist ein Rest. Endet `find` mit einem Fehler (Punkt nicht lesbar, verschwunden, Lesefehler), ist das ebenfalls ein Rest.
   - **jeder andere Typ:** unbekannt. Er wird nicht durchsucht und ist ein Rest.

   Dazu die Prozesse des Kontos (`pgrep -u wb-halter`; jeder Treffer ist ein Rest). Scheitert `findmnt` (Rückgabewert ungleich 0 oder keine Zeile), ist auch das ein Rest. Findet die Prüfung keinen Rest: `deluser --system wb-halter`, dann `delgroup --system` je Gruppe. **Fail closed:** Was `purge` nicht prüfen kann, zählt nie als sauber.
4. **Mit Rest:** Konto und Gruppen bleiben, die Reste bleiben unberührt, und das Journal bekommt die feste Zeile `werkbank: Konto wb-halter und Rollengruppen bleiben, Code 23` (ohne Pfad und ohne Grund). `purge` bricht nicht ab. Es gibt keinen neuen Code: Ob eine Datei, ein Prozess, ein unbekannter Typ, ein nicht lesbarer Punkt oder ein Fehler von `findmnt` den Rest ergab, ändert weder die Folge noch die Zeile. Grund: Eine gelöschte uid vergäbe das System später neu, und der neue Eigentümer bekäme die Reste.

Ein Rest in `/var/lib/wb-halter` ist kein Rest im Sinn von Schritt 4, sondern ein Rückstand: Schritt 2 hat dort alles zu löschen. Das gilt auch, wenn `/var/lib/wb-halter` auf einer eigenen Partition liegt.

**Ausnahmeliste (feste Liste im Vertrag; `referenz.MOUNT_AUSGENOMMEN`).** Diese Typen durchsucht `purge` nie. Jede Zeile nennt den Grund; alle Gründe sind hergeleitet, nicht gemessen.

| Typ | Grund |
|---|---|
| `proc` | Ansicht der Prozesse im Kernel, keine Dateien auf einem Datenträger |
| `sysfs` | Ansicht der Geräte und Treiber im Kernel |
| `devtmpfs` | Gerätedateien, die der Kernel bei jedem Start neu anlegt |
| `devpts` | Pseudo-Terminals, nur im Arbeitsspeicher |
| `tmpfs` | liegt im Arbeitsspeicher und ist nach dem nächsten Start leer (Restrisiko bis dahin: OPEN 30 g) |
| `ramfs` | wie `tmpfs`, im Arbeitsspeicher |
| `cgroup`, `cgroup2` | Ansicht der Kontrollgruppen im Kernel |
| `securityfs` | Schnittstelle der Sicherheitsmodule im Kernel |
| `debugfs`, `tracefs` | Schnittstellen zur Fehlersuche im Kernel |
| `configfs` | Konfigurationsschnittstelle des Kernels |
| `fusectl` | Steuerdatei für FUSE, keine Daten |
| `pstore` | vom Kernel gefüllter Speicher für Absturzprotokolle |
| `bpf` | Ansicht angehefteter BPF-Objekte im Kernel |
| `mqueue` | Nachrichtenwarteschlangen im Kernel |
| `hugetlbfs` | Seiten des Arbeitsspeichers, keine Datenträgerdateien |
| `binfmt_misc` | Schnittstelle für Programmformate im Kernel |
| `efivarfs` | Variablen der Firmware |
| `rpc_pipefs` | Rohre des Kernels für NFS-Dienste |
| `nsfs` | Ansicht der Namensräume im Kernel |
| `autofs` | Einhängepunkt, der erst beim Zugriff füllt; die Ziele erscheinen als eigene Punkte |
| `overlay` | zeigt Dateien anderer Einhängepunkte, deren Ebenen die Suche selbst durchläuft |
| `fuse.gvfsd-fuse`, `fuse.portal` | Sitzungsansichten des Desktops; `root` liest sie ohne `allow_other` nicht |
| `squashfs`, `erofs` | schreibgeschützte Abbilder (etwa Snap-Pakete); kein Programm des Pakets legt dort etwas an |
| `iso9660`, `udf` | schreibgeschützte Medien; kein Programm des Pakets legt dort etwas an |
| `nfs`, `nfs4`, `cifs`, `smb3` | Netz-Dateisysteme; die Dateien gehören einem Server, und die Suche hinge bei einem Ausfall |
| `9p` | Verzeichnis des Wirts in einer VM; kein Programm des Pakets legt dort etwas an |
| `fuse.sshfs` | Netz über SSH; wie die Netz-Dateisysteme |

**Durchsuchte Typen (`referenz.MOUNT_DURCHSUCHT`):** `ext2`, `ext3`, `ext4`, `xfs`, `btrfs`, `f2fs`, `jfs` und `zfs` (Linux-Datenträger, dort kann ein Programm des Pakets Dateien mit der uid anlegen); `vfat`, `exfat` und `ntfs3` (Fremdformate, auch die EFI-Partition `/boot/efi`; sie speichern keine uid, `find` meldet dort die uid der Einhängeoption); `fuseblk` (NTFS über FUSE auf einem Blockgerät). Jeder Typ, der in keiner der beiden Listen steht, ist unbekannt und ein Rest. Kommt ein Typ hinzu, ändert sich die Liste im Vertrag, nicht die Regel.

Form `entfernen` mit `"schritt": 2`, dazu `rest` (`dateien`: absolute Pfade, die `find` fand; `prozesse`: Namen) und `journal`. Ohne Rest gilt Teil A unverändert: Konto, Gruppen, Ablage und Einheit sind Rückstände. Mit Rest sind genau das Konto, die vier Gruppen und die gemeldeten Reste außerhalb von `/var/lib/wb-halter` freigestellt; ein Prozess `werkbank-*` bleibt ein Rückstand.

**Einhängepunkte im Fall (neu, optional, nur mit `"schritt": 2`).** Ein Fall kann die Lage der Dateisysteme nennen; die Referenz leitet daraus den Rest ab, nach Schritt 3 oben:
- `einhaengepunkte`: Liste der Einhängepunkte, wie `findmnt` sie liefert. Je Eintrag genau `pfad` (absolut, einmalig), `typ`, `lesbar` (Wahrheitswert) und `dateien` (absolute Pfade der Dateien mit der uid oder gid auf diesem Dateisystem, die nach Schritt 2 noch da sind). Die Wurzel `/` steht immer in der Liste. `dateien` darf nur ein durchsuchter, lesbarer Punkt tragen; auf einem ausgenommenen Typ kann `purge` nichts finden, und der Fall darf dort nichts behaupten. Jede Datei liegt unter ihrem Punkt.
- `findmnt_fehler` (Wahrheitswert, ohne Angabe `false`): `findmnt` ist gescheitert. Dann ist `einhaengepunkte` leer.
- Mit `einhaengepunkte` ist `rest.dateien` leer; die gefundenen Dateien kommen aus der Liste. `rest.prozesse` gilt weiter. Ohne `einhaengepunkte` gilt `rest.dateien` wie bisher (bestehende Fälle, byte-gleich).
- Rest ist: eine Datei auf einem durchsuchten, lesbaren Punkt; ein nicht lesbarer Punkt eines durchsuchten Typs; ein Punkt mit unbekanntem Typ; ein Fehler von `findmnt`; ein Prozess. Verstößt der Fall gegen die Form (keine Wurzel, Punkt doppelt, Datei auf ausgenommenem Typ oder außerhalb ihres Punkts, Liste bei `findmnt_fehler`, `rest.dateien` zusätzlich), meldet die Prüfung `paket.entfernen_form`.
- `alte_regel` (`gruen` oder `rot`, optional): Gegenprobe. Sie sagt, wie die ersetzte Regel `find / -xdev` den Fall urteilen würde. `pruefen.py` führt diese Regel aus (`referenz.entfernen_code(fall, suche="wurzel")`; sie kennt nur das Wurzel-Dateisystem) und meldet einen Fehler, wenn das Urteil nicht stimmt. `gruen`: die alte Regel nähme den Fall an, obwohl die neue ihn mit einem Code ablehnt (sie ließe das Konto trotz Rest löschen). `rot`: die alte Regel lehnte den richtigen Fall ab.

Zusätzlich:

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 4 | `paket.entfernen_konto_trotz_rest` | Schritt II: es gab einen Rest (Datei auf einem beliebigen lokalen Dateisystem, Prozess, nicht durchsuchbarer Einhängepunkt oder Fehler von `findmnt`), aber das Konto oder eine der vier Gruppen fehlt nach `purge` | `Konto trotz Rest geloescht` |
| 5 | `paket.entfernen_rest_ohne_meldung` | Schritt II: Konto und Gruppen bleiben wegen eines Rests (auch eines nicht durchsuchbaren Einhängepunkts), aber das Journal nennt Code 23 nicht | `Rest ohne Code 23` |

Fälle: `entfernen-konto-*`, darunter `entfernen-konto-gueltig-weg` (nichts bleibt), `entfernen-konto-rest-datei-bleibt` und `entfernen-konto-rest-prozess-bleibt`.

Fälle der Suche über Einhängepunkte (`entfernen-konto-mount-*`, 20 Fälle):
- `mount-rest-zweite-partition`: Rest auf einer zweiten Partition; Konto und Gruppen bleiben, Code 23 (gültig; die alte Regel lehnte ihn ab). `mount-rest-auf-zwei-partitionen` und `mount-rest-zweite-partition-ohne-meldung` (Code `entfernen_rest_ohne_meldung`) ergänzen ihn.
- **Gegenprobe** `mount-rest-zweite-partition-nur-wurzel-geloescht`: dieselbe Lage, aber `purge` suchte nur mit `find / -xdev` und löschte Konto und Gruppen (`entfernen_konto_trotz_rest`). Die alte Regel urteilt diesen Fall grün; `alte_regel` hält das fest, und `pruefen.py` prüft es.
- `mount-nicht-lesbar-bleibt` (gültig) und `mount-nicht-lesbar-geloescht` (`entfernen_konto_trotz_rest`): ein nicht lesbarer Punkt ist ein Rest. Ebenso für `mount-unbekannter-typ-*` und `mount-findmnt-fehler-*`.
- `mount-nur-pseudo-ohne-rest-weg`: neben der Wurzel nur Pseudo-, Netz- und Abbild-Dateisysteme und `/boot/efi`, kein Rest; Konto und Gruppen sind weg. `mount-nur-pseudo-ohne-rest-konto-bleibt` ist derselbe Fall mit stehendem Konto (`entfernen_rueckstand`). `mount-pseudo-nicht-lesbar-weg`: ein nicht lesbarer Punkt eines ausgenommenen Typs ist kein Rest.
- `mount-rest-in-ablage-eigene-partition`: Datei in `/var/lib/wb-halter` auf eigener Partition ist ein Rückstand (`entfernen_rueckstand`).
- `mount-form-*`: sechs Fälle der Form (`entfernen_form`).

Alle bestehenden Fälle und Erwartungen sind unverändert; `bau-faelle.py` schreibt sie byte-gleich.

### Feste Codes der Skripte in Schritt II

| Code | Skript | Wirkung | Text |
|---|---|---|---|
| 21 | `postinst` | kein Abbruch, Journalzeile | `werkbank: Umzug der Ablage gescheitert, Code 21` |
| 23 | `postrm purge` | kein Abbruch, Journalzeile | `werkbank: Konto wb-halter und Rollengruppen bleiben, Code 23` |
| 24 | `preinst` | Abbruch vor dem Entpacken, nichts geändert | `werkbank: Konto oder Gruppe wb-* besteht in fremder Form, Code 24` |

Die Nummern 20 und 22 bleiben frei; 20 ist der Rückgabewert des Prüfbefehls (A108).

### Die Brücke: O1 oder O5 (offen, nichts festgelegt)

Die Entscheidung fällt erst nach L-1 bis L-3. Dieser Teil nennt nur die Folgen für das Paket.

**Befund vorab (aus dem Repo, nicht gemessen):** Unter Linux ist das Rollenprogramm der Rolle `bruecke` `werkbank-modellweg` (Halter-Vertrag 2.3), nicht `werkbank-bruecke`. `NoNewPrivileges=yes` steht in `werkbank-bruecke.service`, nicht in der Kern-Einheit. Der Modellweg startet der Läufer je Sitzung, und der Kern läuft ohne `NoNewPrivileges` (Teil A, „Dienste“). Hergeleitet: Startet der Läufer den Modellweg außerhalb des Einschlusses, greift sein setgid-Bit auch ohne jede Änderung an `werkbank-bruecke.service`. Die Frage O1 gegen O5 betrifft dann nur den Fall, dass `werkbank-bruecke` selbst eine Rolle braucht. Die Messung L-3 muss das zeigen: `NoNewPrivs` und gid des Modellwegs in einer Sitzung.

**Folgen von O1 (setgid, Brücke ohne `NoNewPrivileges`):**
- `werkbank-bruecke.service` verliert `NoNewPrivileges=yes` und jede Option, die es nach sich zieht (die Liste `referenz.KERN_EINHEIT_NNP_SCHLUESSEL`). Die Härtung der Brücke wird schwächer; das braucht eine Entscheidung des Betreibers.
- Die setgid-Liste bekommt `usr/lib/werkbank/werkbank-bruecke` mit `wb-bruecke`, falls die Brücke selbst Rollenprogramm wird.
- Die Brücke bleibt Benutzereinheit; Entscheidung 4 gilt für sie unverändert. `BindsTo` und `After` zur Kern-Einheit bleiben.

**Folgen von O5 (Brücke als System-Einheit unter der Person):**
- Eine neue System-Einheit, etwa `werkbank-bruecke@<nutzer>.service`, mit `User=<nutzer>` und `Group=wb-bruecke` oder `SupplementaryGroups=wb-bruecke`. `NoNewPrivileges=yes` bleibt; die Gruppe kommt ohne setgid.
- Entscheidung 4 wäre auch für diese Einheit aufgehoben. Das geht über den Entscheid vom 23.11. hinaus und braucht eine eigene Entscheidung.
- `BindsTo=werkbank-kern.service` geht nicht über die Grenze von System- und Nutzer-Manager (hergeleitet). Start und Ende der Brücke brauchen einen anderen Weg.
- `werkbank-pruefen`, `prerm`, `postrm` und die Notiz der Neustarts kennen dann eine System-Einheit mehr. Die Brücke braucht keinen Eintrag in der setgid-Liste.

### Annahmen und die Messung dazu

| Nr | Annahme | Messung |
|---|---|---|
| A2-1 | `dpkg` setzt beim Entpacken die Gruppe einer Datei nach ihrem Namen im Archiv, wenn die Gruppe besteht; der Bau kann `root:wb-<rolle>` mit 2755 ins Archiv schreiben, ohne dass der Bauhost die Gruppe kennt | L-8 (neu): Bau auf dem Bauhost und Installation in einer Wegwerf-VM; `stat` der Datei nach dem Entpacken |
| A2-2 | Socket je Rolle in Ordnern von `systemd-tmpfiles`; `connect` ohne Rollengruppe ergibt `EACCES` | L-2; Form in Zug 3 |
| A2-3 | die setgid-Liste hat heute einen Eintrag (`werkbank-modellweg`) | Zug 3 |
| A2-4 | `SO_PEERCRED` liefert für ein setgid-Rollenprogramm die Rollengruppe als gid | L-1 |
| A2-5 | Der Modellweg läuft ohne `NoNewPrivs`, sein setgid-Bit greift | L-3 |
| A2-6 | Ein setgid-Rollenprogramm ist für die Person nicht anhängbar | L-4 |
| A2-7 | `adduser --system` vergibt uid und gid von 100 bis 999 und lässt ein bestehendes Systemkonto gleicher Form unverändert | AL-15 |

### Außentests Schritt II

| Nr | Ausgangslage | Handlung | Erwartet | Gemessen |
|---|---|---|---|---|
| AL-15 | frische VM Ubuntu 26.04.1 | Installation; `getent passwd wb-halter`, `getent group` je Gruppe; Gegenprobe mit einem Konto `wb-halter` mit Shell `/bin/bash` | Konto und vier Gruppen wie in „Konto und Gruppen“; Gegenprobe: `preinst` Code 24, `getent` unverändert | Zeilen von `getent`, Rückgaben |
| AL-16 | VM mit Paket Schritt I und `anmeldung.copilot` (Marke statt Token) | Update auf Schritt II; danach mit einer Störung (Ziel schreibgeschützt) wiederholen | Datei unter `/var/lib/wb-halter/geheim` (0600, `wb-halter`), Heim ohne Ablage; mit Störung: Quelle unverändert, Journal Code 21 | Listen der Ablagen, Modi, Journalzeilen |
| AL-17 | VM mit Paket Schritt II und einer zweiten Partition (Loop-Gerät, eigener Einhängepunkt; Ablauf im Absatz „AL-17 im Einzelnen“) | `apt remove`, dann `apt purge`; Gegenprobe A: vor `purge` eine Datei mit der uid von `wb-halter` auf dem Wurzel-Dateisystem unter `/srv/beispiel`; Gegenprobe B: die Datei auf der zweiten Partition; Gegenprobe C: die zweite Partition mit einem Typ, der in keiner Liste steht | nach `remove` bleiben Ablage, Konto, Gruppen; nach `purge` nichts; Gegenproben A bis C: Konto und Gruppen bleiben, Journal Code 23; mit einem Bau, dessen `purge` nur `find / -xdev` nutzt, löscht B das Konto (das ist der Fehler, den der Test aufdecken muss) | Listen, `getent`, Journalzeilen, `findmnt`-Ausgabe |
| AL-18 | VM mit Paket Schritt II | `stat` der Rollenprogramme; Prozess der Person ohne Rollenprogramm verbindet den Socket | nur die Pfade der setgid-Liste tragen 2755 mit ihrer Gruppe; `connect` scheitert | `stat`, Rückgabe |

**AL-17 im Einzelnen** (Text für den Außentest; der Vertrag führt nichts aus). Die VM ist eine Wegwerf-VM; das Loop-Gerät und der Einhängepunkt entstehen nur dort.
1. Vor dem Test: `truncate -s 64M /var/tmp/beispiel-daten.img`, `mkfs.ext4 -q /var/tmp/beispiel-daten.img`, `mkdir -p /srv/beispiel/daten`, `mount -o loop /var/tmp/beispiel-daten.img /srv/beispiel/daten`. `findmnt /srv/beispiel/daten` nennt `ext4` und ein `/dev/loop*`-Gerät.
2. Grundlauf: `apt remove werkbank` (Ablage, Konto, Gruppen bleiben), dann `apt purge werkbank`. Erwartet: `getent passwd wb-halter` und `getent group` je Gruppe sind leer, das Journal hat keine Zeile mit Code 23.
3. Gegenprobe A (Wurzel): vor `purge` `touch /srv/beispiel/wurzel-rest.txt` und `chown wb-halter /srv/beispiel/wurzel-rest.txt` (eine Datei auf dem Wurzel-Dateisystem, außerhalb des Einhängepunkts). Erwartet: Konto und Gruppen bleiben, Journal Code 23.
4. Gegenprobe B (zweite Partition): vor `purge` `touch /srv/beispiel/daten/bericht.txt` und `chown wb-halter /srv/beispiel/daten/bericht.txt`. Erwartet: Konto und Gruppen bleiben, Journal Code 23, die Datei bleibt unberührt. **Diese Probe trennt die neue von der alten Regel:** Ein `purge`, das nur `find / -xdev` nutzt, findet die Datei nicht, löscht Konto und Gruppen, und die uid erbt die Datei beim nächsten Konto (Fall `entfernen-konto-mount-rest-zweite-partition-nur-wurzel-geloescht`).
5. Gegenprobe C (unbekannter Typ): die zweite Partition mit `mkfs.minix` statt `mkfs.ext4` anlegen und einhängen; kein Rest, keine Datei. Erwartet: Konto und Gruppen bleiben, Journal Code 23 (der Typ `minix` steht in keiner Liste).
6. Ein nicht lesbarer Punkt lässt sich in der VM kaum erzeugen (`root` liest jedes Dateisystem); ihn belegen nur die Fälle `mount-nicht-lesbar-*`.
7. Nach dem Test: `umount /srv/beispiel/daten`, die Image-Datei und die VM verwerfen.

## Teil A3: Linux-Paket, Schreibhelfer der Firmenvorgabe unter dem Systemnutzer `werkbank-vorgabe` (Nachtrag 27.11.2026, Entwurf)

**Entscheid des Betreibers (27.11.2026):** Eine Änderung eines Admins an der Firmenvorgabe schreibt ein kleiner Helfer mit eigenem
Systemnutzer, wie beim Halter. Er ist nicht root und startet über einen systemd-Socket. Er kann genau eine Sache: die Firmenvorgabe
atomar ganz ersetzen. Vorher prüft er über `SO_PEERCRED`, ob der Partner Admin ist, gegen die gerade gültige Datei. Der Dienst des
Nutzers reicht die Änderung nur weiter und schreibt nie selbst. Protokoll, Prüfreihenfolge und Codes legt
`rust/spec/regelebenen/README.md`, Abschnitt 15.12, fest; dieser Teil legt Konto, Einheiten, Orte, Rechte, Erstausstattung und
Entfernen fest. Entscheidung 4 aus Teil A („kein Konto, keine Gruppe“) ist damit auch für das Konto `werkbank-vorgabe` und seine
Hauptgruppe aufgehoben.

**Was gemessen ist:** nur eine Voraussetzung. Am 27.11.2026 am Mac: In einem Ordner mit Modus `0555` scheitern
`open(O_CREAT|O_EXCL)` und `rename` mit errno 13 (`EACCES`), auch für den Eigentümer der Datei. Jede andere Aussage über Kernel,
`dpkg`, `adduser` und systemd ist hergeleitet; die Annahmen stehen am Ende mit ihrer Messung.

### Entscheidungen in Teil A3

1. **Name.** Konto und Hauptgruppe heißen `werkbank-vorgabe`, ohne die alte Bedeutung der Namen `wb-*` (Team-Issue #11).
2. **Ein Systemkonto, keine Rollengruppe.** `werkbank-vorgabe` mit eigener Hauptgruppe, Heim `/nonexistent`, Shell
   `/usr/sbin/nologin`, uid im Bereich der Systemkonten. Kein Konto ist Mitglied der Gruppe. Die Admins stehen nur in der
   Kontenliste der Firmenvorgabe; eine Gruppe der Admins gäbe es als zweite Quelle, und die ist verboten (regelebenen 15.2).
3. **System-Einheiten** `werkbank-vorgabe.socket` und `werkbank-vorgabe.service`. Nur der Socket lauscht; der Helfer bindet nie
   selbst und startet erst bei der ersten Verbindung.
4. **Ordner des Helfers unter root-eigenen Eltern** (Entscheid des Betreibers, 27.11.2026; schließt OPEN 33 a). `/etc/werkbank/vorgabe/`
   gehört `werkbank-vorgabe` mit Modus `0755`. Alle Eltern ab `/etc/werkbank` gehören root und haben nicht `& 022`. Der Ordner hat
   kein Schreibrecht für eine Gruppe, und der Helfer hat kein `CAP_DAC_OVERRIDE`. Grund (gemessen, oben): In einem Ordner mit Modus
   `0555` scheitern `open(O_CREAT|O_EXCL)` und `rename` mit errno 13, auch für den Eigentümer der Datei. In einem root-eigenen
   Ordner könnte der Helfer also weder die Temp-Datei anlegen noch umbenennen.
5. **Die IT pflegt die Datei weiter mit `sudo`.** Danach setzt sie Eigentümer und Modus zurück (regelebenen OPEN 27).
6. Teil A gilt weiter, soweit dieser Teil nichts anderes sagt. Teil A3 hängt nicht an Teil A2; das deb kann ihn mit Schritt I oder
   Schritt II tragen (OPEN 33 e).
7. **Erster Admin** (Entscheid des Betreibers, 27.11.2026; schließt OPEN 33 b). `postinst` trägt das installierende Konto als ersten
   Admin ein, nur wenn noch keine Firmenvorgabe da ist. Eine vorhandene Firmenvorgabe ändert `postinst` nie. Grund: Wer `sudo` hat,
   kann die Datei ohnehin direkt ändern; der Eintrag gibt ihm keine neue Macht. Ablauf und Prüfpunkte stehen unter „Erster Admin“.

### Orte und Rechte

| Pfad | Art | Eigentümer | Modus | Wer legt an |
|---|---|---|---|---|
| `/usr/lib/werkbank/werkbank-vorgabe` | `dienst` | `root:root` | `0755` | Archiv, Quelle `bin:vorgabe:werkbank-vorgabe` |
| `/usr/lib/systemd/system/werkbank-vorgabe.socket` | `daten` | `root:root` | `0644` | Archiv, Quelle `datei:paket/ubuntu-26.04-x64/werkbank-vorgabe.socket` |
| `/usr/lib/systemd/system/werkbank-vorgabe.service` | `daten` | `root:root` | `0644` | Archiv, Quelle `datei:paket/ubuntu-26.04-x64/werkbank-vorgabe.service` |
| `/usr/share/werkbank/vorgabe/firmenvorgabe.json` | `daten` | `root:root` | `0644` | Archiv, Vorlage der Erstausstattung |
| `/etc/werkbank/` | Ordner | `root:root` | `0755` | Teil A |
| `/etc/werkbank/vorgabe/` | Ordner | `werkbank-vorgabe:werkbank-vorgabe` | `0755` | `postinst`, nie das Archiv |
| `/etc/werkbank/vorgabe/firmenvorgabe.json` | Datei | `werkbank-vorgabe:werkbank-vorgabe` | `0644` | `postinst` (Erstausstattung), danach Helfer oder IT |
| `/run/werkbank-vorgabe/` | Ordner | `root:root` | `0755` | systemd (`DirectoryMode=` der Socket-Einheit) |
| `/run/werkbank-vorgabe/vorgabe.sock` | Socket | `root:root` | `0666` | systemd |

- Jeder Eintrag des Archivs bleibt `root:root`; Stufe 9 der Gegenprobe ändert sich nicht. Ordner und Datei unter `/etc/werkbank/vorgabe/`
  stehen nicht im Archiv, denn ihr Eigentümer besteht erst nach `preinst`, und ein Update darf die Datei nie ersetzen.
- **Warum der Socket `0666` hat:** Der Dienst jedes Nutzers muss ihn erreichen (DI2: der Dienst läuft unter dem Konto des Nutzers).
  Eine Gruppe der Admins schiede aus (Entscheidung 2). Die Grenze ist die Prüfung des Partners im Helfer (regelebenen 15.12, H1 bis H5),
  nicht das Recht am Socket. Ein Nutzer, der kein Admin ist, bekommt `firmenvorgabe.partner_nicht_admin`, bevor der Helfer ein Byte
  der Anfrage liest.
- **`/etc/werkbank/vorgabe.json` ist eine andere Datei.** Sie ist die Vorgabe der Netzziele nach Vertrag 2 (`root:root`, `0644`),
  die `postinst` bei jedem `configure` atomar aus dem Erststart-Profil neu schreibt und gegen die `lauf` die Routentabelle prüft
  (Konstante `VORGABE_PFAD` im Modul `ziele::vorgabe` von `lauf`, nur root als Eigentümer). Sie bleibt root-eigen. Der Helfer schreibt nur
  `/etc/werkbank/vorgabe/firmenvorgabe.json`; `ReadWritePaths=` nennt nur seinen Ordner, also kann er `vorgabe.json` nicht ändern
  (regelebenen 15.12.4).
- Der Ordner des Dienstes wird nie in einen Einschluss gebunden (Vertrag `einschluss`); `/run/werkbank-vorgabe/` ebenso nicht. Der Helfer
  prüft trotzdem die Cgroup des Partners (H2).

### Konto und Gruppe (`preinst`)

`preinst` (`install` und `upgrade`) legt das Konto vor dem Entpacken an, wie Teil A2 („Konto und Gruppen“):

1. Besteht `werkbank-vorgabe` als Konto oder Gruppe schon, prüft `preinst` die Form: uid und gid von 100 bis 999, Hauptgruppe
   `werkbank-vorgabe`, Heim `/nonexistent`, Shell `/usr/sbin/nologin`, Gruppe ohne Mitglieder. Weicht eine ab, endet `preinst` mit dem
   **festen Code 25** und ändert nichts (fail closed). Ein fremdes Konto dieses Namens wird nie übernommen.
2. Fehlt es, legt `preinst` es an: `adduser --system --group --no-create-home --home /nonexistent --shell /usr/sbin/nologin
   werkbank-vorgabe`. Bestehendes in richtiger Form bleibt unverändert.
3. Kein anderes Konto und keine andere Gruppe ändert sich.

### Die Einheiten

`werkbank-vorgabe.socket`:

```ini
[Unit]
Description=Werkbank: Schreibhelfer der Firmenvorgabe (Socket)

[Socket]
ListenStream=/run/werkbank-vorgabe/vorgabe.sock
SocketUser=root
SocketGroup=root
SocketMode=0666
DirectoryMode=0755
Accept=no
Backlog=8
RemoveOnStop=yes

[Install]
WantedBy=sockets.target
```

`werkbank-vorgabe.service` (ohne `[Install]`; nur der Socket startet ihn):

```ini
[Unit]
Description=Werkbank: Schreibhelfer der Firmenvorgabe
Requires=werkbank-vorgabe.socket
After=werkbank-vorgabe.socket

[Service]
Type=simple
ExecStart=/usr/lib/werkbank/werkbank-vorgabe
User=werkbank-vorgabe
Group=werkbank-vorgabe
UMask=0077
Restart=on-failure
RestartPreventExitStatus=64 78
NoNewPrivileges=yes
CapabilityBoundingSet=
AmbientCapabilities=
ProtectSystem=strict
ReadWritePaths=/etc/werkbank/vorgabe
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
PrivateNetwork=yes
IPAddressDeny=any
RestrictAddressFamilies=AF_UNIX
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@privileged @resources
RemoveIPC=yes
```

- Härtung wie beim Halter (Teil A2, „Die Einheit `wb-halter.service`“) und dazu ohne Netz: Der Helfer braucht nur den geerbten
  Unix-Socket und den einen Ordner. Er startet nichts. Kein `SupplementaryGroups=`, keine Zeile `Environment`, `EnvironmentFile` oder
  `PassEnvironment`. Den Socket bekommt er über `sd_listen_fds` (Variable `LISTEN_FDS` von systemd).
- `ProtectProc=` bleibt auf der Vorgabe: Der Helfer liest `/proc/<pid>/cgroup` des Partners (H2). Mit `ProtectProc=invisible` sähe er
  fremde Prozesse nicht, und H2 lehnte jeden ab (Annahme A3-3).
- `Accept=no`: Ein Prozess nimmt eine Verbindung nach der anderen an. So laufen zwei Anfragen nie gleichzeitig (regelebenen 15.12.2).
- Exitcodes des Helfers: 0 nach SIGTERM, 64 bei einem Aufruf mit Argumenten, 78 ohne geerbten Socket oder mit mehr als einem. Jede
  Ablehnung einer Anfrage ist eine Antwort mit Code, kein Ende des Prozesses.

### Erstausstattung (`postinst configure`)

Der Schritt läuft bei jedem `configure` und ist wiederholbar. Reihenfolge:

1. **Ordner.** Fehlt `/etc/werkbank/vorgabe/`, legt `postinst` ihn mit `mkdir` (Modus `0700`) an, setzt `chown
   werkbank-vorgabe:werkbank-vorgabe` und dann `chmod 0755`. Besteht er, prüft `postinst` ihn mit `lstat`: ein Ordner, kein Verweis,
   Eigentümer `werkbank-vorgabe`, Modus `0755`. Weicht er ab, ändert `postinst` nichts und schreibt die feste Journalzeile
   `werkbank: Ordner der Firmenvorgabe in fremder Form, Code 26` (ohne Pfad). Die Installation bricht nicht ab (Entscheidung 7 aus
   Teil A); der Helfer lehnt dann mit `firmenvorgabe.datei_unsicher` ab, also fail closed.
2. **Datei mit dem ersten Admin, nur wenn sie fehlt.** Den Schritt legt der Abschnitt „Erster Admin“ fest. Er läuft nur, wenn
   Schritt 1 den Ordner in der richtigen Form hat. Code 26 aus Schritt 1 überspringt ihn.
3. **Einheiten.** `systemctl daemon-reload`, dann `systemctl enable --now werkbank-vorgabe.socket`. Der Dienst des Helfers startet
   erst bei der ersten Verbindung.

Die Vorlage im Archiv ist `{"admins": [], "leseklassen": []}` (JSON mit sortierten Schlüsseln, Einzug 1, ein Zeilenende).
`postinst` schreibt sie nie unverändert, sondern immer mit genau einem Konto in `admins`. Mit leerer `leseklassen` liest niemand
(regelebenen 15.11).

### Erster Admin (`postinst configure`, Schritt 2; Entscheid des Betreibers, 27.11.2026)

**Regel.** `postinst` legt die Firmenvorgabe nur an, wenn unter dem Namen `firmenvorgabe.json` im Ordner des Helfers kein Eintrag
besteht (`lstat` meldet `ENOENT`). Dann trägt sie genau ein Konto in `admins`: das installierende Konto. Besteht der Eintrag in
irgendeiner Form, ändert `postinst` ihn nie: weder Inhalt noch Eigentümer noch Modus. Das gilt bei der Erstinstallation, bei jedem
Upgrade, bei `dpkg-reconfigure` und für eine Datei, die die IT vor der Installation abgelegt hat. `postinst` liest und prüft den
Inhalt einer vorhandenen Datei nicht. Ist sie ungültig, bleibt sie unverändert; der Ladefehler bleibt fail closed (Dienst: kein Tor;
Helfer: `firmenvorgabe.ladefehler`, regelebenen 15.12.3, H4).

**Grund.** Wer `postinst` startet, hat root. Er kann die Datei ohnehin direkt schreiben. Der Eintrag gibt ihm also keine neue Macht.
Er erspart der IT nur den ersten Schritt von Hand. Eine vorhandene Vorgabe ist dagegen eine Entscheidung der Firma; ein Paket
überschreibt sie nie.

**Das installierende Konto** bestimmt `postinst` wie beim Halter (Abschnitt „halter.json“, Schritt 1):

1. `SUDO_USER` nennt einen Namen. `postinst` holt dazu die uid mit `id -u` und dann den Kontonamen aus `passwd` (`getent passwd
   <uid>`, erstes Feld), nie aus der Umgebung.
2. Kein installierendes Konto gibt es, wenn `SUDO_USER` fehlt oder leer ist (etwa `pkexec` oder `apt` als root ohne `sudo`), wenn das
   Konto nicht in `passwd` steht, wenn die uid `0` ist oder wenn der Name nicht die Kontoform hat, die H4 für `admins` verlangt.
   Grund für uid `0`: root braucht keinen Eintrag, um die Datei zu ändern, und ein Admin root wäre ein Konto, unter dem jeder Dienst mit
   root den Helfer ansprechen könnte (hergeleitet).

**Kein installierendes Konto.** `postinst` legt keine Datei an und schreibt die feste Journalzeile
`werkbank: Firmenvorgabe nicht angelegt, kein Konto aus sudo, Code 28` (ohne Name und Pfad). Die Installation bricht nicht ab
(Entscheidung 7 aus Teil A). Bis die IT die Datei pflegt, ist keine Änderung durch einen Admin möglich: Der Helfer lehnt mit
`firmenvorgabe.datei_unsicher` ab (H3, keine reguläre Datei). Gründe für „keine Datei“ statt einer Datei mit leerer Kontenliste:

- Eine leere Kontenliste wäre eine vorhandene Vorgabe. `postinst` änderte sie danach nie, und auch ein späteres
  `sudo dpkg-reconfigure werkbank` trüge keinen ersten Admin mehr ein. Ohne Datei holt genau dieser Aufruf den Eintrag nach.
- Eine Datei ohne Admin ist keine Entscheidung der Firma, sieht aber wie eine aus. Ohne Datei bleibt sichtbar, dass niemand
  entschieden hat (Journalzeile, Code 28).
- Beide Wege geben keine Macht: Mit leerer Kontenliste ist ebenso niemand Admin. Wie der Lader eine fehlende Firmenvorgabe
  behandelt, legt regelebenen 15.12.4 fest (Entscheid des Betreibers, 27.11.2026; schließt regelebenen OPEN 30): Ohne Datei lädt er
  keine Admins, und `setzen` lehnt jede Änderung eines Admins mit `ebenen.nicht_befugt` ab; die Regeln der Firma gelten weiter.

**Ablauf (atomar, als root).** Nur nach Schritt 1 ohne Code 26 und nur mit installierendem Konto:

1. Inhalt bilden: die Vorlage `/usr/share/werkbank/vorgabe/firmenvorgabe.json` lesen, `admins` auf genau `[<konto>]` setzen und als
   JSON mit sortierten Schlüsseln, Einzug 1 und einem Zeilenende schreiben. Fehlt die Vorlage oder ist sie nicht
   `{"admins": [], "leseklassen": []}`, entsteht keine Datei, und das Journal bekommt Code 26.
2. Den Ordner mit `O_DIRECTORY|O_NOFOLLOW` öffnen und an diesem Griff Form, Eigentümer und Modus aus Schritt 1 wiederholen (`fstat`).
   Jede weitere Handlung läuft relativ zu diesem Griff.
3. Temp-Datei mit zufälligem Namen im selben Ordner anlegen: `O_CREAT|O_EXCL|O_NOFOLLOW`, Modus `0600`. Ganz schreiben, `fsync`.
4. `fchown werkbank-vorgabe:werkbank-vorgabe`, dann `fchmod 0644`. Inhalt mit dem Inhalt aus Punkt 1 vergleichen.
5. Auf `firmenvorgabe.json` bringen, **ohne zu überschreiben**: `renameat2` mit `RENAME_NOREPLACE` oder `linkat` mit danach
   `unlinkat` der Temp-Datei. Beide scheitern mit `EEXIST`, wenn inzwischen eine Datei unter dem Namen liegt; dann bleibt diese
   unverändert (hergeleitet). Ein einfaches `rename` ist verboten, denn es ersetzte eine Datei, die die IT gerade abgelegt hat.
6. `fsync` des Ordners.

Scheitert ein Punkt, entfernt `postinst` die Temp-Datei, ändert sonst nichts und schreibt die Journalzeile mit Code 26. Danach steht
entweder keine Datei oder die vollständige neue Datei mit Eigentümer `werkbank-vorgabe:werkbank-vorgabe` und Modus `0644`, nie ein
Teil. Der Name des Kontos steht nur in der Datei, nie im Journal.

**Lage nach den Fällen.**

| Lage vor `postinst configure` | Ergebnis |
|---|---|
| Erstinstallation mit `sudo` durch ein Konto in Kontoform | Datei mit `admins` genau `[<konto>]`, `leseklassen` leer, `werkbank-vorgabe:werkbank-vorgabe`, `0644` |
| Upgrade mit vorhandener Vorgabe (auch durch ein anderes Konto mit `sudo`) | Datei byte-gleich, Eigentümer und Modus unverändert; kein neues Konto in `admins` |
| Vorab von der IT abgelegte Vorgabe | unverändert, auch wenn sie root gehört. Liegt sie in einem Ordner, den die IT angelegt hat und der root gehört, meldet Schritt 1 Code 26; der Helfer lehnt dann mit `firmenvorgabe.datei_unsicher` ab, bis die IT Eigentümer und Modus nach Abschnitt „Orte und Rechte“ setzt (regelebenen OPEN 27) |
| Kein installierendes Konto | keine Datei, Journal Code 28, `postinst` rc 0 |
| Vorhandene Datei ungültig (kein JSON, keine Kontenliste, ein Verweis) | unverändert; kein Journal von `postinst`; Dienst: Ladefehler, kein Tor; Helfer: `firmenvorgabe.ladefehler` oder bei einem Verweis `firmenvorgabe.datei_unsicher` |

### `remove` und `purge`

- **`remove`:** `prerm` hält `werkbank-vorgabe.socket` und `werkbank-vorgabe.service` an, `postrm remove` schaltet den Socket ab.
  Konto, Gruppe, Ordner und Datei bleiben. Grund: Die Firmenvorgabe ist eine Einstellung der Firma; ein Update über `remove` und
  Installation darf sie nicht verlieren.
- **`purge`:** zusätzlich zu Teil A, in dieser Reihenfolge:
  1. Socket und Dienst anhalten und abschalten; `/run/werkbank-vorgabe` entfernen, falls `RemoveOnStop` ihn nicht entfernt hat.
  2. `/etc/werkbank/vorgabe/firmenvorgabe.json` und jede andere Datei in `/etc/werkbank/vorgabe/` löschen, ohne einem Verweis zu
     folgen; dann den Ordner entfernen. `/etc/werkbank` geht nach Teil A, wenn leer.
  3. **Prüfung auf Rest nach Weg a** wie in Teil A2 („`purge` in Schritt II“, Schritt 3): `find` über jeden durchsuchten
     Einhängepunkt mit der uid und der gid von `werkbank-vorgabe`, dieselbe Ausnahmeliste, dazu `pgrep -u werkbank-vorgabe`. Ohne Rest:
     `deluser --system werkbank-vorgabe`, dann `delgroup --system werkbank-vorgabe`, falls die Gruppe noch besteht.
  4. **Mit Rest:** Konto und Gruppe bleiben, die Reste bleiben unberührt, und das Journal bekommt die feste Zeile
     `werkbank: Konto werkbank-vorgabe bleibt, Code 27`. `purge` bricht nicht ab.

### Feste Codes in Teil A3

| Code | Skript | Wirkung | Text |
|---|---|---|---|
| 25 | `preinst` | Abbruch vor dem Entpacken, nichts geändert | `werkbank: Konto werkbank-vorgabe besteht in fremder Form, Code 25` |
| 26 | `postinst` | kein Abbruch, Journalzeile | `werkbank: Ordner der Firmenvorgabe in fremder Form, Code 26` |
| 27 | `postrm purge` | kein Abbruch, Journalzeile | `werkbank: Konto werkbank-vorgabe bleibt, Code 27` |
| 28 | `postinst` | kein Abbruch, Journalzeile, keine Firmenvorgabe | `werkbank: Firmenvorgabe nicht angelegt, kein Konto aus sudo, Code 28` |

### Prüfpunkte für den Bau

`rust/paket/test_bau.py` (Klasse `Vorgabe`) prüft jeden Punkt an den Blöcken der Skripte gegen `referenz.py` (`vorgabe_konto_code`,
`vorgabe_einheit_code`, `vorgabe_erstausstattung`, `vorgabe_purge`, `vorgabe_block_code`). Fälle in `faelle/` gibt es dafür noch nicht;
V-9 bis V-11 und die echten Rechte unter root misst AL-19 bis AL-22.

**Umsetzung (Projektentscheid, 27.11.2026).** `preinst` trägt den Block `# --- vorgabe-konto begin/end`. `postinst` führt die
Erstausstattung als Schritt 2e vor dem Einschluss (Schritt 3 von Teil A) aus, im Block `# --- firmenvorgabe begin/end`. Die Schritte am
offenen Ordner laufen in `python3 -I` (Abhängigkeit `python3` aus Teil A): `os.open` mit `O_DIRECTORY|O_NOFOLLOW`, die Temp-Datei mit
`O_EXCL|O_NOFOLLOW` relativ zum Griff, `fsync`, `fchown`, `fchmod 0644`, Vergleich mit `pread`, dann `os.link` mit
`follow_symlinks=False` (`linkat` ohne Überschreiben) und `unlink` der Temp-Datei, zuletzt `fsync` des Ordners. Die Prüfung auf Rest von
`purge` steht im Block `# --- vorgabe begin/end` von `postrm`; ihre Typlisten sind `referenz.MOUNT_AUSGENOMMEN` und
`referenz.MOUNT_DURCHSUCHT`. Das Programm `werkbank-vorgabe` baut das Crate `rust/crates/vorgabe-helfer`.

| Nr | Prüfpunkt | Erwartet |
|---|---|---|
| V-1 | Konto nach `preinst` | `werkbank-vorgabe` mit uid und gid von 100 bis 999, Heim `/nonexistent`, Shell `/usr/sbin/nologin`, Gruppe ohne Mitglied |
| V-2 | fremdes Konto `werkbank-vorgabe` (etwa Shell `/bin/bash`) | `preinst` Code 25, kein Konto und keine Gruppe geändert |
| V-3 | Socket-Einheit | genau die Zeilen oben; `SocketMode=0666`, `Accept=no`; kein zweiter `Listen*`-Eintrag |
| V-4 | Dienst-Einheit | `User=werkbank-vorgabe`, kein `SupplementaryGroups=`, `ReadWritePaths=` genau `/etc/werkbank/vorgabe`, `PrivateNetwork=yes`, `RestrictAddressFamilies=AF_UNIX`, leeres `CapabilityBoundingSet=`, kein `[Install]`, kein `Environment*` |
| V-5 | Archiv | jeder Eintrag `root:root`; kein Eintrag unter `etc/werkbank/vorgabe/` |
| V-6 | Erstinstallation mit `SUDO_USER` eines Kontos in Kontoform, ohne Datei | Ordner `werkbank-vorgabe` `0755`; Datei `werkbank-vorgabe:werkbank-vorgabe` `0644`, Inhalt gleich der Referenz mit `admins` genau `[<konto>]` und leerer `leseklassen`; keine Temp-Datei im Ordner; Konto nicht im Journal |
| V-7 | Upgrade mit vorhandener Vorgabe, `SUDO_USER` eines anderen Kontos | Datei byte-gleich, Eigentümer und Modus unverändert, auch wenn sie root gehört; das andere Konto steht nicht in `admins` |
| V-12 | vorab von der IT abgelegte Vorgabe im Ordner des Helfers, danach Erstinstallation mit `SUDO_USER` | Datei byte-gleich, Eigentümer und Modus unverändert; das installierende Konto steht nicht in `admins`; kein Journal Code 28 |
| V-13 | vorab von der IT angelegter Ordner `root:root` mit Vorgabe | Ordner und Datei unverändert, Journal Code 26, `postinst` rc 0 |
| V-14 | kein installierendes Konto: `SUDO_USER` fehlt, ist leer, nennt uid `0`, ein Konto ohne Eintrag in `passwd` oder einen Namen außerhalb der Kontoform | keine Datei, Journal Code 28 ohne Name, `postinst` rc 0; ein späteres `dpkg-reconfigure` mit `SUDO_USER` legt die Datei nach V-6 an |
| V-15 | vorhandene Datei ungültig: kein JSON, `admins` fehlt, ein Verweis auf eine andere Datei | Eintrag byte-gleich und nicht verfolgt (beim Verweis auch das Ziel unverändert); kein Journal von Schritt 2 |
| V-16 | eine Datei erscheint zwischen `lstat` und Punkt 5 (eingeschleust) | die erschienene Datei bleibt byte-gleich; keine Temp-Datei; Journal Code 26 |
| V-17 | Block `# --- firmenvorgabe begin/end` von `postinst` | `test_bau.py` vergleicht ihn mit der Referenz: kein `rename` und kein `mv` ohne Schutz vor dem Überschreiben, kein Lesen von `firmenvorgabe.json`, Konto nur aus `id -u` und `getent passwd` |
| V-8 | Ordner als Verweis oder mit fremdem Eigentümer | nichts geändert, Journal Code 26, `postinst` rc 0 |
| V-9 | `remove` | Konto, Gruppe, Ordner und Datei bleiben; Socket abgeschaltet |
| V-10 | `purge` ohne Rest | Ordner, Datei, Konto und Gruppe weg |
| V-11 | `purge` mit Rest (Datei der uid auf einer zweiten Partition oder ein Prozess) | Konto und Gruppe bleiben, Journal Code 27 |

### Annahmen und die Messung dazu

| Nr | Annahme | Messung |
|---|---|---|
| A3-1 | systemd legt `/run/werkbank-vorgabe/` mit `DirectoryMode=0755` als `root:root` an und den Socket mit `0666` | AL-19 |
| A3-2 | `SO_PEERCRED` am Socket, den systemd übergibt, nennt die uid des verbindenden Prozesses, nicht die von systemd | AL-20 |
| A3-3 | Der Helfer liest `/proc/<pid>/cgroup` eines fremden Partners unter `ProtectProc=` als Vorgabe | AL-20 |
| A3-4 | `getpwnam_r` und `getpwuid_r` funktionieren unter `PrivateNetwork=yes` für lokale Konten; Konten aus LDAP ohne lokalen Zwischenspeicher fallen aus (dann fail closed: kein Admin) | AL-20 |
| A3-5 | `rename` im Ordner des Helfers gelingt unter `ProtectSystem=strict` mit `ReadWritePaths=/etc/werkbank/vorgabe` | AL-21 |

### Außentests Teil A3

| Nr | Ausgangslage | Handlung | Erwartet | Gemessen |
|---|---|---|---|---|
| AL-19 | frische VM Ubuntu 26.04.1 | Installation; `getent passwd werkbank-vorgabe`, `stat` von Ordner, Datei und Socket, `systemctl show werkbank-vorgabe.service` | Werte der Tabelle „Orte und Rechte“; der Dienst läuft erst nach der ersten Verbindung | Zeilen von `getent`, `stat`, `systemctl` |
| AL-20 | VM mit Paket und zwei Nutzern, einer davon in der Kontenliste | jeder Nutzer schickt eine gültige Anfrage; dazu ein Prozess des Admins in der Einheit eines Laufs | Admin: Antwort ohne Code; Nichtadmin: `firmenvorgabe.partner_nicht_admin`; im Lauf: `firmenvorgabe.partner_eingeschlossen`; Datei danach nur einmal geändert | Antworten, sha256 der Datei, Journalzeilen |
| AL-21 | VM mit Paket | Anfrage eines Admins, dann `stat` | Datei `werkbank-vorgabe:werkbank-vorgabe` `0644`, im Ordner keine Temp-Datei | `stat`, `ls -la` |
| AL-22 | VM mit Paket | `apt remove`, dann `apt purge`; Gegenprobe: vor `purge` eine Datei der uid unter `/srv/beispiel` | nach `remove` bleiben Konto und Datei; nach `purge` nichts; Gegenprobe: Konto bleibt, Journal Code 27 | `getent`, Listen, Journalzeilen |

## Teil B: Windows-Paket (MSI), folgt nach dem deb

### Entscheidung: MSI, gebaut mit WiX

Das Windows-Paket ist **eine MSI-Datei** (Windows Installer, `x64`, je Rechner, `ALLUSERS=1`). Es ist nicht MSIX.

Gründe, in dieser Reihenfolge:

1. **Der Halter braucht ein Dienstkonto `NT SERVICE\wb-halter`, der Filterdienst `NT SERVICE\wb-plattform`** (A116, A122). MSI kann Dienste mit virtuellem Konto eintragen. Für MSIX fand die Recherche am 20.11. nur die Konten Local Service, Network Service und Local System (hergeleitet aus der Microsoft-Seite zu `desktop6:Service`); virtuelle Dienstkonten kamen in den Quellen nicht vor. Die Lücke ist ungeprüft.
2. **Der Installer muss Rechte setzen, die ein Paketmanager nicht setzt:** fünf nicht vererbbare ACEs an der Filter-Engine (A122), die Ordnerrechte des Admin-Pfads und den Vorstart (A126). MSI hat dafür Custom Actions, die als SYSTEM laufen; MSIX hat keine.
3. **Dienste in MSIX brauchen die eingeschränkten Fähigkeiten `packagedServices` oder `localSystemServices`.** Beide gelten als eingeschränkt: Sie brauchen eine Freigabe im Store oder stehen nur Microsoft und ausgewählten Partnern offen (Quellen unten).
4. **Firmen verteilen MSI mit jeder Softwareverteilung ohne Sonderweg** (Gruppenrichtlinie, Intune, SCCM, `msiexec /i … /qn`). Das Paketformat des Betreibers ist offen; MSI ist der kleinste gemeinsame Nenner.

**Werkzeuge und Lizenzlage** (Quellenlage 20.11.2026, kein Rechtsgutachten; nur hergeleitet):

| Werkzeug | Aufgabe | Lizenz nach den Quellen | Folge |
|---|---|---|---|
| WiX Toolset **5.0.2** (angeheftet) | baut die MSI aus einer Quelldatei `.wxs` | MS-RL; keine Wartungsgebühr (Abschnitt „Entscheidung zu WiX“ mit Quellen) | die Fassung ist entschieden (Auftrag, Abschnitt unten); der Bau trägt sie in `rust/README.md`, Tabelle der Werkzeuge, ein. |
| `signtool` (Windows SDK) | Authenticode-Signatur der MSI | Teil des Windows SDK; nicht im Paket | läuft nur auf dem Windows-Bauhost |
| `cargo` (MSVC-Ziel `x86_64-pc-windows-msvc`) | baut die Programme | wie im Rust-Gerüst (`rust/README.md`) | der Bauhost ist Windows (OPEN 4) |

Die Werkzeuge liegen nie im Paket, und **keine Datei aus WiX liegt in der MSI** (Zusage Z12, AT-8). Die Lizenzprüfung der Bibliotheken im Paket bleibt `./gates.sh licenses` (`deny.toml`); die Stückliste unten kommt von dort.

### Entscheidung zu WiX (OPEN 3, entschieden 20.11.2026)

**Auftrag (Projektentscheid):** die neueste Fassung wählen, deren Lizenz und Nutzungsbedingungen zur Lizenzliste in `rust/deny.toml` passen; ausdrücklich prüfen, ob für die Fassung eine Gebühr oder eine Bedingung für die kommerzielle Nutzung der Binärdateien gilt; aus selbst geladenen Seiten, mit Adresse und Datum. Passt keine, die Alternative nennen.

**Die Liste in `rust/deny.toml`:** erlaubt sind `MIT` und `Apache-2.0` (und ihre OR-Formen), dazu `Unicode-3.0` für zwei benannte Crates. Alles andere wird abgelehnt. Das Produkt wird unter Apache-2.0 als Programm ausgeliefert (Kommentar dort).

**Selbst geladene Seiten** (abgerufen am 20.11.2026 laut `date -u`; nur gelesen, ohne Zugangsdaten):

| Adresse | Befund |
|---|---|
| `https://docs.firegiant.com/wix/osmf/` | Der Quelltext bleibt frei, aber Organisationen mit mehr als 10 000 US-Dollar Jahresumsatz „are required to sponsor the wixtoolset GitHub organization“. Die Gebühr kam mit WiX v6 auf; die Pflicht zur Annahme der EULA wird ab v7 erzwungen. |
| `https://raw.githubusercontent.com/wixtoolset/wix/v7.0.1/OSMFEULA.txt` | Die Gebühr gilt für Nutzer, die die Software in umsatzbringenden Tätigkeiten einsetzen und mindestens 10 000 US-Dollar Jahresumsatz haben. „The Fee is not a license fee.“ |
| `https://api.github.com/repos/wixtoolset/wix/releases?per_page=30` | Neueste Fassung `v7.0.1` (2026-10-26); `v7.0.0` (2026-04-06); `v6.0.2` (2025-08-28); `v6.0.0` (2025-04-08, Text der Veröffentlichung: „use of the WiX Toolset requires an Open Source Maintenance Fee“); `v5.0.2` (2024-10-05, ohne Gebührentext); `v4.0.6` (2024-10-05). |
| `https://api.nuget.org/v3-flatcontainer/wixtoolset.sdk/5.0.2/wixtoolset.sdk.nuspec` und `.../7.0.1/...` | 5.0.2: Lizenzausdruck `MS-RL`. 7.0.1: Lizenzdatei `OSMFEULA.txt`. |
| `https://raw.githubusercontent.com/wixtoolset/wix/v5.0.2/LICENSE.TXT` | Microsoft Reciprocal License (MS-RL): Dateien, die Code der Software enthalten, bleiben unter MS-RL mit Quelltext; eigene Dateien dürfen unter beliebigen Bedingungen stehen. |
| `https://crates.io/api/v1/crates/msi` | Crate `msi` 0.11.1, Lizenz MIT, veröffentlicht 2026-10-31 („Read/write Windows Installer (MSI) files“). |

**Befund:** Keine WiX-Fassung steht wörtlich auf der Liste von `deny.toml` (MS-RL ist nicht darauf; v6 und höher tragen dazu die Wartungsgebühr für umsatzbringende Nutzung der Binärdateien). Die Fassungen 6 und 7 scheiden wegen der Gebühr aus. **Entscheidung: WiX 5.0.2**, die neueste Fassung ohne Gebühr (die Gebühr beginnt laut der Seite von FireGiant mit v6). Bedingung, damit MS-RL das Produkt nicht erreicht: WiX ist **nur Bauwerkzeug** (liegt nie im Paket), und die MSI enthält **keine** aus WiX stammende Datei, auch keine Custom-Action-Bibliothek einer WiX-Erweiterung (nur die eigenen Custom Actions, Zusage Z12). Dass `deny.toml` Abhängigkeiten des ausgelieferten Programms regelt und kein Bauwerkzeug, ist unsere Lesart; sie braucht die Bestätigung des Betreibers (OPEN 3).

**Alternative, wenn auch Bauwerkzeuge der Liste folgen müssen:** ein eigener MSI-Schreiber auf dem Crate `msi` (MIT); der Aufwand ist nicht geschätzt (hergeleitet: Dienst-, ACL- und Custom-Action-Tabellen, Kabinettdateien und Signatur wären eigene Arbeit). `wixl` aus `msitools` wurde nicht beurteilt: Die Lizenzdatei war unter der geprüften Adresse nicht zu laden.

### Paketbau (Windows)

#### Der Befehl

```
rust/paket.sh windows [--version <a.b.c>] [--ausgabe <ordner>] [--unsigniert]
```

Ein Befehl baut aus dem Rust-Workspace das Windows-Paket. Er läuft auf dem Windows-Bauhost (Git Bash oder WSL ist dort keine Voraussetzung des **Produkts**, wohl aber des Bauskripts; OPEN 4). Er arbeitet nur unter `rust/` und schreibt nur nach `--ausgabe` (Vorgabe: der Ordner `paket-ausgabe` unter `rust/`, von git ignoriert).

Schritte in fester Reihenfolge:

| Schritt | Handlung | bei Fehler |
|---|---|---|
| 1 | Werkzeug-Ordner, Toolchain und WiX in der festgelegten Fassung prüfen (wie `gates.sh tools`) | Exit 127 |
| 2 | `cargo build --release --locked --target x86_64-pc-windows-msvc` für die Programme der Positivliste; **ohne** das Merkmal `attrappe` und ohne ein Merkmal mit dem Anfang `test` | Exit 70 |
| 3 | `./gates.sh licenses` laufen lassen und die SBOM in den Baum legen (`lizenzen/sbom.cdx.json`, aus den CycloneDX-Dateien je Crate zu einer zusammengeführt) | Exit 65 oder der Code des Tors |
| 4 | Baum stufen: genau die Dateien der Positivliste an ihren Zielen | Exit 65, `paket.quelle_fehlt` |
| 5 | **Gegenprobe des Lieferumfangs** (Abschnitt „Positivliste und Ausschlussliste“) | Exit 65, ein `paket.*`-Code |
| 6 | `manifest.json` schreiben (Abschnitt „Manifest“) und nach diesem Vertrag prüfen | Exit 65, ein `paket.*`-Code |
| 7 | MSI mit WiX bauen: Dateien des Baums, Dienste, Custom Actions, `manifest.json` als letzte Datei | Exit 70 |
| 8 | ohne `--unsigniert`: MSI mit `signtool` signieren; der Schlüssel liegt nie im Repository und nie im Bauablauf des Produkts, der Pfad steht in der eigenen Konfiguration des Bauhosts | Exit 70 |

Ergebnis in `--ausgabe`: `wb-<version>-windows-x64.msi` und `wb-<version>-windows-x64.manifest.json` (Kopie des Manifests aus der MSI, damit die IT es vor der Installation lesen kann). Ohne `--unsigniert` gilt die MSI als Auslieferung. Mit `--unsigniert` ist sie ein Entwicklungsbau: Der Dateiname endet auf `-unsigniert.msi`, und sie ist **nie** ein Auslieferungsstand.

Bei einem Fehler der Schritte 3 bis 6 schreibt der Befehl auf stderr eine Zeile `<code>: <grund>` (ASCII, höchstens 200 Byte) und danach bis zu 20 Zeilen, die mit zwei Leerzeichen beginnen und je einen Pfad nennen. Bei Erfolg schreibt er auf stdout den Pfad der MSI.

Exitcodes: `0` gebaut und geprüft; `64` Aufruf falsch; `65` Lieferumfang, Stückliste oder Manifest verletzt den Vertrag; `70` Fehler im Bau; `127` Werkzeug fehlt oder hat eine andere Fassung als festgelegt.

#### Installation und Orte

Die Installation läuft mit **einem Befehl**, ohne Eingabe, ohne Netz und ohne WSL, Python und Node (A60, A69, A72):

```
msiexec /i wb-<version>-windows-x64.msi /qn /l*v wb-install.log
```

`msiexec` liefert nur die Codes des Windows Installers (0, 3010, 1603 und weitere). **Einen eigenen Code „Stelle einrichten“ kann `msiexec` nicht liefern** (Entscheidung unten, A108). Als letzte Handlung ruft die MSI `wb pruefen` und schreibt dessen Zeilen in das Installationsprotokoll; das Ergebnis ändert den Rückgabewert von `msiexec` nicht. Die IT liest danach den Prüfbefehl selbst.

Orte nach A73 (Annahme, bis die Bausteine sie bestätigen):

| Ort | Inhalt | Schreiben dürfen |
|---|---|---|
| `%ProgramFiles%\Werkbank\` | Programme, `manifest.json`, `lizenzen\` | nur Administratoren und SYSTEM; normale Nutzer lesen und führen aus |
| `%ProgramData%\Werkbank\admin\` | Firmenvorgabe, darin die Einstellungsdatei nach Vertrag 2 | nur Administratoren und SYSTEM; normale Nutzer lesen |
| `%ProgramData%\Werkbank\halter\` | Datei des Halters (A116) | nur `NT SERVICE\wb-halter`, lesen nur dieses Konto und SYSTEM |
| `%LOCALAPPDATA%\Werkbank\` | Nutzerdaten im Profil | der Nutzer |

Der Ort und die Sollrechte des Vault-Ordners stehen nicht in diesem Vertrag. Der Prüfbefehl nimmt sie aus derselben Konfiguration des Produkts wie das Programm „Lauf starten“ (Vertrag 1, Abschnitt „Aufruf“); das Projekt bestätigt sie (Annahme 4).

Weitere Zusagen der MSI: Sie setzt die Dienste `wb-halter` und `wb-plattform` unter ihren virtuellen Konten; sie setzt die fünf ACEs der Filter-Engine für `NT SERVICE\wb-plattform` nach A122 mit einer Custom Action (`wb-plattform.exe --einrichten`, idempotent, als SYSTEM; Annahme 1); sie startet jedes Programm der Positivliste einmal vor, bevor ein Nutzer wartet (A126); sie liest nichts aus dem Netz. Die Deinstallation entfernt Programme, Dienste und die Filtergruppe, aber nicht `%LOCALAPPDATA%\Werkbank\` und nicht die Sperre je Container (A123: Abräumen nur beim Löschen eines Projekts). Was mit `%ProgramData%\Werkbank\` geschieht, ist offen (OPEN 8).

## Teil C: macOS-Paket (pkg)

Der Abschnitt hält den deb-Teil nicht auf; er beschreibt das macOS-Paket nur so weit, wie es das gleiche Manifest und den gleichen Prüfbefehl braucht.

### Entscheidungen

1. **Ein pkg**, gebaut mit `pkgbuild` und `productbuild` (Bordmittel von macOS, in Xcode oder den Command Line Tools). Der Vorschlag stammt vom Projekt. Die Vorlage ist der alte Weg `the previous implementation` und `the previous implementation` (nur gelesen): Inhalt `root:wheel`, für Gruppe und Welt nicht schreibbar, nicht verschiebbar (`BundleIsRelocatable` falsch), Prüfsumme im Release-Text.
2. **Das pkg bleibt unsigniert und wird nicht notarisiert; es gibt kein Developer-Konto** (Projektentscheid). Die Sicherung ist wie beim deb die Prüfsumme (sha256) über einen zweiten Weg; die Anleitung nennt den Warndialog (Abschnitt „Der Warndialog“). Der frühere Weg lief ebenfalls ohne Notarisierung. Der Befehl `sudo installer -pkg <datei> -target /` im Terminal ist der Weg ohne Netz (hergeleitet, nicht gemessen; AM-6 misst es).
3. **Plattformname `macos-arm64`** im Manifest (Apple-Silicon), Pfade relativ zu `/`. Pfade dürfen hier Großbuchstaben tragen, weil `/Library` so heißt; verglichen wird ohne Rücksicht auf die Schreibung.
4. **Gleiches Manifest, gleicher Prüfbefehl.** Der Prüfbefehl heißt `/Library/Werkbank/bin/werkbank-pruefen` und liegt über `/etc/paths.d/werkbank` im Pfad (wie im alten Weg).
5. **Ein Einrichtungsbefehl wie beim deb:** `werkbank-einrichten` fragt nach Adresse und Schlüssel (ohne Echo, nie in argv). Der Schlüssel geht in den Schlüsselbund, nicht in eine Datei. Es gelten die Stufen 1 und 2 der Tabelle „Einrichten“ im Teil A (der Schlüssel erscheint nirgends, kein Echo); die Stufen 3 bis 5 betreffen die Datei und entfallen. `security add-generic-password` bekommt den Wert nie mit `-w <wert>` auf der Befehlszeile (hergeleitet: ohne Wert fragt es am Terminal).

### Der Befehl zum Bauen

```
rust/paket.sh macos [--version <a.b.c>] [--ausgabe <ordner>]
```

Schritte wie beim deb, mit diesen Unterschieden: Bauziel `aarch64-apple-darwin`; die Gegenprobe des Lieferumfangs und das Manifest wie dort; dann `pkgbuild --root <baum> --identifier werkbank.paket --version <a.b.c> --install-location /` und `productbuild`, **ohne Signatur**. Ergebnis: `werkbank-<version>-macos-arm64.pkg`, `.sha256` und die Kopie des Manifests. Exitcodes wie beim deb.

### Der Warndialog (das pkg ist nicht signiert und nicht notarisiert)

Öffnet der Nutzer das pkg per Doppelklick, nachdem es über ein Netz oder einen Browser auf den Mac kam (Quarantäne-Attribut), zeigt macOS eine Warnung. Der Wortlaut hängt von der macOS-Fassung ab; sinngemäß sagt sie, Apple könne nicht prüfen, ob das Paket frei von Schadsoftware ist (hergeleitet, nicht gemessen). Der Text der Anleitung ist fest und steht so in der Anleitung des Probelaufs:

> Beim ersten Öffnen zeigt macOS eine Warnung, dass das Paket nicht von Apple geprüft ist. Das ist hier erwartet: Das Paket ist weder signiert noch notarisiert. Vergleiche zuerst die Prüfsumme (Schritt 1). Ist sie gleich, gibt es zwei Wege. Erstens im Terminal: `sudo installer -pkg werkbank-<version>-macos-arm64.pkg -target /`. Zweitens per Doppelklick: die Warnung schließen, die Systemeinstellungen öffnen, „Datenschutz & Sicherheit“ wählen, unten bei der Meldung zum Paket „Trotzdem öffnen“ wählen und das Administratorkennwort eingeben. Ist die Prüfsumme nicht gleich, installiere nichts.

### Bestandteile

Alle Dateien gehören `root:wheel`; Programme 0755, übrige Dateien 0644; für Gruppe und Welt nicht schreibbar.

| Teil | Datei und Ort | Dienst | Lieferant |
|---|---|---|---|
| Halter | `/Library/Werkbank/lib/werkbank-halter` | launchd-Dienst `werkbank.halter` (`/Library/LaunchDaemons/werkbank.halter.plist`, unter eigenem Konto) | Plattformschicht |
| „Lauf starten“ mit Einschluss | `/Library/Werkbank/lib/werkbank-lauf` | keine | Vertrag 1, Einschluss laut macOS-Einschluss-Vertrag |
| Einrichtung | `/Library/Werkbank/bin/werkbank-einrichten` | keine | Paket |
| Prüfbefehl | `/Library/Werkbank/bin/werkbank-pruefen` | keine | Paket |
| Stückliste, Manifest | `/Library/Werkbank/sbom.cdx.json`, `/Library/Werkbank/manifest.json` | – | Paket |

Alles Weitere (Suche, Tor, Kette, Vorführ-Vault, Einstellungen, Läufer) folgt dem Teil A, wenn das macOS-Paket diese Teile mitbringt; die Liste legt das Projekt mit den Bausteinen fest (OPEN 22).

### Prüfbefehl unter macOS (sechs Prüfungen)

| Prüfung | Zustände | Was geprüft wird |
|---|---|---|
| `launchd` | `ok`, `fehlt`, `gestoppt`, `konto`, `nicht_pruefbar` | Der Dienst `werkbank.halter` ist eingetragen, läuft und läuft unter seinem eigenen Konto |
| `schluesselbund` | `ok`, `fehlt`, `nicht_pruefbar` | Der Schlüsselbund-Eintrag des Modellwegs ist **vorhanden, ohne dass sein Inhalt gelesen oder ausgegeben wird** (nie `-w`, nie `-g`) |
| `einschluss` | `ok`, `fehlt`, `probelauf`, `nicht_pruefbar` | Der Einschluss ist verfügbar (Vertrag) und ein Probelauf gelingt |
| `einstellungen`, `vault`, `paketstand` | wie unter Windows | wie unter Windows; `paketstand` liest `/Library/Werkbank/manifest.json` |

Ausgabe, Reihenfolge und Exitcodes wie im Abschnitt „Prüfbefehl“; die Sätze der drei neuen Prüfungen stehen dort.

### Entfernen

macOS kennt kein `apt purge`. Das Paket bringt `/Library/Werkbank/bin/werkbank-entfernen` mit (entfernt Programme, Dienst, Daten und den Schlüsselbund-Eintrag; danach `pkgutil --forget werkbank.paket`). Der Fall `entfernen-*` aus dem Teil A gilt unverändert für die Listen vorher und nachher (OPEN 23).

### Außentests (macOS)

| Nr | Ausgangslage | Handlung | Erwartet |
|---|---|---|---|
| AM-1 | frischer Mac (Apple-Silicon), kein Netz | Prüfsumme vergleichen, `sudo installer -pkg … -target /`, `werkbank-einrichten` | Rückgabe 0; Installation und Einrichtung höchstens 15 Minuten; Eingaben außer Adresse und Schlüssel 0 |
| AM-2 | installiertes Paket | je Zustand der Tabelle erzeugen, `werkbank-pruefen` | Zeilen und rc wie in den Fällen `pruefung-macos-*` |
| AM-3 | Schlüssel mit der Marke eingegeben | Marke in Ausgaben, `ps`, Protokollen, Ergebnisdatei und im Journal (`log show`) suchen | Treffer 0; Eintrag im Schlüsselbund vorhanden |
| AM-4 | installiertes Paket | `werkbank-entfernen`; Listen vorher und nachher vergleichen | Rückstände 0 außer den genannten Ausnahmen |
| AM-6 | pkg mit Quarantäne-Attribut (per Browser oder Netz kopiert) | Doppelklick; danach den Weg aus der Anleitung („Trotzdem öffnen“); dazu `sudo installer -pkg … -target /` auf einer zweiten Kopie | die Warnung erscheint und der Text der Anleitung passt zu ihr; beide Wege installieren; der Wortlaut der Warnung wird festgehalten |
| AM-5 | gebautes pkg (jeder Mac) | `pkgutil --expand-full`; Dateimenge mit Manifest und Positivliste vergleichen | Zähler abweichender Dateien: 0 |

Der Mac für die Messung ist offen; das Projekt legt ihn fest. Der Vertrag startet nichts.

## Gemeinsam: Manifest, Lieferumfang und Stückliste

### Manifest

`manifest.json` liegt im Produktordner und in der MSI als letzte Datei. Es steht nie in seiner eigenen Liste. Ein JSON-Objekt, UTF-8 ohne BOM, höchstens 1 048 576 Byte, geschlossene Form auf jeder Ebene, kein doppelter Schlüssel in irgendeinem Objekt (A139; doppelt zählt nach dem Dekodieren: `"bau"` und `"bau"` sind derselbe Schlüssel). Das Manifest enthält keinen Zeitstempel und keinen Pfad des Bauhosts.

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | genau `1`; `1.0` ist keine Ganzzahl | Fassung dieses Vertrags |
| `paket` | Text | genau `werkbank` (Arbeitsname) | Name des Produkts |
| `version` | Text | `a.b.c`, ohne führende Null, `a` und `b` höchstens 255, `c` höchstens 65535 | Version des Pakets; sie ist die `ProductVersion` der MSI (Grenzen des Windows Installers); eine Vorabversion gibt es nicht |
| `plattform` | Text | `ubuntu-26.04-x64`, `windows-x64` oder `macos-arm64` | das Betriebssystem des Pakets; die Reihenfolge des Baus ist deb, MSI, pkg |
| `bau` | Text | `auslieferung` oder `test` | ein Testbau ist nie eine Auslieferung |
| `merkmale` | Liste von Kennungen | 0 bis 16, ohne Doppel | die Cargo-Merkmale des Baus; im Auslieferungsbau ist `attrappe` und jedes Merkmal mit dem Anfang `test` verboten |
| `dateien` | Liste von Objekten | 1 bis 2000, Pfade ohne Doppel (ohne Rücksicht auf die Schreibung) | jede Datei des Pakets außer dem Manifest selbst |
| `bibliotheken` | Ganzzahl | 1 bis 100 000 | Zahl der Bibliotheken in `lizenzen/sbom.cdx.json` |

**Datei:** `pfad` (Text), `groesse` (Ganzzahl 0 bis 2 147 483 647), `sha256` (64 Zeichen `[0-9a-f]`), `art` (`programm`, `dienst`, `daten` oder `lizenz`).

**Pfad:** relativ zur Installationswurzel, Trenner `/`, höchstens 200 Zeichen. Wurzel unter Windows ist der Produktordner, unter Ubuntu und macOS `/`. Zeichenvorrat je Segment `[a-z0-9._-]` (Kleinbuchstaben); unter macOS auch Großbuchstaben, weil `/Library` so heißt. Kein Segment ist `.` oder `..`, kein Segment endet mit `.`, kein Segment beginnt mit einem reservierten Windows-Namen (`con`, `prn`, `aux`, `nul`, `com1` bis `com9`, `lpt1` bis `lpt9`, mit oder ohne Endung). Kein Pfad ist der Ort des Manifests selbst: Windows `manifest.json`, Ubuntu `usr/share/werkbank/manifest.json`, macOS `Library/Werkbank/manifest.json`.

**Pflichtdateien** (Annahme 1: die Namen legen die Bausteine fest; eine Pflichtdatei mit anderer `art` gilt als fehlend):

- `windows-x64`: `bin/wb.exe` (`programm`), `bin/wb-halter.exe` und `bin/wb-plattform.exe` (`dienst`), `lizenzen/sbom.cdx.json` (`daten`).
- `ubuntu-26.04-x64`: `usr/bin/werkbank-probelauf`, `usr/bin/werkbank-pruefen`, `usr/lib/werkbank/werkbank-lauf`, `usr/lib/werkbank/werkbank-lauf-kind` (Nachtrag 21.11.2026: das Trampolin; ohne es startet `werkbank-lauf` kein Kind; Fälle `manifest-pflichtdatei-linux-lauf-kind*`), `usr/lib/werkbank/rangkern`, `usr/lib/werkbank/kette`, `usr/lib/werkbank/such-mcp`, `usr/lib/werkbank/probelauf/drehbuch.py` und die 13 Module `usr/lib/werkbank/probelauf/drehbuchlib/<modul>.py` (alle `programm`; die Module nennt die Zeile „Bibliothek des Läufers“ der Bestandteile), `usr/lib/werkbank/werkbank-kern`, `usr/lib/werkbank/werkbank-bruecke` (Nachtrag 21.11.2026, Fall `manifest-pflichtdatei-linux-bruecke`), `usr/lib/werkbank/werkbank-modellweg` und `usr/lib/werkbank/werkbank-halter` (Nachtrag Halter, Fall `manifest-pflichtdatei-linux-halter`) (`dienst`), dazu als `daten` die Vorlage `usr/share/werkbank/probelauf/einstellungen.json`, `usr/share/werkbank/probelauf/konfig.json`, `usr/lib/systemd/user/werkbank-kern.service`, `usr/lib/systemd/user/werkbank-bruecke.service` (Fall `manifest-pflichtdatei-linux-einheit-bruecke`), `usr/lib/systemd/user/wb-halter.service` (Nachtrag Halter, Fall `manifest-pflichtdatei-linux-einheit-halter`) und `usr/share/werkbank/sbom.cdx.json`. `/etc/werkbank/halter.json` ist keine Pflichtdatei; `postinst` schreibt sie. `usr/share/werkbank/probelauf/erststart.json` (Nachtrag Erststart) steht in der Positivliste, ist aber noch keine Pflichtdatei des Manifests (OPEN 32). Die Einheit `werkbank-modellweg.service` ist **keine** Pflichtdatei und nicht im Paket. **Zusätzlich** mindestens eine vollständige Profilform: A `etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss` (`daten`) **und** der Starter `usr/lib/werkbank/werkbank-einschluss` (`programm`) oder B `etc/apparmor.d/usr.lib.werkbank.bwrap` (je `daten`); das alte `etc/apparmor.d/werkbank-einschluss-bwrap` zählt nicht (Fall `manifest-pflichtdatei-linux-profil-altname`), ebenso nicht das Profil am Pfad von `werkbank-lauf` (Fall `manifest-pflichtdatei-linux-profil-altname-lauf`); fehlt der Starter, fehlt Form A (Fall `manifest-pflichtdatei-linux-starter`).
- `macos-arm64`: `Library/Werkbank/bin/werkbank-einrichten`, `Library/Werkbank/bin/werkbank-pruefen`, `Library/Werkbank/lib/werkbank-lauf` (`programm`), `Library/Werkbank/lib/werkbank-halter` (`dienst`), `Library/LaunchDaemons/werkbank.halter.plist` und `Library/Werkbank/sbom.cdx.json` (`daten`).

**Fehlercodes und Prüfreihenfolge.** Die Prüfung meldet genau einen Code: den der ersten verletzten Stufe. Die Codes und Gründe sind feste Texte (ASCII).

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.zu_gross` | mehr als 1 048 576 Byte | `Manifest zu gross` |
| 2 | `paket.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, oberste Ebene kein Objekt, Text nach dem Objekt | `kein gueltiges JSON` |
| 3 | `paket.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auf jeder Ebene | `unbekannter Schluessel` |
| 4 | `paket.schluessel_fehlt` | ein Pflichtschlüssel fehlt, auf jeder Ebene | `Pflichtfeld fehlt` |
| 5 | `paket.typ_falsch` | falscher JSON-Typ (etwa `fassung` als Text oder `1.0`) | `falscher Typ` |
| 6 | `paket.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber nicht `1` | `Fassung unbekannt` |
| 7 | `paket.anzahl` | Liste außerhalb ihrer Grenzen, oder `bibliotheken` außerhalb 1 bis 100 000 | `Anzahl ausserhalb der Grenzen` |
| 8 | `paket.version_form` | Form oder Zahlenbereich der Version verletzt | `Version hat falsche Form` |
| 9 | `paket.wert_unbekannt` | `paket`, `plattform`, `bau` oder `art` hat keinen erlaubten Wert | `Wert unbekannt` |
| 10 | `paket.kennung_form` | ein Merkmal verletzt die Kennungsform `^[a-z0-9][a-z0-9._-]{0,63}$` | `Kennung hat falsche Form` |
| 11 | `paket.pfad_form` | ein Pfad verletzt die Pfadregeln (auch der Ort des Manifests selbst) | `Pfad hat falsche Form` |
| 12 | `paket.pruefsumme_form` | `sha256` verletzt die Form | `Pruefsumme hat falsche Form` |
| 13 | `paket.groesse_bereich` | `groesse` außerhalb 0 bis 2 147 483 647 | `Groesse ausserhalb des Bereichs` |
| 14 | `paket.pfad_doppelt` | derselbe Pfad zweimal, ohne Rücksicht auf die Schreibung | `Pfad doppelt` |
| 15 | `paket.merkmal_doppelt` | dasselbe Merkmal zweimal | `Merkmal doppelt` |
| 16 | `paket.merkmal_verboten` | `bau` ist `auslieferung` und ein Merkmal ist `attrappe` oder beginnt mit `test` | `Merkmal im Auslieferungsbau verboten` |
| 17 | `paket.pflichtdatei_fehlt` | eine Pflichtdatei fehlt oder hat eine andere `art` | `Pflichtdatei fehlt` |

Das Schema erkennt die Stufen 3 bis 13 nur zum Teil: nicht den Zahlenbereich der Version, nicht die Segmentregeln der Pfade (Kleinschrift außerhalb von macOS, `..`, Punkt am Ende, reservierte Namen, Ort des Manifests) und nicht `1.0` gegen `1` (JSON Schema hält beide für gleich). Die Stufen 14 bis 17 liegen jenseits des Schemas.

### Positivliste und Ausschlussliste (Lieferumfang)

Was im Paket liegt, steht **namentlich** in der Positivliste. Was nie hineinkommt, steht in der Ausschlussliste. Beide liegen je Plattform unter `rust/paket/<plattform>/` (`lieferumfang.txt`, `ausschluss.txt`); die Ausschlussliste gehört zur Plattform, weil das deb den Läufer des Probelaufs (ein Python-Skript) ausliefert und deshalb kein `*.py` ausschließt, aber `*.pyc`. Das Vorbild ist der alte Weg (`the previous implementation`, `the previous implementation`); die Form ist neu, weil die Quellen Cargo-Programme sind.

**Positivliste.** Eine Zeile je Datei: `<art> <ziel> <quelle>`, getrennt durch Leerzeichen; Leerzeilen und Zeilen mit `#` am Anfang sind Kommentar. `art` wie im Manifest. `ziel` folgt den Pfadregeln des Manifests. `quelle` hat eine von drei Formen:

- `bin:<crate>:<name>`: das Programm `<name>` des Crates `<crate>` aus dem Bau.
- `datei:<pfad>`: eine Datei des Arbeitsbaums, Pfad relativ zu `rust/`, Zeichen `[A-Za-z0-9._-]`, kein Segment `.` oder `..`.
- `erzeugt:sbom`: die zusammengeführte Stückliste; nur mit dem Ziel `lizenzen/sbom.cdx.json` (Windows) oder `usr/share/werkbank/sbom.cdx.json` (Ubuntu).

**Ausschlussliste.** Eine Zeile je Muster: `<muster>  # <grund>` (zwei Leerzeichen, `#`, Leerzeichen, Grund mit mindestens 12 Zeichen). Das Muster hat eine von vier Formen: ein genauer Pfad, `*.<endung>` (jeder Dateiname mit dieser Endung, in jedem Ordner), `<ordner>/` (alles darunter) oder `crate:<name>` (kein Programm dieses Crates). Verglichen wird in Kleinschrift. Die Anfangsliste (Vorschlag, der Bau ergänzt): `*.pdb`, `*.py`, `*.sh`, `*.js`, `*.mjs`, `*.rs`, `*.rlib`, `*.toml`, `tests/`, die Attrappe, jedes Crate, das nur Prüfzwecken dient, und die Dateien der Bauwerkzeuge (Abschnitt „Bauwerkzeuge“). Grund: Kein Testwerkzeug, keine Attrappe, kein Entwicklerwerkzeug und kein Skript im Paket (A60: kein Python, kein Node).

**Das Paket `drehbuchlib` (Nachtrag 20.11.2026).** Das deb liefert den Läufer als `drehbuch.py` **und** das Paket `drehbuchlib/` aus, das es importiert. Jede Datei steht namentlich in der Positivliste, je Modul eine Zeile `programm usr/lib/werkbank/probelauf/drehbuchlib/<modul>.py datei:spec/vorfuehrung/laeufer/drehbuchlib/<modul>.py`; fehlt eine Zeile, entsteht `paket.ungelistet`, fehlt eine Datei im Baum `paket.fehlt_im_baum`. Das deb-Muster `*.pyc` der Ausschlussliste trifft `__pycache__` unter `drehbuchlib/`. Dieselben Dateien sind Pflichtdateien des Manifests (Abschnitt „Manifest“). Fälle: `liste-gueltig-linux-drehbuchlib`, `liste-drehbuchlib-*`, `manifest-pflichtdatei-linux-drehbuchlib*`.

**Der Läufer im Paket (Nachtrag 20.11.2026, Trockenlauf).** Das deb liefert vom Läufer nur einzelne Dateien aus: `werkbank-probelauf`, `drehbuch.py`, die 13 Module von `drehbuchlib/`, die Konfiguration `konfig.json` und die 21 Dateien der Vorführ-Vorlage des Vaults. Quellen: `usr/bin/werkbank-probelauf` aus `datei:spec/vorfuehrung/laeufer/werkbank-probelauf`, `usr/share/werkbank/probelauf/konfig.json` aus `datei:spec/vorfuehrung/laeufer/konfig.probelauf.json`, jede Vault-Datei `usr/share/werkbank/probelauf/vorfuehr-vault/<pfad in Kleinschrift>` aus `datei:spec/such-socket/vorfuehr-vault/<pfad>`. Nicht im Paket stehen `attrappen/`, `browser/`, `mutationsprobe.py` und der Test `test_werkbank_probelauf.py` des Läufers. Das ist keine neue Regel der Gegenprobe: Sie stehen in keiner Positivliste, ein Eintrag im Baum ergäbe `paket.ungelistet`. Die Ausschlussliste des deb führt sie zusätzlich als genaue Muster (`usr/lib/werkbank/probelauf/attrappen/`, `usr/lib/werkbank/probelauf/browser/`, `usr/lib/werkbank/probelauf/mutationsprobe.py`, `usr/lib/werkbank/probelauf/test_werkbank_probelauf.py`, `usr/bin/test_werkbank_probelauf.py`), dann ergibt ein Eintrag im Baum `paket.ausschluss_getroffen` (Stufe 6 vor 7). Fälle: `liste-gueltig-linux-laeufer`, `liste-laeufer-*`. Die älteren Fälle der Liste (`liste-gueltig-linux` und andere) behalten ihre Quelle `datei:paket/deb/werkbank-probelauf`; die Gegenprobe prüft nur, dass jede gelistete Quelle da ist, keinen Ort der Quelle von `werkbank-probelauf`. Hergeleitet, nicht gemessen: Im Arbeitsbaum fehlt `spec/vorfuehrung/laeufer/werkbank-probelauf` heute; solange das Projekt sie nicht liefert, ergibt die echte Liste `paket.quelle_fehlt`.

**Die Gegenprobe** (Schritt 5 des Befehls und Tor `./gates.sh paket`) vergleicht die Listen mit dem gestuften Baum. Die Dateien des Baums werden in Kleinschrift verglichen.

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.liste_form` | eine Zeile der Positivliste verletzt die Form | `Positivliste hat falsche Form` |
| 2 | `paket.ausschluss_form` | eine Zeile der Ausschlussliste verletzt die Form, oder der Grund hat weniger als 12 Zeichen | `Ausschlussliste hat falsche Form` |
| 3 | `paket.liste_doppelt` | dasselbe Ziel zweimal | `Ziel doppelt` |
| 4 | `paket.quelle_fehlt` | eine gelistete Quelle gibt es im Arbeitsbaum nicht | `Quelle fehlt` |
| 5 | `paket.vorfuehrung_einstellung` | das Ziel `usr/share/werkbank/probelauf/einstellungen.json` hat nicht die Quelle `datei:spec/einstellungen/vorfuehrung.json` (die Vorlage ist genau die Vorführdatei von Vertrag 2; Annahme 9) | `Einstellungsvorlage hat falsche Quelle` |
| 6 | `paket.ausschluss_getroffen` | ein gelistetes Ziel, ein gelistetes Programm oder eine Datei im Baum trifft die Ausschlussliste | `Ausgeschlossene Datei im Paket` |
| 7 | `paket.ungelistet` | eine Datei im Baum steht nicht in der Positivliste | `Datei ohne Eintrag` |
| 8 | `paket.fehlt_im_baum` | ein gelistetes Ziel fehlt im Baum | `Gelistete Datei fehlt` |
| 9 | `paket.modus_schreibbar` | ein Eintrag des gestuften Baums (Ordner, Programm oder Datei) ist für die Gruppe oder die Welt schreibbar (`Modus & 0o022 != 0`) | `Eintrag ist fuer Gruppe oder Welt schreibbar` |

Eine ausgeschlossene, nicht gelistete Datei im Baum ergibt `paket.ausschluss_getroffen`, nicht `paket.ungelistet`: Stufe 6 kommt vor Stufe 7.

**Modi im gestuften Baum (Stufe 9; Nachtrag 20.11.2026, an earlier test run).** Der Bau setzt jeden Modus selbst und unabhängig von der `umask` des Bauenden: Ordner 0755, Programme und Dienste (Art `programm`, `dienst`) 0755, übrige Dateien 0644, alle `root:root` (der Bau-Befehl `rust/paket.sh` setzt `umask 022`, und `rust/paket/bau.py` setzt danach den Modus jedes Eintrags der Stufe, auch des Wurzelordners und von `DEBIAN/`). Stufe 9 ist die Gegenprobe darauf: Sie liest die Modi des gestuften Baums und lehnt jeden Eintrag ab, der für die Gruppe oder die Welt schreibbar ist. Ein Bau unter `umask 002` ergibt dieselben Modi wie unter `umask 022` (Test in `rust/paket/test_bau.py`, Messung in der VM). Im Fall der Form `liste` steht der Baum der Modi im optionalen Feld `modi` (Pfad, Ordner mit `/` am Ende, Modus als Oktaltext); ohne das Feld entfällt die Stufe. Fälle: `liste-modus-*`. `manifest.json` im Baum (an dem Ort jeder Plattform) ist die einzige Datei, die nicht in der Positivliste steht. Fehlt ein Ziel, entsteht kein Teilpaket.

**Die Attrappe** (aus) gibt es nur in Testbauten. Das erzwingt der Vertrag dreifach: Der Bau schaltet das Merkmal `attrappe` im Auslieferungsbau nicht ein (Schritt 2); das Manifest eines Auslieferungsbaus mit diesem Merkmal ist ungültig (Stufe 16); und die Datei der Attrappe steht in der Ausschlussliste (Stufe 5). Die Annahme: Das Merkmal heißt `attrappe` und die Datei `bin/wb-attrappe.exe`; der Baustein Einschluss bestätigt die Namen (Annahme 5).

### Stückliste (A75)

`wb lizenzen` schreibt die Stückliste aus `lizenzen/sbom.cdx.json`. Eine Zeile je Bibliothek: `<name>TAB<version>TAB<lizenz>TAB<herkunft>LF`. ASCII, LF, kein CR. Sortiert nach `name`, dann `version`, bytegenau. `name`: `[A-Za-z0-9_.-]`, 1 bis 64 Zeichen. `version`: `[0-9A-Za-z.+-]`, 1 bis 64 Zeichen. `lizenz`: ein SPDX-Ausdruck aus `[A-Za-z0-9.+() -]`, 1 bis 128 Zeichen. `herkunft`: genau `crates.io` (jede andere Quelle verbietet `deny.toml`); Ausnahme nur `ubuntu-archiv` für `bubblewrap` (Form B des Profils). Kein Feld ist leer. Jede Bibliothek im Paket steht in der Liste; dieselbe Bibliothek in zwei Versionen darf zweimal stehen. `rustls-webpki` (prüft Ketten, enthält keine Wurzeln) ist erlaubt.

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.stueckliste_form` | leere Ausgabe, letzte Zeile ohne LF, CR, Steuerzeichen außer TAB und LF, Zeichen außerhalb ASCII | `Stueckliste hat falsche Form` |
| 2 | `paket.stueckliste_leer_feld` | eine Zeile hat nicht genau vier Felder, oder ein Feld ist leer | `Feld fehlt oder ist leer` |
| 3 | `paket.stueckliste_feld_form` | ein Feld verletzt Zeichenvorrat oder Länge | `Feld hat falsche Form` |
| 4 | `paket.stueckliste_herkunft` | `herkunft` ist nicht `crates.io`; Ausnahme nur `ubuntu-archiv` für die Bibliothek `bubblewrap` (Form B des Profils) | `Herkunft nicht crates.io` |
| 5 | `paket.stueckliste_verboten` | eine Bibliothek, die ein festes CA-Bündel mitbringt (`webpki-roots`, `webpki-root-certs`); der System-Truststore gilt immer (Vorgabe 20.11.) | `Bibliothek mit festem CA-Buendel` |
| 6 | `paket.stueckliste_doppelt` | derselbe Name in derselben Version zweimal | `Bibliothek doppelt` |
| 7 | `paket.stueckliste_reihenfolge` | die Zeilen sind nicht nach Name, dann Version sortiert | `Reihenfolge falsch` |
| 8 | `paket.stueckliste_fehlt` | eine Bibliothek im Paket steht nicht in der Liste | `Bibliothek ohne Eintrag` |

### Bauwerkzeuge (Nachtrag 20.11.2026)

Ein Bauwerkzeug baut das Paket und **liegt nie im Paket**. Die Liste steht je Plattform unter `rust/paket/<plattform>/bauwerkzeuge.tsv` neben `lieferumfang.txt` und `ausschluss.txt`, damit Lizenz und Herkunft nachprüfbar sind, obwohl `deny.toml` nur die Bibliotheken im Paket regelt (OPEN 3).

**Form.** Eine Zeile je Werkzeug: `<name>TAB<fassung>TAB<lizenz>TAB<pruefsumme>TAB<ausgeliefert>LF`. ASCII, LF, kein CR. `name`: `[a-z0-9][a-z0-9.+-]{0,63}`. `fassung`: `[0-9A-Za-z.+~:-]{1,64}`, genau die Fassung, die der Bau benutzt. `lizenz`: ein SPDX-Ausdruck aus `[A-Za-z0-9.+() -]`, 1 bis 128 Zeichen. `pruefsumme`: `sha256:` und 64 Kleinbuchstaben-Hexziffern, die Prüfsumme der Datei, wie sie geladen wurde (NuGet-Paket oder deb aus dem Archiv). `ausgeliefert`: genau `nein`. Kein Feld ist leer.

**Pflichtzeilen:** `ubuntu-26.04-x64`: `dpkg-dev` und `build-essential`; `windows-x64`: `wix`; `macos-arm64`: keine.

Gemessen am 20.11.2026 (Quellen: `https://api.nuget.org/v3-flatcontainer/wix/5.0.2/wix.5.0.2.nupkg`, `https://archive.ubuntu.com/ubuntu/dists/resolute/main/binary-amd64/Packages.gz` und die daraus geladenen Dateien; Lizenz aus `wix.nuspec` und aus `usr/share/doc/<paket>/copyright`):

| Name | Fassung | Lizenz | Prüfsumme (sha256) | wird nicht ausgeliefert |
|---|---|---|---|---|
| `wix` | `5.0.2` | `MS-RL` | `f30ef0c74e2a986126539c5780be93ac24e8136eaf723b1937b26272703ae173` | `nein` |
| `dpkg-dev` | `1.23.7ubuntu1` | `GPL-2.0-or-later` | `507b526fd78f9dff89696bbc3085af3117059f37fd281542778ac8b97be44690` | `nein` |
| `build-essential` | `12.12ubuntu2` | `GPL-2.0-or-later` | `e4896a79cf1d2b62c6d5be654e34a84678cc8cad344c6468419d53b614e1af2b` | `nein` |

Die Fassungen von `dpkg-dev` und `build-essential` sind die des Archivs `resolute` am Messtag; der Bau trägt die Fassung ein, die der Bauhost wirklich hat, und die Prüfsumme der dazu geladenen Datei. Eine andere Fassung ist kein Fehler dieses Vertrags, ein fehlender Eintrag oder `ja` schon.

**Keine Datei eines Bauwerkzeugs im Paket.** Die Dateien der Bauwerkzeuge stehen in der Ausschlussliste der Plattform. Anfangsliste (der Bau ergänzt): deb `usr/bin/dpkg-deb`, `usr/bin/dpkg-shlibdeps`, `usr/bin/dpkg-buildpackage`, `usr/bin/gcc`, `usr/bin/make`; MSI `bin/wix.exe` und `wix/`. Eine solche Datei im Baum oder in der Positivliste ergibt `paket.ausschluss_getroffen` (Stufe 6, vor `paket.ungelistet`); das Manifest nennt sie nie. Fälle: `liste-bauwerkzeug-*`, `liste-gueltig-linux-bauwerkzeug-ausschluss`.

| Stufe | Code | Verletzung | Grund (fest) |
|---|---|---|---|
| 1 | `paket.bauwerkzeuge_form` | leere Liste, letzte Zeile ohne LF, CR, Zeichen außerhalb ASCII, Steuerzeichen außer TAB und LF, eine Zeile ohne genau fünf Felder, ein leeres Feld oder ein Feld, das seine Form verletzt | `Bauwerkzeugliste hat falsche Form` |
| 2 | `paket.bauwerkzeug_doppelt` | derselbe Name zweimal | `Bauwerkzeug doppelt` |
| 3 | `paket.bauwerkzeug_ausgeliefert` | `ausgeliefert` ist nicht genau `nein` | `Bauwerkzeug im Paket` |
| 4 | `paket.bauwerkzeug_fehlt` | eine Pflichtzeile der Plattform fehlt | `Pflichtwerkzeug fehlt` |

## Gemeinsam: Prüfbefehl (A20, A108)

```
wb pruefen                 (Windows)
werkbank-pruefen           (Ubuntu und macOS, Arbeitsname)
```

Der Befehl nimmt kein Argument. **Eine einzige Ausnahme gibt es unter Ubuntu:** `werkbank-pruefen --vor-entfernen`, aufgerufen nur von `prerm` bei `apt remove`. Dieser Aufruf prüft nur, ob eine Schlüsseldatei da ist: Ist sie da (gleich in welchem Zustand), schreibt er `pruef.schluessel_rest`; fehlt sie, schreibt er nichts; ist die Prüfung nicht möglich, schreibt er den Satz von `pruef.schluessel_nicht_pruefbar`. Der Rückgabewert ist immer 0, damit das Entfernen nie daran scheitert. Jedes andere Argument, auch dieses unter Windows und macOS, ergibt rc 64. Ein Lauf sagt der IT, was fehlt. Er läuft alle Einzelprüfungen seiner Plattform (Windows 5, Ubuntu 7, macOS 6), auch wenn eine rot ist, und ändert nichts am Rechner. Er schreibt weder Geheimnisse noch Dateiinhalte noch Pfade außerhalb des Manifests: nur feste Texte und die Pfade aus dem Manifest. Die Tabelle unten gilt für Windows; die Prüfungen unter Ubuntu und macOS stehen im nächsten Absatz und in Teil C.

### Die fünf Prüfungen und ihre Zustände

Jede Einzelprüfung meldet einen Zustand. Jeder Zustand außer `ok` ergibt genau eine Zeile (bei `paketstand` je Datei eine). Der Zustand `nicht_pruefbar` gilt, wenn die Prüfung nicht möglich war (Recht fehlt, Frist von 10 s verstrichen): Er ist rot, nie still grün (fail closed).

| Prüfung | Zustände | Was geprüft wird |
|---|---|---|
| `halter` | `ok`, `fehlt`, `konto`, `gestoppt`, `nicht_pruefbar` | Der Dienst `wb-halter` ist eingetragen, läuft unter `NT SERVICE\wb-halter` und ist gestartet |
| `wfp` | `ok`, `dienst_fehlt`, `filter_fehlt`, `sperre_fehlt`, `nicht_pruefbar` | Der Filterdienst `wb-plattform` läuft, die Filtergruppe der Werkbank besteht, und jeder bekannte Agentencontainer hat sein Sperrpaar für 127.0.0.0/8 und ::1 (A122) |
| `einstellungen` | `ok`, `fehlt`, `ungueltig`, `nicht_pruefbar` | Die Einstellungsdatei ist lesbar und gültig nach Vertrag 2 (`einstellungen::lesen`); bei `ungueltig` gehört der `einst.*`-Code dazu |
| `vault` | `ok`, `fehlt`, `kein_ordner`, `rechte`, `nicht_pruefbar` | Der Vault-Ordner besteht, ist ein Ordner und hat die Sollrechte (Annahme 4) |
| `paketstand` | Manifest `ok`, `fehlt`, `ungueltig`, `nicht_pruefbar`; dazu Bauart, Version und Dateien | `manifest.json` ist gültig nach diesem Vertrag; die installierte Version (`ProductVersion`) gleicht der des Manifests; jede Datei hat Größe und `sha256` des Manifests; im Produktordner liegt keine Datei außer `manifest.json`, die nicht im Manifest steht; `bau` ist `auslieferung` |

**Ubuntu (acht Prüfungen, Reihenfolge der Ausgabe):** `dienste` (`werkbank-kern.service`, `werkbank-bruecke.service` und `wb-halter.service` (Nachtrag Halter) je `ok`, `fehlt`, `inaktiv`, `fehlgeschlagen`, `nicht_pruefbar`; der Modellweg ist keine Einheit des Pakets und wird nicht geprüft; **Nachtrag Rolle aus der Gruppe:** dazu, als Teil dieser Prüfung, die Rollengruppe `wb-bruecke` (Lagefeld `rollengruppe`: `ok`, `fehlt`, `mitglied`, `nicht_pruefbar`) und das Rollenprogramm `/usr/lib/werkbank/werkbank-modellweg` (Lagefeld `rollenprogramm`: `ok`, `fehlt`, `modus`, `nicht_pruefbar`), beide Zeilen nach den Zeilen der Einheiten, die Gruppe zuerst; fehlt ein Lagefeld, gilt `ok`, damit bleiben alle früheren Fälle und die Zahl acht), `apparmor` (`ok`, `unconfined`, `bwrap_fehlt`, `datei_fehlt`, `nicht_geladen`, `abweichend`, `fremd`, `probelauf`, `system_bwrap_frei`, `kopie_abweichend`, `nicht_pruefbar`), `einstellungen` (wie oben), `nutzerordner` (`/var/lib/werkbank/<nutzer>/`: `ok`, `fehlt`, `rechte` für Modus oder Eigentümer, `nicht_pruefbar`), `schluessel` (`ok`, `fehlt`, `art`, `eigentuemer`, `modus`, `leer`, `nicht_pruefbar`; nur Art, Eigentümer, Modus und Leere, nie der Inhalt), `copilot` (Anmeldung der Route `copilot` in der Ablage des Halters, Zustände `ok`, `ablage`, `fehlt`, `art`, `eigentuemer`, `modus`, `leer`, `form`, `nicht_pruefbar`; dazu der Befund zum alten Ort; siehe Prüfung `copilot` oben), `vault` (der Vorführ-Vault des Nutzers) und `paketstand` (wie oben, Wurzel `/`; fremde Dateien werden nur unter `usr/lib/werkbank/` und `usr/share/werkbank/` gesucht). **Der Nutzer (Nachtrag 20.11.2026, an earlier test run):** Ruft ein Nutzer ohne root auf (die reale Benutzerkennung, `getuid`, ist nicht 0), ist der Nutzer der der echten Kennung; sein Name kommt aus `/etc/passwd`, und `SUDO_USER` gilt dann nicht (eine Umgebungsvariable des Aufrufers legt keinen anderen Nutzer fest). Mit `sudo` (reale Kennung 0) ist der Nutzer der, der `sudo` aufrief (`SUDO_USER`). Ruft root ohne `SUDO_USER` auf, bleibt es wie bisher: ohne Nutzer sind Dienste, Nutzerordner und Schlüssel `nicht_pruefbar`. Ohne root läuft jede Sonde direkt als dieser Nutzer (kein `setpriv`). Eine Prüfung, die dem Nutzer nicht lesbar ist (etwa die Liste der geladenen Profile unter `/sys/kernel/security/apparmor/`), bleibt `nicht_pruefbar` (rot, nie still grün); dazu steht vor `pruef.rot` oder `pruef.ok` die Zeile `pruef.hinweis_sudo` (Lagefeld `ohne_root`, `true` bei einem Aufruf ohne root; ein Hinweis, kein Befund: er zählt nicht in `pruef.rot` und ändert den rc nicht). Er steht nach den anderen Hinweiszeilen. Fälle: `pruefung-linux-ohne-root-*`, `pruefung-linux-mit-root-nicht-pruefbar-ohne-hinweis`. **Ubuntus Profil ohne root (Nachtrag 1, 20.11.):** Die Liste der geladenen Profile ist ohne root nicht lesbar. Bei aktiver Sperre leitet der Befehl den Zustand von `bwrap-userns-restrict` dann aus der Probe des System-`bwrap` ab, die er als Nutzer startet (Kind liest `/proc/self/attr/current`): Enthält das Label des Kindes `bwrap` oder `unpriv_bwrap`, gilt das Profil als geladen (S1); scheitert die Probe mit `setting up uid map: Permission denied`, gilt es als nicht geladen (S0); sonst steht keine Zeile da (Lagefeld `ubuntu_profil` fehlt). Mit root bleibt die Liste der Profile die Quelle. Die Prüfung `apparmor` (unsere Labels) bleibt ohne root `nicht_pruefbar` mit `pruef.hinweis_sudo`. Fälle: `pruefung-linux-ohne-root-ubuntu-profil-*`. Welche Prüfungen ein Nutzer ohne root in der VM vollständig bekommt, steht in der Ergebnisdatei der Messung (Abschnitt „Aufruf ohne root“ der Zusagen). Das AppArmor-Profil gilt als `ok`, wenn die Sperre der Namensräume nicht aktiv ist und der Probelauf des Einschlusses gelingt.

Dazu kommen zwei Eingaben, die keine Prüfung sind: `konten` (Zahl der menschlichen lokalen Konten des Rechners, mindestens 1) und `stelle` (`ok` oder `fehlt`: die Entscheidungsstelle des Rechners ist eingerichtet; Annahme 3).

### Ausgabe und Codes

Ausgabe auf stdout, eine Zeile je Befund, ASCII, höchstens 200 Byte, Zeilenende LF: `<code>: <satz>`. Reihenfolge: `halter`, `wfp`, `einstellungen`, `vault`, `paketstand`. Im `paketstand`: zuerst das Manifest; ist es nicht `ok`, folgt sonst nichts. Sonst `testbau`, `version`, dann die Dateien. Die Dateizeilen stehen nach Pfad (bytegenau), bei gleichem Pfad nach Art `fehlt`, `abweichend`, `fremd`. Es erscheinen höchstens 20 Dateizeilen; weitere ersetzt eine Zeile `paketstand_weitere` mit ihrer Zahl.

| Code | Satz (fest, ASCII) |
|---|---|
| `pruef.halter_fehlt` | `Der Dienst wb-halter ist nicht eingetragen.` |
| `pruef.halter_konto` | `Der Dienst wb-halter laeuft nicht unter dem Konto NT SERVICE\wb-halter.` |
| `pruef.halter_gestoppt` | `Der Dienst wb-halter ist nicht gestartet.` |
| `pruef.halter_nicht_pruefbar` | `Der Dienst wb-halter war nicht pruefbar.` |
| `pruef.wfp_dienst_fehlt` | `Der Filterdienst wb-plattform laeuft nicht.` |
| `pruef.wfp_filter_fehlt` | `Die Filtergruppe der Werkbank fehlt in der Filterplattform.` |
| `pruef.wfp_sperre_fehlt` | `Fuer einen Agentencontainer fehlt die Sperre fuer 127.0.0.0/8 und ::1.` |
| `pruef.wfp_nicht_pruefbar` | `Die Filter der Werkbank waren nicht pruefbar.` |
| `pruef.einstellungen_fehlt` | `Die Einstellungsdatei fehlt oder ist nicht lesbar.` |
| `pruef.einstellungen_ungueltig` | `Die Einstellungsdatei ist ungueltig (<einst-code>).` |
| `pruef.einstellungen_nicht_pruefbar` | `Die Einstellungsdatei war nicht pruefbar.` |
| `pruef.vault_fehlt` | `Der Vault-Ordner fehlt.` |
| `pruef.vault_kein_ordner` | `Der Vault-Pfad ist kein Ordner.` |
| `pruef.vault_rechte` | `Der Vault-Ordner hat falsche Rechte.` |
| `pruef.vault_nicht_pruefbar` | `Der Vault-Ordner war nicht pruefbar.` |
| `pruef.paketstand_manifest_fehlt` | `Das Manifest des Pakets fehlt.` |
| `pruef.paketstand_manifest_ungueltig` | `Das Manifest des Pakets ist ungueltig.` |
| `pruef.paketstand_nicht_pruefbar` | `Der Paketstand war nicht pruefbar.` |
| `pruef.paketstand_testbau` | `Das Paket ist ein Testbau.` |
| `pruef.paketstand_version` | `Die installierte Version weicht vom Manifest ab.` |
| `pruef.paketstand_datei_fehlt` | `Die Datei <pfad> fehlt.` |
| `pruef.paketstand_datei_abweichend` | `Die Datei <pfad> weicht vom Manifest ab.` |
| `pruef.paketstand_datei_fremd` | `Die Datei <pfad> steht nicht im Manifest.` |
| `pruef.paketstand_weitere` | `<n> weitere Dateien weichen ab.` |
| `pruef.dienst_fehlt` | `Der Dienst <einheit> ist nicht eingetragen.` (Ubuntu; `<einheit>` ist `werkbank-kern.service` oder `werkbank-bruecke.service`) |
| `pruef.dienst_inaktiv` | `Der Dienst <einheit> ist nicht aktiv.` |
| `pruef.dienst_fehlgeschlagen` | `Der Dienst <einheit> ist fehlgeschlagen.` |
| `pruef.dienst_nicht_pruefbar` | `Der Dienst <einheit> war nicht pruefbar.` |
| `pruef.rollengruppe_fehlt` | `Die Rollengruppe wb-bruecke fehlt; der Modellweg bekommt beim Halter keine Rolle.` (Nachtrag Rolle aus der Gruppe; Ubuntu) |
| `pruef.rollengruppe_mitglied` | `Die Rollengruppe wb-bruecke hat ein Mitglied, ein Passwort oder eine falsche Form; ein Prozess eines Kontos koennte die Rolle bruecke annehmen.` (Befund, rc 1; Regeln unten) |
| `pruef.rollengruppe_nicht_pruefbar` | `Die Rollengruppe wb-bruecke war nicht pruefbar.` |
| `pruef.rollenprogramm_fehlt` | `Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg fehlt.` |
| `pruef.rollenprogramm_modus` | `Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg hat nicht root:wb-bruecke mit Modus 2755.` |
| `pruef.rollenprogramm_nicht_pruefbar` | `Das Rollenprogramm war nicht pruefbar.` |
| `pruef.apparmor_bwrap_fehlt` | `Das Programm /usr/bin/bwrap fehlt.` |
| `pruef.apparmor_datei_fehlt` | `Die Datei des AppArmor-Profils fehlt.` |
| `pruef.apparmor_nicht_geladen` | `Das AppArmor-Profil ist nicht geladen.` |
| `pruef.apparmor_abweichend` | `Das AppArmor-Profil weicht vom Paket ab.` |
| `pruef.apparmor_fremd` | `Ein anderes AppArmor-Profil gilt fuer /usr/bin/bwrap.` |
| `pruef.apparmor_probelauf` | `Der Probelauf des Einschlusses scheitert.` |
| `pruef.apparmor_unconfined` | `Ein Profil der Werkbank ist ohne Einschraenkung geladen.` |
| `pruef.apparmor_system_bwrap_frei` | `Das System-bwrap ist fuer jeden Nutzer freigegeben.` (nur bei Freigabe durch ein Profil der Werkbank oder ohne Profil) |
| `pruef.hinweis_system_bwrap_ubuntu` | `Das System-bwrap ist durch das Ubuntu-Profil frei; die Firma kann dieses Profil entfernen.` (Hinweis, kein Befund) |
| `pruef.hinweis_ubuntu_profil_geladen` | `Das Ubuntu-Profil bwrap-userns-restrict ist geladen.` (Hinweis, kein Befund; nur bei aktiver Sperre) |
| `pruef.hinweis_ubuntu_profil_entladen` | `Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen.` (Hinweis, kein Befund; nur bei aktiver Sperre) |
| `pruef.hinweis_sudo` | `Ohne root waren nicht alle Pruefungen moeglich; fuer diese Pruefungen mit sudo aufrufen.` (Hinweis, kein Befund; nur unter Ubuntu bei einem Aufruf ohne root und nur, wenn mindestens eine Prüfung `nicht_pruefbar` meldet) |
| `pruef.hinweis_ca_zugelassen` | `Die Zulassung der lokalen CA ist erteilt; die Cloud-Route ist nicht gesperrt.` (Hinweis, kein Befund; Ubuntu) |
| `pruef.hinweis_ca_offen` | `Die Zulassung der lokalen CA ist offen; die Cloud-Route ist gesperrt. Das ist kein Fehler der Installation.` (Hinweis, kein Befund, rc bleibt 0; Ubuntu) |
| `pruef.hinweis_ca_abgelehnt` | `Die Zulassung der lokalen CA ist abgelehnt; die Cloud-Route ist gesperrt.` (Hinweis, kein Befund, rc bleibt 0; Ubuntu) |
| `pruef.hinweis_ca_doppelt` | `lauf.json nennt ca_zulassung mehr als einmal; das gilt als offen, die Cloud-Route ist gesperrt.` (Hinweis, kein Befund, rc bleibt 0; Ubuntu; so liest das Paket einen doppelten Schlüssel: als `offen`) |
| `pruef.hinweis_harness_nicht_gefunden` | `<n> nicht gefunden in ~/.nvm/versions/node/v*/bin, ~/.local/bin, ~/.npm-global/bin, ~/.volta/bin, /usr/local/bin, /usr/bin; andere Orte werden nicht gesucht.` (Hinweis, kein Befund; `<n>` ist `copilot` oder `pi`; Ubuntu) |
| `pruef.hinweis_harness_ausserhalb` | `<n> liegt an einem gesuchten Ort, der echte Pfad aber nicht unter /usr, /opt, ~/.nvm, ~/.local, ~/.npm-global oder ~/.volta; er wird nicht genommen.` (Hinweis, kein Befund; Ubuntu) |
| `pruef.hinweis_harness_nicht_eingetragen` | `<n> liegt an einem gesuchten Ort, steht aber nicht in lauf.json; sudo dpkg-reconfigure werkbank.` (Hinweis, kein Befund; Ubuntu) |
| `pruef.hinweis_ca_unbekannt` | `Die Zulassung der lokalen CA war nicht lesbar; die Cloud-Route ist gesperrt.` (Hinweis, kein Befund, rc bleibt 0; Ubuntu; die Datei ist da, aber ein Verweis, kein JSON oder ohne einen der drei Werte; ein doppelter Schlüssel hat seit dem 21.11. die eigene Zeile `pruef.hinweis_ca_doppelt`) |
| `pruef.apparmor_kopie_abweichend` | `Die bwrap-Kopie des Produkts weicht vom System-bwrap ab.` |
| `pruef.apparmor_nicht_pruefbar` | `Das AppArmor-Profil war nicht pruefbar.` |
| `pruef.nutzerordner_fehlt` | `Der Ordner des Nutzers unter /var/lib/werkbank fehlt.` |
| `pruef.nutzerordner_rechte` | `Der Ordner des Nutzers hat falsche Rechte oder einen falschen Eigentuemer.` |
| `pruef.nutzerordner_nicht_pruefbar` | `Der Ordner des Nutzers war nicht pruefbar.` |
| `pruef.programmordner_verboten` | `Ein Programmordner in lauf.json beruehrt den Anmeldespeicher oder ein Geheimnis; der Einschluss lehnt jede Sitzung ab (5 verboten).` (Befund, rc 1; Ubuntu; steht nach dem Nutzerordner und vor der Schluesseldatei. Derselbe Code mit dem Satz `Der Ordner beruehrt den Anmeldespeicher oder ein Geheimnis; der Einschluss lehnt ihn ab (5 verboten).` ist die Ausgabe von `werkbank-pruefen --programmordner`) |
| `pruef.schluessel_fehlt` | `Die Schluesseldatei fehlt.` |
| `pruef.schluessel_art` | `Die Schluesseldatei ist keine gewoehnliche Datei.` |
| `pruef.schluessel_eigentuemer` | `Die Schluesseldatei gehoert nicht dem Nutzer.` |
| `pruef.schluessel_modus` | `Die Schluesseldatei hat nicht den Modus 0600.` |
| `pruef.schluessel_leer` | `Die Schluesseldatei ist leer.` |
| `pruef.schluessel_nicht_pruefbar` | `Die Schluesseldatei war nicht pruefbar.` |
| `pruef.copilot_fehlt` | `Die Anmeldedatei der Copilot CLI fehlt.` |
| `pruef.copilot_art` | `Die Anmeldedatei der Copilot CLI ist keine gewoehnliche Datei.` |
| `pruef.copilot_eigentuemer` | `Die Anmeldedatei der Copilot CLI gehoert nicht dem Nutzer.` |
| `pruef.copilot_modus` | `Die Anmeldedatei der Copilot CLI hat nicht den Modus 0600.` |
| `pruef.copilot_leer` | `Die Anmeldedatei der Copilot CLI ist leer.` |
| `pruef.copilot_form` | `Die Anmeldedatei der Copilot CLI hat falsche Form.` (nicht ein Wert von 1 bis 12288 druckbaren ASCII-Zeichen ohne Leerzeichen und genau ein abschließendes LF, Fund F11; alt: „1 bis 4096 …, mit oder ohne abschließendes LF“; geprüft wird die Form, der Inhalt erscheint nie) |
| `pruef.copilot_nicht_pruefbar` | `Die Anmeldedatei der Copilot CLI war nicht pruefbar.` |
| `pruef.copilot_ablage` | `Die Ablage des Halters hat falsche Rechte, einen falschen Eigentuemer oder einen Verweis.` (Nachtrag Halter; vor `pruef.copilot_fehlt` geprüft) |
| `pruef.copilot_alter_ort` | `Am frueheren Ort liegt noch eine Anmeldedatei der Copilot CLI; sie wird nicht mehr gelesen und muss weg.` (Nachtrag Halter; Befund, rc 1; steht nach der Zeile der Prüfung `copilot`) |
| `pruef.copilot_rest` | `Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie.` (nur bei `--vor-entfernen`) |
| `pruef.schluessel_rest` | `Eine Schluesseldatei liegt noch da; apt purge loescht sie.` (nur bei `--vor-entfernen`, also nach `apt remove`; Satz ohne Inhalt, Länge und Pfad) |
| `pruef.launchd_fehlt` | `Der Dienst werkbank.halter ist nicht eingetragen.` (macOS) |
| `pruef.launchd_gestoppt` | `Der Dienst werkbank.halter laeuft nicht.` |
| `pruef.launchd_konto` | `Der Dienst werkbank.halter laeuft nicht unter seinem eigenen Konto.` |
| `pruef.launchd_nicht_pruefbar` | `Der Dienst werkbank.halter war nicht pruefbar.` |
| `pruef.schluesselbund_fehlt` | `Der Schluesselbund-Eintrag des Modellwegs fehlt.` |
| `pruef.schluesselbund_nicht_pruefbar` | `Der Schluesselbund-Eintrag war nicht pruefbar.` |
| `pruef.einschluss_fehlt` | `Der Einschluss ist nicht verfuegbar.` |
| `pruef.einschluss_probelauf` | `Der Probelauf des Einschlusses scheitert.` |
| `pruef.einschluss_nicht_pruefbar` | `Der Einschluss war nicht pruefbar.` |
| `pruef.rot` | `<n> Befunde.` (letzte Zeile, wenn es mindestens einen Befund gibt; `n` zählt die Befundzeilen davor) |
| `pruef.ok` | `Alle <n> Pruefungen bestanden.` (`n` ist 5 unter Windows, 7 unter Ubuntu, 6 unter macOS) |
| `pruef.stelle_einrichten` | `Installation fertig, Stelle einrichten. Befehl: wb stelle einrichten` |
| `pruef.aufruf` | `Der Pruefbefehl kennt dieses Argument nicht.` (stderr) |
| `pruef.intern` | `Interner Fehler.` (stderr; entsteht nur durch einen Fehler im Programm, kein sichtbarer Fall) |

**Regeln der Rollengruppe und des Rollenprogramms (Nachtrag Rolle aus der Gruppe).** `rollengruppe` liest `/etc/group`, `/etc/passwd` und,
wenn vorhanden, `/etc/gshadow`, in dieser Reihenfolge der Zustände: `fehlt`, wenn `wb-bruecke` nicht in `/etc/group` steht; `mitglied`, wenn
die Gruppe mehr als einen Eintrag hat, ihre gid keine Zahl oder 0 ist, das vierte Feld ein Mitglied nennt, das Passwortfeld nicht `x`, `!`,
`*` oder leer ist, ein Konto in `/etc/passwd` sie als Hauptgruppe hat (jedes Konto, nicht nur die Person), oder ihr Eintrag in
`/etc/gshadow` ein Passwort (nicht leer und nicht mit `!` oder `*` beginnend), einen Verwalter oder ein Mitglied nennt; `nicht_pruefbar`,
wenn `/etc/group` oder `/etc/passwd` nicht lesbar ist oder `/etc/gshadow` da, aber nicht lesbar ist (ohne root der Normalfall; dann gilt
der Hinweis `pruef.hinweis_sudo`). `rollenprogramm` sieht `/usr/lib/werkbank/werkbank-modellweg` ohne Verweis an: `fehlt` ohne Eintrag;
`modus`, wenn es kein gewöhnliches File ist, nicht uid 0 gehört, seine Gruppe nicht die gid von `wb-bruecke` ist (oder die Gruppe fehlt)
oder sein Modus nicht genau 2755 ist; `nicht_pruefbar`, wenn der Blick scheitert. Gegenproben: Fälle
`pruefung-linux-rolle-*` (je Zustand ein Fall, dazu beide zugleich und ein Fall mit allen anderen Prüfungen grün). Grund
(Projektentscheid, 25.11.2026): Mit einem Konto in der Gruppe hätte jeder Prozess dieses Kontos die Rolle `bruecke`; ohne setgid hätte der
Modellweg keine.

Der Befehl nennt den Satz und den Code, nicht mehr: Er sagt nicht, wie man den Fehler behebt. Der Grund ist die Zusage „keine Pfade und keine Inhalte“; eine Behebungsanleitung gehört in die Einrichtungsanleitung (A137).

### Exitcodes

| rc | Bedeutung |
|---|---|
| `0` | kein Befund; letzte Zeile `pruef.ok` (A20) |
| `1` | mindestens ein Befund; letzte Zeile `pruef.rot` |
| `20` | **A108: Ausgang des Prüfbefehls nach der Installation, „Installation fertig, Stelle einrichten“.** Die Installation selbst (`apt`, `msiexec`) liefert diesen Code nicht. Kein Befund; nur die Stelle fehlt, und der Rechner hat mindestens zwei Konten. Die einzige Zeile ist `pruef.stelle_einrichten`. Das ist kein Fehlschlag. |
| `64` | Aufruf falsch (ein Argument), eine Zeile auf stderr |
| `70` | interner Fehler, eine Zeile auf stderr |

Regeln zu A108, mit ihren Gegenproben als Fälle:

- Fehlt die Stelle auf einem Rechner mit **einem** Konto, gilt rc 0 (Gegenprobe 1).
- Gibt es **irgendeinen** Befund, gilt rc 1 und die Zeile der Stelle entfällt, damit niemand den Code 20 für einen Fehler hält und die letzte Zeile nie zwei Bedeutungen trägt (Gegenprobe 2). Die Zeile erscheint erst, wenn alles andere in Ordnung ist.
- Der Code 20 gehört dem Prüfbefehl, nicht `msiexec` (siehe „Installation und Orte“). Die frühere Installation (`install.sh`) nutzte die Codes 10 und 11; sie gelten im Neubau nicht weiter.

Frist: Eine Einzelprüfung dauert höchstens 10 s, dann gilt sie als `nicht_pruefbar`. Der ganze Lauf dauert höchstens 30 s (Zusage; die Messung folgt mit dem Bau).

## Gemeinsam: Sicherung und Wiederherstellung (A19), nur Schnittstelle und Zusage

Gebaut wird später. Dieser Vertrag legt nur fest, was der Bau einhalten muss.

```
wb sichern <zielordner>          schreibt genau eine Sicherungsdatei in den Zielordner, Pfad auf stdout
wb wiederherstellen <datei>      stellt aus der Sicherungsdatei den Betrieb wieder her
```

Zusagen:

| Nr | Zusage | geprüft durch |
|---|---|---|
| S1 | Die Sicherung läuft im laufenden Betrieb und liest einen **in sich stimmigen Stand**: Das Protokoll ist bis zu einer vollständigen Zeile gesichert und die Kettenprüfung der Sicherung grün. | Außentest, später |
| S2 | Der Umfang der Sicherung steht als Positivliste wie der Lieferumfang. Eine Datei außerhalb der Liste kommt nicht hinein. | Fälle, später |
| S3 | Die Sicherung enthält **kein Geheimnis im Klartext**. Ob sie das Geheimnisfach des Halters überhaupt enthält, ist offen (OPEN 13). | Gegenprobe, später |
| S4 | `wiederherstellen` prüft **vor dem ersten Schreiben**: Form der Datei, Prüfsummen, Kettenprüfung des Protokolls. Eine verletzte Prüfung bricht ab, ohne etwas zu ändern (kein Teilzustand). | Fälle, später |
| S5 | `wiederherstellen` läuft auf einem Rechner, auf dem nur das Paket installiert ist (Rechner geleert). Danach ist `wb pruefen` rc 0 oder 20, die Protokollprüfung grün und die Anmeldung möglich. | Außentest (A19), später |
| S6 | Die Sicherung ändert den Betrieb nicht und schreibt nur in den Zielordner. Sie schreibt nie in den Produktordner. | Außentest, später |

Reservierte Codes (Namensraum `sich.`, noch keine Tabelle): Exitcodes `0`, `64` Aufruf, `65` Sicherung ungültig (Prüfsumme oder Kette), `66` Datei oder Ziel nicht lesbar oder nicht beschreibbar, `69` Betrieb läuft und darf nicht überschrieben werden, `70` intern.

## Sicherheitszusagen als prüfbare Eigenschaften (Windows und gemeinsam)

Die Zusagen des deb stehen im Teil A (ZL1 bis ZL7), die Außentests des deb dort (AL-1 bis AL-10) und die des pkg im Teil C (AM-1 bis AM-5).

| Nr | Zusage | Prüfung |
|---|---|---|
| Z1 | Ein Auslieferungsbau enthält die Attrappe nicht. | Fälle `manifest-merkmal-verboten-*`, `liste-ausschluss-getroffen-attrappe`; Außentest AT-8 |
| Z2 | Paketinhalt, Manifest und Positivliste nennen dieselben Dateien. Eine Datei ohne Eintrag kommt nicht mit, ein Eintrag ohne Datei bricht den Bau. | Fälle `liste-ungelistet*`, `liste-fehlt-im-baum`; AT-8 |
| Z3 | Kein Testwerkzeug, kein Entwicklerwerkzeug und kein Skript im Paket; kein Python, kein Node. | Fälle `liste-ausschluss-getroffen-*`; AT-4 |
| Z4 | Jede Bibliothek im Paket steht in der Stückliste mit Lizenz und Herkunft `crates.io`. | Fälle `stueckliste-*`; AT-9 |
| Z5 | Die ausgelieferte MSI trägt eine gültige Authenticode-Signatur; ein manipuliertes Paket wird abgelehnt. Der Signaturschlüssel liegt nie im Repository und nie im Bauablauf des Produkts. | AT-2 |
| Z6 | Die Installation braucht kein Netz und keine Eingabe. | AT-1, AT-5 |
| Z7 | Produktordner und Admin-Pfad sind für normale Nutzer schreibgeschützt; neue Dateien entstehen nur im Profil. | AT-3 |
| Z8 | Der Prüfbefehl ändert nichts und gibt weder Geheimnisse noch Dateiinhalte noch Pfade außerhalb des Manifests aus. | Fälle `pruefung-*` (Zeilen sind feste Texte); AT-6 |
| Z9 | Eine nicht mögliche Prüfung ist rot, nie grün (fail closed). | Fälle `pruefung-*-nicht-pruefbar` |
| Z10 | Je Befund genau eine Zeile; die Reihenfolge ist fest; ein Lauf nennt alle Befunde, nicht nur den ersten. | Fälle `pruefung-alles-rot`, `pruefung-paketstand-sortiert` |
| Z11 | Der erste Start jedes Programms nach der Installation dauert nicht länger als beim zweiten Start plus die Zeit des Echtzeitschutzes, weil die Installation es vorgestartet hat (A126). | AT-7 |
| Z12 | Die MSI enthält keine aus WiX stammende Datei (keine Erweiterungsbibliothek in der Binary-Tabelle, kein WiX-Bootstrapper). | AT-8 (zusätzlich: Binary-Tabelle der MSI auflisten) |

## Außentests (Windows-VM, Teil B)

Alle laufen in einer Windows-VM (eigener Schnappschuss je Lauf, kein Token in der VM, je Block höchstens 30 Minuten). Dieser Vertrag startet keine VM. AT-8 braucht keine Windows-VM.

| Nr | Ausgangslage | Handlung | Erwartet | Gemessen |
|---|---|---|---|---|
| AT-1 | frische VM, kein Netz, kein WSL | MSI mit dem Befehl oben installieren (A72, M4) | Rückgabe 0 oder 3010, Eingabeaufforderungen 0, **Gesamtdauer höchstens 15 Minuten** | Rückgabewert, Dauer in Sekunden, Eingabeaufforderungen |
| AT-2 | echte MSI und ein Zweitpaket mit einem geänderten Byte | `Get-AuthenticodeSignature` je Paket; stille Installation beider | die echte hat Status `Valid`, die manipulierte nicht (`HashMismatch`); die manipulierte wird nicht installiert | Status, Rückgabewerte; ob `msiexec` allein ablehnt, ist offen (OPEN 2) |
| AT-3 | installiertes Paket, normaler Nutzer | schreiben in `%ProgramFiles%\Werkbank\` und `%ProgramData%\Werkbank\admin\`; danach eine Sitzung mit Suche (A73) | beide Schreibversuche scheitern, neue Dateien nur im Profil | Rückgabe ungleich 0; Prüfsummen von Produktordner und Admin-Pfad vorher und nachher gleich |
| AT-4 | Positivliste des Pakets | Prüfliste der VM: WSL, Python, Node nicht vorhanden; `wb --version` und je ein Unterbefehl für Hooks und Brain laufen (A60) | Rückgabe 0 | Prüfliste, Rückgabewerte |
| AT-5 | VM, deren Netz nur den eigenen Rechner erreicht | Installation, Start, ein Lauf mit lokaler Attrappe der Modell-Quelle (A69) | alles gelingt | Mitschnitt: Verbindungen zu Adressen außerhalb des Rechners 0 |
| AT-6 | installierte VM | je Code der Tabelle den Zustand erzeugen (Dienst anhalten, Filtergruppe löschen, Einstellungsdatei beschädigen, Rechte am Vault ändern, Datei ändern, entfernen und hinzufügen), `wb pruefen` ausführen | Zeilen und rc wie in den Fällen `pruefung-*`; ohne Fehler rc 0 | Zeilen und rc je Zustand; Abweichungen 0 |
| AT-7 | VM mit Echtzeitschutz, frisch installiert | den ersten Start von `wb.exe` messen, danach den zweiten (A126) | der erste Start liegt nicht über dem gemessenen Wert der ersten Starts (0,8 bis 10,3 s) und nicht über dem zweiten Start plus 100 ms Abstand | Zeiten beider Starts |
| AT-8 | gebaute MSI (jeder Host mit `msiextract` oder `msiexec /a`) | alle Dateien der MSI entpacken und mit Manifest und Positivliste vergleichen; Auslieferungs-MSI auf die Attrappe absuchen | Dateimenge der MSI = Manifest = Positivliste (außer `manifest.json`); keine Attrappe; kein Skript | Zähler abweichender Dateien: 0 |
| AT-9 | installiertes Paket | `wb lizenzen`; die Bibliotheken im Paket auflisten | jede Bibliothek steht in der Liste, kein Feld leer (A75) | Zähler ohne Eintrag und mit leerem Feld: 0 |
| AT-10 | VM mit zwei weiteren Konten und ohne Stelle; frisches Konto | Installation, danach `wb pruefen` (A108) | rc 20, letzte Zeile der Satz mit dem Befehl; die App startet und antwortet | Rückgabewert, letzte Zeile, Ping |
| AT-11 | dieselbe VM mit einem Konto | wie AT-10 | rc 0 | Rückgabewert |
| AT-12 | installiertes Paket, ein echter Fehler (Rollendatei oder Dienst fehlt) | `wb pruefen` | rc 1, nicht 20 | Rückgabewert, Zeilen |
| AT-13 | Version 1 installiert, Paket der Version 2 | Version 2 einspielen, danach Rückweg auf 1 (A76) | `wb --version` zeigt zuerst 2, dann 1; Nutzerdaten bleiben | Versionsausgabe, Prüfsumme der Nutzerdaten |

## Abstimmung mit den anderen Bausteinen

Das Projekt stimmt das mit den Bausteinen ab. Was dieser Vertrag von wem liest, liefert oder annimmt:

| Baustein | Liefert an diesen Vertrag | Liest von diesem Vertrag | Annahme |
|---|---|---|---|
| Plattformschicht und Halter | Programme `wb-halter` und `wb-plattform`, deren Dienstnamen und Konten; eine Einzelprüfung je Dienst und je Filter (`halter`, `wfp`); `wb-plattform --einrichten` und `--entfernen` für die Custom Actions | Pflichtdateien, Orte, Frist von 10 s, den Zustandsvorrat von `halter` und `wfp` | **1.** Die Namen `wb`, `wb-halter`, `wb-plattform`, die Dienstkonten und die Befehle `--einrichten` und `--entfernen`; die Probe der Prüfungen ist lesend und braucht höchstens das Recht, das ein Administrator hat. |
| Einstellungen und Start (Verträge 1 und 2) | `einstellungen::lesen` mit den `einst.*`-Codes; Ort der Einstellungsdatei | Prüfung `einstellungen`; Ort in `%ProgramData%\Werkbank\admin\` | **2.** Der Ort der Einstellungsdatei ist `%ProgramData%\Werkbank\admin\einstellungen.json` (Vertrag 2, OPEN 3, noch offen). |
| Entscheidungsstelle | `stelle`-Zustand `ok` oder `fehlt`, Zahl der menschlichen Konten, den Befehl zum Einrichten | die Eingaben `konten` und `stelle`, den Exitcode 20 | **3.** Der Befehl heißt `wb stelle einrichten`; die Stelle liefert beide Eingaben der Prüfung. |
| Vault | Ort und Sollrechte des Vault-Ordners in der Konfiguration des Produkts | Prüfung `vault` | **4.** Die Sollrechte lassen sich als eine Ja-Nein-Prüfung ausdrücken (Schreibrecht nur für die Sollkonten). |
| Einschluss | Name des Merkmals der Attrappe und Name ihrer Datei; Crate des Windows-Einschlusses | Positivliste, Ausschluss, Manifest-Merkmale | **5.** Das Merkmal heißt `attrappe`, die Datei `bin/wb-attrappe.exe`. |
| Formate und Protokoll | Protokollprüfung (Kettenprüfung) für die Wiederherstellung | Schnittstelle von Teil 3 | **6.** Die Kettenprüfung ist als Bibliothek aufrufbar, ohne dass der Dienst läuft. |
| Konsole | — | Prüfbefehl zeigt in Fassung 1 keine Werte (A134 verlangt, dass Konsole und Prüfbefehl denselben Wert zeigen) | **7.** Die Werte folgen in einer späteren Fassung als Option `--werte`. |
| Läufer des Probelaufs | Befehl `werkbank-probelauf`, `drehbuch.py`, `konfig.json`, Vorlage der Einstellungen, Exitcodes 0 bis 3, Ergebnisdatei (Form dort) | Paketort, Eigentümer und Modus der Dateien, Ort der Ergebnisdatei (frei durch `--ergebnis`) | **9.** Die Vorlage der Einstellungen ist die Vorführdatei von Vertrag 2, im Arbeitsbaum `spec/einstellungen/vorfuehrung.json` (Nachtrag 20.11.2026; eine Datei `the previous implementation` gibt es nicht); der Läufer liest die Arbeitskopie unter `/var/lib/werkbank/<nutzer>/`. Nachtrag Erststart: Die Arbeitskopie kopiert `postinst` aus dem Erststart-Profil `erststart.json`, nicht aus dieser Vorlage; die Vorlage bleibt im Paket. Python 3 liegt in Ubuntu 26.04. |
| AppArmor (entschieden 20.11. V1, siehe OPEN 25) | Profiltext, Einrichtungsschritt, Probelauf, feste Codes (Entwurf auf 24.04.5) | Pfad, Name und Form des Profils im Paket (Form A oder B, Dateinamen `usr.lib.werkbank.*`) | **10. Gemessen.** Beide Formen sind auf 26.04.1 gemessen; es gilt Form A in der Fassung V1 (Dateiname und Text stehen im Abschnitt „Das AppArmor-Profil“). Form B bleibt als Fall. |
| Modellweg (Gateway) | `werkbank-modellweg`, vom Läufer je Sitzung gestartet (keine Benutzereinheit), Art der Schlüssel-Übergabe | Ort und Modus der Schlüsseldatei, Eigentümer, Regel „nie in Ausgabe, Protokoll oder Befehlszeile“ | **11.** Der Weiterreicher liest die Datei des Nutzers und die Adresse aus der Arbeitskopie der Einstellungen; ein Adressfeld gibt es in Vertrag 2 noch nicht. |
| Anmeldung der Copilot CLI (Betreiber) | — | `~/.copilot` ist die einzige Ausnahme im Heim beim Entfernen | **12.** Die Copilot CLI legt ihre Anmeldung nur dort ab. |
| Halter unter Linux (`rust/spec/halter/`, Nachtrag Halter) | Programm `werkbank-halter`, Ort und Regeln der Ablage, Form einer Datei der Ablage, Start-Exitcodes 5, 64, 74, 78 | Einheit `wb-halter.service`, Ort, Eigentümer und Modus von `halter.json`, `remove` und `purge` der Ablage | **15.** Der Halter legt fehlende Ebenen der Ablage selbst an und liest `halter.json` nur von `/etc/werkbank`. |
| Kern und Brücke | `werkbank-kern`, Brücke auf `127.0.0.1` mit Token | Benutzereinheit, Ort, Start durch den Läufer | **13.** Der Kern-Dienst läuft als Benutzereinheit, nicht als Systemdienst (Anmeldung der Copilot CLI im Heim). |
| macOS (Einschluss; Plattformschicht: Halter) | Einschluss laut macOS-Einschluss-Vertrag; Halter, Schlüsselbund-Eintrag | Prüfungen `launchd`, `schluesselbund`, `einschluss` | **14.** Der Dienst heißt `werkbank.halter`; der Eintrag im Schlüsselbund ist ohne Inhalt prüfbar. |
| Unterlagen (A136, A137) | — | die Tabellen der Zeilen und die Geheimnistabelle des Installers | **8.** Die Einrichtungsanleitung führt auf einen Prüfbefehl mit rc 0 oder 20. |

## Prüfbarkeit der Fälle

`pruefen.py` prüft heute (689 Fälle; alt: 669): Jede Datei in `faelle/` hat ein Gegenstück in `erwartet/`; der Code oder die Zeilen stimmen mit dem Referenzprüfer überein; das Schema nimmt jeden gültigen Manifest-Fall an, lehnt jeden Fall der Stufen 3 bis 13 ab (außer den Fällen, die das Schema nicht trennen kann) und nimmt jeden Fall der Stufen 14 bis 17 an; jeder Code der Tabellen dieser Datei steht in mindestens einem Fall (außer `pruef.intern`), und jeder Code eines Falls steht in den Tabellen.

Erst mit dem Bau prüfbar: die Rust-Umsetzung gegen dieselben Fälle (Crate `paket` mit Manifest, Listen, Stückliste, Steuerdatei, Entfernen und Einrichten; die Prüfbefehle); das Zusammenspiel mit echten Diensten; alle Außentests oben; die Messung von Frist und Dauer.

```sh
python3 -I rust/spec/paket/bau-faelle.py             # Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/paket/pruefen.py   # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

## Entschieden

- **Reihenfolge:** deb (M1), danach MSI, danach pkg. Alle drei teilen Manifest, Lieferumfang, Stückliste und Prüfbefehl.
- **Das AppArmor-Profil gilt nur für den festen Einschluss, nie für `/usr/bin/bwrap`** (Form A bevorzugt, Form B als Rückfall; die Wahl trifft die Messung auf 26.04.1). Ein Profil mit Anhang am System-`bwrap` ist ein Gate-Fehler (`paket.profil_system_bwrap`) und auf dem Rechner ein roter Befund.
- Das deb heißt `werkbank` (Arbeitsname), Architektur `amd64`, ohne Revisionsteil. Programme unter `/usr/lib/werkbank/`, feste Daten unter `/usr/share/werkbank/probelauf/`, alles je Nutzer Veränderliche unter `/var/lib/werkbank/<nutzer>/` mit Modus 0700 und dem Nutzer als Eigentümer (Projektentscheid).
- Die Dienste des deb sind Benutzereinheiten; das Paket legt kein Konto und keine Gruppe an.
- Die Signatur des Probelaufs ist eine Prüfsumme (sha256) über einen zweiten Weg; `gpg`-Signatur und apt-Spiegel kommen mit Fassung 2.
- Entfernt wird mit `apt purge`; die drei Ausnahmen sind die Ergebnisdatei, `~/.copilot` und die Systemprotokolle. Ein Fall belegt das.
- Der Schlüssel des Gateways steht nie in argv, Umgebung, Ausgabe, Protokoll, Ergebnisdatei oder Journal; die Datei hat Modus 0600 und gehört dem Nutzer; ein Fall je Stelle belegt das.
- Kein festes CA-Bündel: `ca-certificates` als Abhängigkeit, `webpki-roots` und `webpki-root-certs` in der Stückliste verboten.
- Das Windows-Paket ist eine MSI, gebaut mit WiX **5.0.2** (neueste Fassung ohne Wartungsgebühr; Quellen und Bedingung im Abschnitt „Entscheidung zu WiX“); MSIX entfällt aus den vier Gründen oben.
- `apt remove` lässt die Schlüsseldatei liegen und sagt es mit dem festen Satz `pruef.schluessel_rest`; `apt purge` löscht sie (Schlüssel ist Nutzerdaten).
- (Nachtrag Halter, 23.11.) Das deb liefert den Halter als Benutzereinheit `wb-halter.service` ohne eigenes Konto; `postinst` schreibt `/etc/werkbank/halter.json` (`root:root`, 0644). Die Anmeldung der Copilot CLI liegt als `anmeldung.copilot` in der Ablage des Halters im Heim; `remove` lässt sie liegen und sagt es, `purge` löscht sie als die Person.
- (Schritt II, Entscheid des Betreibers, 23.11.2026) Entscheidung 4 ist für den Systemnutzer `wb-halter` und drei Rollengruppen aufgehoben; `preinst` legt sie mit `adduser --system` an. `purge` löscht Konto und Gruppen nach Weg a: erst wenn `find -uid` und `-gid` nichts mehr finden und kein Prozess des Kontos läuft (Teil A2). Die Brücke (O1 oder O5) ist offen.
- Das macOS-pkg bleibt unsigniert und wird nicht notarisiert; die Anleitung nennt den Warndialog.
- Bei Gleichstand der Messung zum AppArmor-Profil gilt Form A.
- Ein Befehl `rust/paket.sh <deb|windows|macos>` baut; der Befehl der IT zum Installieren ist unter Windows `msiexec /i … /qn`, unter Ubuntu `sudo apt install ./werkbank_<version>_amd64.deb`, unter macOS `sudo installer -pkg … -target /`.
- Der Code 20 des A108 gehört dem Prüfbefehl, nicht `msiexec`.
- Der Prüfbefehl ist rot bei `nicht_pruefbar` (fail closed) und lässt bei jedem Befund die Zeile der Stelle weg.
- Das Manifest enthält keinen Zeitstempel. Ein reproduzierbarer Bau (gleiche Quellen, gleiche Prüfsummen) ist nicht zugesagt.
- Die Pfade im Manifest sind klein geschrieben (unter macOS auch groß); verglichen wird ohne Rücksicht auf die Schreibung.

## OPEN

1. **Namen mit den Bausteinen abstimmen:** `wb pruefen`, `wb lizenzen`, `wb stelle einrichten`, die Programmnamen und die Dienstnamen. Ändert ein Baustein einen Namen, ändern sich Pflichtdateien, Sätze und Fälle.
2. **Signatur.** Für Schritt I gilt die Prüfsumme (sha256) über einen zweiten Weg (Teil A; A71), unter Ubuntu und macOS. Die Signatur des deb kommt vor Schritt II. Der Signaturschlüssel und das Zertifikat für das deb ab Fassung 2 (`gpg`-Signatur über `SHA256SUMS`, eingebettete `debsigs`-Signatur oder ein signierter apt-Spiegel) und für die MSI (Authenticode) werden **vor Schritt II** geklärt (Projektentscheid). Windows Installer lehnt ein manipuliertes Paket nicht von sich aus ab, solange keine Richtlinie die Signatur verlangt (hergeleitet; AT-2 misst es). Fällt die Messung so aus, prüft die IT die Signatur vor der Installation (`Get-AuthenticodeSignature` oder der Weg der Softwareverteilung).
3. **WiX-Lizenz: entschieden (WiX 5.0.2, Abschnitt „Entscheidung zu WiX“).** Offen bleibt nur die Bestätigung des Betreibers, dass `deny.toml` für Bauwerkzeuge nicht gilt, solange keine Datei aus WiX in der MSI liegt; sonst gilt die Alternative dort.
4. **Bauhost.** Das deb baut ein Ubuntu 26.04 auf x86_64 (Container oder VM), das pkg ein Mac, die MSI ein Windows-Rechner. Die MSVC-Bauweise und `signtool` brauchen Windows. Ob der Bau auf einem Windows-Runner, in der VM oder auf einem anderen Windows-Rechner läuft, ist offen; ebenso, ob `paket.sh` unter Git Bash oder als PowerShell-Skript läuft.
5. **Vault-Ort und Sollrechte** (Annahme 4) und der Ort der Einstellungsdatei (Annahme 2).
6. **Version.** Die Obergrenzen 255.255.65535 stammen vom Windows Installer. Wie die Version aus Git-Ständen entsteht, ist offen.
7. **Installationsprotokoll.** Der Aufruf von `wb pruefen` am Ende der MSI ändert den Rückgabewert von `msiexec` nicht. Ob die IT ein eigenes kleines Startprogramm will, das beides verbindet, hängt von ihrer Softwareverteilung ab.
8. **Deinstallation und `%ProgramData%\Werkbank\`.** Ob Einstellungen, Protokoll und die Datei des Halters bleiben oder gehen, ist offen. Geheimnisse dürfen nicht unbemerkt bleiben, Nutzerdaten nicht ohne Wunsch verschwinden (A76).
9. **Update und Rückweg (A76).** Die MSI nutzt ein Hauptupgrade; der Rückweg auf die vorige Version ist nicht entworfen. Das ist Teil des Betriebs (Betrieb), nicht dieses Vertrags.
10. **Linux-Server.** Das deb dieses Vertrags ist für einen Ubuntu-Laptop mit einer Person (M1). Ein Linux-Server (A70, A71) mit Systemdiensten unter eigenen Konten ist eine spätere Fassung; die Prüfungen `halter` und `wfp` gelten dort anders.
11. **Rechte des Prüfbefehls.** Die Einzelprüfungen brauchen unter Windows teils Administratorrechte (Filter-Engine, ACL des Vaults). Läuft der Befehl ohne sie, meldet er `nicht_pruefbar` und rc 1. Ob ein Lauf als normaler Nutzer erwünscht ist, ist offen.
12. **Werte statt Zustände** (A134): Konsole und Prüfbefehl sollen denselben Wert zeigen. Fassung 1 gibt nur Zustände aus.
13. **Umfang der Sicherung.** Ob das Geheimnisfach des Halters in die Sicherung gehört (verschlüsselt) oder nach der Wiederherstellung neu angelegt wird (A07), entscheidet eine spätere Fassung mit dem Baustein Halter.
14. **Eingabe `konten`.** Was ein „menschliches Konto“ ist (Profil vorhanden, nicht gesperrt, keine Dienstkonten), legt der Baustein Stelle fest.
15. **Ubuntu 26.04.1 ist in Wegwerf-VMs gemessen, nicht auf einem Betriebsrechner.** Kernel, AppArmor-Fassung, Sperre der Namensräume, bwrap- und Python-Fassung, das Profil V1, der Probelauf, die Installation, der Bau und der Lauf mit `pi` sind dort gemessen (siehe „Was gemessen ist und was nicht“); aus dieser Liste ist nichts mehr hergeleitet. Offen bleibt der echte Betriebsrechner: Kernel, Richtlinien einer Organisation und Secure Boot können abweichen, und die Außentests AL-1 bis AL-10 laufen dort noch nicht.
16. **Abhängigkeiten ohne Netz.** `python3`, `bubblewrap`, `apparmor`, `ca-certificates`, `systemd` und `libssl3t64` müssen auf dem Laptop schon installiert sein, sonst braucht `apt` ein Netz. Ob `bubblewrap` zur Standardinstallation gehört, ist nicht belegt; der Betreiber prüft vorher `dpkg -s` (Anleitung, Schritt 2). Alternative: die fehlenden `.deb`-Dateien aus dem Ubuntu-Archiv mitliefern.
17. **Kern-Dienst als Benutzereinheit.** Ob der Läufer die Einheiten startet oder ob sie beim Anmelden anlaufen, und wie der Prüfbefehl sie für den Nutzer liest (`systemctl --user --machine=<nutzer>@` als root; hergeleitet), legt der Baustein Kern fest (Annahme 13).
18. **Weitere Nutzer.** Das Paket legt den Nutzerordner nur für den Nutzer an, der `sudo` aufruft. Ein zweiter Nutzer des Laptops braucht einen eigenen Schritt, der hier nicht beschrieben ist.
19. **`remove` gegen `purge`: entschieden (Projektentscheid).** `remove` lässt die Schlüsseldatei liegen und sagt es (`pruef.schluessel_rest`); `purge` löscht sie. Offen bleibt nur, ob `prerm` den Hinweis auch an `apt` zurückgibt, wenn `apt` ohne Terminal läuft (der Satz geht dann nur ins Journal).
20. **Aufruf ohne `sudo`-Nutzer** (etwa `pkexec`, `apt` als root ohne `SUDO_USER`): kein Nutzerordner; der Prüfbefehl meldet `pruef.nutzerordner_fehlt`.
21. **macOS: entschieden (Projektentscheid):** keine Notarisierung, kein Developer-Konto, das pkg bleibt unsigniert; die Anleitung nennt den Warndialog. Offen bleibt der genaue Wortlaut der Warnung je macOS-Fassung (AM-6 hält ihn fest).
22. **Umfang des macOS-Pakets.** Welche Teile des Teils A (Suche, Tor, Kette, Läufer) das erste macOS-Paket mitbringt, legt das Projekt fest.
23. **Entfernen unter macOS.** Ob `werkbank-entfernen` den Schlüsselbund-Eintrag löscht und wie die Listen vorher und nachher entstehen, ist offen.
24. **Ausschlussliste je Plattform.** Die Listen von Windows, Ubuntu und macOS sind getrennte Dateien; die Anfangsliste je Plattform schreibt der Bau.
25. **Profilform A oder B (entschieden 20.11.: A in der Fassung V1, Starter `werkbank-einschluss`, kein Profil am Pfad von `werkbank-lauf`, nie `unconfined`; der Text unten zur Wahl ist Herkunft).** Welche Form gilt, Dateiname und Text stehen im Abschnitt „Das AppArmor-Profil“ (gemessen auf 26.04.1). Ob der `dpkg`-Trigger für die Kopie (Form B) reicht, und ob der Übergang `Px` aus einem Profil mit `flags=(unconfined)` auf 26.04.1 greift, ist ungemessen. **Bei Gleichstand der Messung gilt Form A** (Projektentscheid: kein zweites `bwrap`-Binärprogramm im System).
26. **Quelle von `konfig.json` (Nachtrag 20.11.2026, entschieden nach dem Trockenlauf).** Ziel `usr/share/werkbank/probelauf/konfig.json`, Quelle `datei:spec/vorfuehrung/laeufer/konfig.probelauf.json`; nicht umbenannt, `konfig.attrappe.json` nie im Paket (Abschnitt „Der Läufer im Paket“). Der Vertrag erzwingt die Quelle mit keinem eigenen Code; sie steht in der Positivliste, die der Bau schreibt (nächster Paketschritt).
27. **Fassungen der Bauwerkzeuge (Nachtrag 20.11.2026).** Die Fassungen und Prüfsummen von `dpkg-dev` und `build-essential` sind die des Archivs `resolute` am 20.11.2026; ein Bauhost mit einem späteren Stand trägt seine eigenen ein. `deny.toml` regelt nur Bibliotheken im Paket; ob GPL-Bauwerkzeuge und MS-RL zusätzlich eine Bestätigung brauchen, steht in OPEN 3.
28. **Nachtrag Halter: Nachzug außerhalb dieses Vertrags.** (a) `lauf.json`, Eintrag `schluessel.copilot`, und der Modellweg
    (Vertrag 8) zeigen noch auf `/var/lib/werkbank/<nutzer>/copilot.schluessel`; der Zug, der den Modellweg auf den Halterweg
    stellt, nimmt den Eintrag heraus (Vertrag 1 und Vertrag 8). (b) Der Läufer liest „angemeldet“ heute aus der alten
    Datei; er braucht den neuen Ort oder `schluessel.vorhanden` über den Halter. (c) Die Form von `halter.json` hat noch kein Schema
    (Halter-Vertrag OPEN 21 b); bis dahin gibt es keine Fälle zum Inhalt, nur zu Ort, Eigentümer, Modus und `purge`. **(c) aufgelöst**
    (Nachtrag Rolle aus der Gruppe): Form `$defs/konfig`, Inhalt über `referenz.halter_konfig` und `test_bau.py`. (d) Code:
    `rust/paket/ubuntu-26.04-x64/` (`postinst`, `prerm`, `postrm`, die Einheit `wb-halter.service`, die Positivliste),
    `rust/crates/werkbank-pruefen` (Prüfung `copilot`, Einheit des Halters) und ein Crate `halter` mit `[[bin]] werkbank-halter`
    folgen diesem Stand in eigenen Zügen; ohne das Crate endet ein Bau ohne `--geruest` mit `paket.quelle_fehlt`. (e) Die
    Verbotsliste des Einschlusses führt die Ablage noch nicht (Halter-Vertrag OPEN 21 a).
29. **Nachtrag Halter: Grenzen.** Ein Nutzer, der `XDG_STATE_HOME` setzt, findet die Ablage trotzdem unter `<heim>/.local/state`
    (Halter-Vertrag 2.3); das ist gewollt. `purge` räumt die Ablage nur für den Nutzer aus `lauf.json` oder `SUDO_USER`; die Ablage
    eines zweiten Nutzers (OPEN 18) bleibt. Ist das Heim beim `purge` nicht eingehängt (etwa ein verschlüsseltes Heim ohne
    Anmeldung), bleibt die Ablage liegen, und eine Zeile geht ins Journal; das ist nicht gemessen. Schritt II (Systemnutzer, Halter-Vertrag OPEN 18) ändert Ablage, Einheit, `remove` und
    `purge` und ist hier nicht eingebaut; seine Fassung steht in Teil A2.
30. **Schritt II (Teil A2), offen:**
    (a) Die Brücke: O1 oder O5, nach L-1 bis L-3 (Abschnitt „Die Brücke“ in Teil A2). (b) Die Messungen L-1 bis L-8 und die
    Außentests AL-15 bis AL-18. (c) Die Hauptgruppe `wb-halter` ist eine vierte Systemgruppe neben den drei Rollengruppen; der
    Entscheid nennt sie nicht ausdrücklich. Ohne sie gälte `--ingroup nogroup`, und die Ablage gehörte `wb-halter:nogroup`.
    (d) `Pre-Depends: adduser` in der Steuerdatei; Alternative: Gruppen erst in `postinst` und setgid über `dpkg-statoverride`
    statt über das Archiv (dann prüft Stufe 9 einen Baum ganz ohne setgid und die Liste in `postinst`). (e) Folgen im
    Prüfbefehl ohne Fälle: Einheit des Halters als System-Einheit, Prüfung `copilot` am Ort `/var/lib/wb-halter/geheim`
    (nur mit root lesbar, ohne root `nicht_pruefbar`, oder über `schluessel.vorhanden`), neue Prüfung „kein Konto in einer
    Rollengruppe“. (f) Teil A lässt ein setgid-Bit in Stufe 9 durch (2755 ist nicht schreibbar); erst Teil A2 prüft es.
    (g) **Behoben (24.11.2026):** `find / -xdev` übersah Reste auf anderen Dateisystemen; `purge` sucht jetzt je Einhängepunkt (Schritt 3 in „`purge` in Schritt II“), unbekannte oder nicht lesbare Punkte sind ein Rest. **Was die Suche trotzdem nicht findet (hergeleitet, nicht gemessen):** (1) Dateien in `tmpfs`, `ramfs` und anderen ausgenommenen Typen; sie sind nach dem nächsten Start weg, aber bis dahin kann eine neu vergebene uid sie sehen. (2) Dateien auf Netz-Dateisystemen (`nfs`, `cifs`) und in einer VM-Freigabe (`9p`). (3) Dateien, die ein späterer Einhängepunkt verdeckt (liegt ein Rest unter `/srv/beispiel/daten` auf der Wurzel und dort ist später etwas eingehängt, sieht `find` nur das Eingehängte). (4) Einhängepunkte in einem fremden Namensraum (Container, `PrivateMounts`), die `findmnt` im Namensraum von `purge` nicht zeigt. (5) Ein Punkt, der zwischen `findmnt` und `find` verschwindet (zählt als Rest; das Konto bleibt, auch wenn nichts zurückblieb). (6) Die Typlisten sind hergeleitet; ein Typ, den ein Rechner nutzt und der fehlt, hält das Konto fest (fail closed), bis die Liste ihn aufnimmt. Die Messung gehört in den Außentest AL-17. (h) Ein schon bestehendes Systemkonto `wb-halter` in
    richtiger Form übernimmt `preinst`, und `purge` löscht es danach; ein Fall dazu fehlt. (i) Code, Positivliste,
    `preinst`, `tmpfiles.d` und die Einheit folgen in eigenen Zügen; Halter 2.3 Fassung 2 folgt mit Zug 3.
31. **Nachtrag Rolle aus der Gruppe, offen.** (a) Kopfname `authorization` und Vorsatz `"Bearer "` der Copilot-Ziele in `halter.json`
    sind nicht gemessen; ein Lauf mit Konto muss sie bestätigen. (b) Die Namen der Ziele (Host des Netzziels) muss der Bau der Zielliste
    (Vertrag 8, `halter_ziel`) übernehmen; das Beispiel dort nennt andere Namen. (c) Gruppen aus NSS (LDAP, SSSD) sieht weder der Halter
    noch der Prüfbefehl (Halter-Vertrag OPEN 23). (d) Eine Datei außerhalb des Pakets mit der gid von `wb-bruecke` sucht `purge` in Schritt
    I nicht; nach `delgroup` könnte eine neue Gruppe diese gid erben (hergeleitet). (e) Der Modellweg muss als setgid-Programm im sicheren
    Modus laufen (Halter-Vertrag 2.3); die Liste der Stellen gehört Vertrag 8. Bis dahin gilt die Rolle `bruecke` für jeden Aufrufer des
    Modellwegs, der ihn mit eigener Umgebung und eigenen Argumenten startet.
32. **Nachtrag Erststart, offen.** (a) `usr/share/werkbank/probelauf/erststart.json` ist keine Pflichtdatei des Manifests; die Liste
    der Pflichtdateien liegt im Prüfbefehl (`werkbank-pruefen`, Folgezug Code). (b) Die Konfiguration des Läufers
    (`konfig.probelauf.json`) startet die Cloud-Sitzung mit `start.cloud.modell` `auto`; die Arbeitskopie aus dem Profil trägt an der
    Route `copilot` das Modell `gpt-6-luna`, und der Start endet dann mit `start.modell_route` (Vertrag 1). Der Läufer muss
    `gpt-6-luna` wählen (Folgezug Vorführung, hergeleitet, nicht gemessen). (c) Eine Arbeitskopie aus einem früheren Paket bleibt
    bei einem Update liegen (Schritt 2 überschreibt nie); sie trägt dann die Vorführ-Einstellungen. (d) Wann der Schalter
    `erststart_antwortform` wieder wegfällt, entscheidet die Leitung; `werkbank-pruefen` meldet ihn noch nicht als Befund
    (Halter-Vertrag OPEN 24).
33. **Ort der Gespräche, Nachtrag 27.11.2026, offen.** Gewählt ist **Form A**: `/var/lib/werkbank/<nutzer>/gespraeche`, nach dem Namen wie `zustand`, `einstellungen` und `protokoll`. Grund: Der Vertrag `start` legt die Gespräche in den Zustandsordner der Installation; der Elternordner gehört dem Nutzer (0700), so dass `lauf` als der Nutzer dort anlegen kann und `purge` sie mit dem Zustandsordner entfernt. `lauf.json` ist eine Datei je Installation für genau einen Nutzer, ein Name im Pfad ist darin wie bei `zustand` und `ziellisten` üblich. **Form B**: `/var/lib/werkbank/g/<uid>` nach der uid, wie `sitzungen` (`s/<uid>`) und `laufzeit`, mit `/var/lib/werkbank/g` als `root:root` 0755 und einem neuen Code für `g`. Sie hielte die Gespräche außerhalb des geschützten Zustandsordners und bräuchte keine Ausnahme im Einschluss, ließe aber `purge` ein eigenes Ziel räumen und trennte Gespräche vom Zustand des Nutzers. Wer die Form B will, ändert die Zeile `gespraeche` von `lauf.json`, `lauf.gespraeche` von `kern.json`, Code 30 und die Fälle `dienstkonfig-*` und `laufkonfig-*`. Ebenfalls offen: Ob `postinst` den Ordner anlegen soll (gewählt, Code 30) oder allein „Lauf starten“ (`lauf` legt ihn mit 0700 an, wenn er fehlt; eine fremde Form endet dann bei jedem Start mit 65 oder 70, nicht bei der Installation).

33. **Teil A3 (Schreibhelfer), offen.** (a) **Geschlossen (Entscheid des Betreibers, 27.11.2026):** Der Ordner
    `/etc/werkbank/vorgabe/` gehört `werkbank-vorgabe` mit Modus `0755`, alle Eltern ab `/etc/werkbank` gehören root ohne `& 022`;
    kein Gruppenschreibrecht, kein `CAP_DAC_OVERRIDE` (Teil A3, Entscheidung 4). (b) **Geschlossen (Entscheid des Betreibers,
    27.11.2026):** `postinst` trägt das installierende Konto als ersten Admin ein, nur wenn noch keine Firmenvorgabe da ist; ohne
    installierendes Konto entsteht keine Datei (Code 28; Teil A3, „Erster Admin“). Der Code steht in `postinst`; `test_bau.py` prüft
    V-6, V-7 und V-12 bis V-17. (c) Die
    Prüfung `dienste` von `werkbank-pruefen` kennt `werkbank-vorgabe.socket` noch nicht. (d) Die Positivliste trägt die vier neuen Pfade, das Manifest
    entsteht aus ihr. Die Pflichtdateien des Manifests (`referenz.PFLICHTDATEIEN`) nennen sie noch nicht; das änderte die Fälle
    `manifest-*`. Empfehlung: mit dem nächsten Zug der Fälle aufnehmen. (e) Ob das deb Teil A3 mit Schritt I oder erst mit Schritt II trägt, entscheidet der
    Bau. (f) Zwei Dateien gleichen Namensstamms, `/etc/werkbank/vorgabe.json` und `/etc/werkbank/vorgabe/`, liegen nebeneinander.
    Empfehlung: den Ordner des Helfers mit dem Bau in `/etc/werkbank/firmenvorgabe/` umbenennen, falls die Verwechslung stört; hier
    bleibt der Name, damit Fälle und Vertrag zueinander passen.

## Quellen für Lizenz- und Formatfragen (Stand 20.11.2026)

Die Quellen zur Wahl der WiX-Fassung (selbst geladen, mit Datum) stehen im Abschnitt „Entscheidung zu WiX“. Die folgenden Seiten stammen aus der ersten Recherche und wurden dort nur über eine Suche gesehen:

- WiX und OSMF: [About the WiX Toolset](https://docs.firegiant.com/wix/), [Open Source Maintenance Fee](https://docs.firegiant.com/wix/osmf/), [WiX v6.0.0 available](https://www.firegiant.com/blog/2025/4/7/wix-v600-available/).
- MSIX und Dienste: [desktop6:Service](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-desktop6-service), [Convert an installer that includes services](https://learn.microsoft.com/en-us/windows/msix/packaging-tool/convert-an-installer-with-services), [Restricted Capabilities (Revenera)](https://docs.revenera.com/installshield/helplibrary/IHelpRestricted_Capabilities.htm).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `agent-workbench` `pi` `wb-hub`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
