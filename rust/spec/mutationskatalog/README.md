# Vertrag: Mutationskatalog und Prüfung jeder gelesenen Datei (Fassung 1, Entwurf)

Dieser Vertrag schließt die Lücke B38: A23 (Mutationskatalog) und A25 (Prüfung jeder gelesenen Datei auf
Eigentümer und Modus) hatten keinen Vertrag. Er gilt vor Schritt II.

Stand: 23.11.2026. Grundlage ist `development` (Stand dieses Zweigs). Fundstellen ohne Präfix liegen unter
`rust/`. Beispielpfade stehen in der Form `/srv/beispiel/...`.

Der Vertrag beschreibt zwei Dinge, die zusammengehören: Teil A legt fest, wie eine Sicherheitsregel ihren
Mutanten bekommt und wie der Katalog rot wird; Teil B legt die Regel fest, die Teil A für A25 prüfbar macht.

---

# Teil A: Der Mutationskatalog (A23)

## A1. Was der Katalog ist

Der Katalog ist eine Datei `katalog.json` neben diesem Vertrag. Sie nennt zu jeder Sicherheitsregel
mindestens einen Mutanten: eine genau benannte Änderung an einer genau benannten Stelle im Quelltext und den
Test, der dadurch rot wird.

Der Katalog ersetzt `cargo mutants` nicht (Abschnitt A6). Er beantwortet die Frage, die `cargo mutants` nicht
beantwortet: hat **jede** Sicherheitsregel eine Gegenprobe, und zeigt jede Gegenprobe noch auf die Stelle, für
die sie geschrieben wurde?

## A2. Form eines Katalogeintrags

`katalog.json` ist ein JSON-Objekt mit drei Schlüsseln.

```json
{
  "fassung": 1,
  "regeln": [
    { "id": "R-KERNABBILD", "titel": "...", "anforderung": "A23" }
  ],
  "eintraege": [
    {
      "id": "K01",
      "regel": "R-KERNABBILD",
      "anforderung": "A23",
      "datei": "rust/crates/kernabbild/src/lib.rs",
      "funktion": "entscheiden",
      "anker": "Muster::Inhalt(roh) if roh.first() == Some(&b'|')",
      "mutant": "Die Bedingung wird zu `roh.first() != Some(&b'|')`.",
      "roter_test": "kernabbild: harte_null_mit_pipe_am_anfang_bricht_ab"
    }
  ]
}
```

Jedes Feld ist Pflicht und ein nicht leerer Text:

| Feld | Bedeutung |
|---|---|
| `id` | Kennung des Eintrags, Form `K` und zwei Ziffern, im Katalog einmalig. |
| `regel` | Die `id` einer Regel aus `regeln`. Die Regel ist die Sicherheitsregel, nicht der Test. |
| `anforderung` | Die A-Nummer, die die Regel verlangt (Form `A` und zwei oder drei Ziffern). |
| `datei` | Pfad des Quelltexts, relativ zur Wurzel des Repos, ohne führenden Schrägstrich und ohne `..`. |
| `funktion` | Name der Funktion, in der der Anker steht. |
| `anker` | Der Quelltext, den der Mutant ändert: mindestens 12 Byte, eine Zeile, wörtlich aus `datei`. |
| `mutant` | Die genaue Änderung in einem Satz. Nicht „Prüfung entfernen“, sondern was danach dasteht. |
| `roter_test` | Der Test, der nach dieser Änderung rot wird, als `<Crate>: <Testname>` oder `<Crate>: <Fall>`. |

`regeln` trägt je Regel `id`, `titel` und `anforderung`.

## A3. Die Regel „trifft genau einmal“

Der Anker ist die Stelle. Er muss in `datei` **genau einmal** vorkommen, und die nächste Zeile `fn <name>`
oberhalb des Ankers muss `funktion` sein.

Daraus folgen die Grenzen für einen Anker:

- Ein Anker ist eine einzelne Zeile ohne Zeilenumbruch. Je mehr Zeilen ein Anker umfasst, desto häufiger
  reißt er bei Änderungen, die den Befund nicht berühren (gemessen in einem anderen Projekt, 27.08.2026).
- Ein Anker ist lang genug, um einmalig zu sein, und kurz genug, um eine Umbenennung in der Nachbarschaft zu
  überleben. Zwölf Byte sind die untere Grenze, nicht das Ziel.
- Ein Anker enthält keinen Namen, der nur in einem Kommentar steht: er ist Code.
- Zwei Einträge dürfen denselben Anker nicht teilen. Zwei Mutanten derselben Stelle brauchen zwei Anker.

## A4. Was „ohne Ziel“ heißt und wie der Katalog rot wird

Ein Mutant ist **ohne Ziel**, wenn mindestens eines gilt:

1. `datei` gibt es nicht.
2. Der Anker kommt in `datei` nicht vor (die Stelle ist weggefallen oder umgeschrieben).
3. Der Anker kommt mehr als einmal vor (die Stelle ist nicht mehr eindeutig).
4. Die Funktion oberhalb des Ankers ist nicht `funktion` (der Code ist verschoben).

Ein Mutant ohne Ziel macht den Katalog rot: `katalog-pruefen.py` schreibt je Fall eine Zeile `FUND:` und endet
mit Rückgabecode 3. Rot ist rot. Ein Eintrag wird nicht entfernt, um den Katalog grün zu machen; er wird auf
die neue Stelle gezogen, oder sein Wegfall wird im Ergebnis mit seinem Ersatz genannt.

Rot wird der Katalog außerdem, wenn eine Regel aus `regeln` keinen Eintrag hat (A23: jede Sicherheitsregel hat
Fälle) oder wenn ein Pflichtfeld fehlt.

## A5. Das Prüfwerkzeug

```
/usr/bin/python3 -I rust/spec/mutationskatalog/katalog-pruefen.py rust/spec/mutationskatalog/katalog.json
```

Ohne `--wurzel` nimmt das Werkzeug die Wurzel des Repos aus seinem eigenen Ort (drei Ebenen über sich). Es
liest nur, schreibt nichts und braucht kein Linux.

| Rückgabecode | Bedeutung |
|---|---|
| 0 | Jeder Anker trifft genau einmal, jede Regel hat einen Eintrag. |
| 2 | Aufruf oder Form des Katalogs falsch (kein Befund über den Quelltext). |
| 3 | Mindestens ein Befund (`FUND:`). |

Die Gegenprobe des Werkzeugs liegt in `gegenproben/`: `anker-doppelt.json` (ein Anker steht siebenmal),
`anker-fehlt.json` (ein Mutant ohne Ziel) und `anker-verschoben.json` (der Anker steht in einer anderen
Funktion als der Eintrag nennt). Jede muss 3 ergeben.

```
bash rust/spec/mutationskatalog/gegenprobe.sh
```

Das Skript fährt vier Läufe (den Katalog und die drei Gegenproben), vergleicht jeden Rückgabecode mit der
Erwartung und endet mit 1, sobald einer abweicht. Es ruft das Werkzeug nie hinter einer Pipe auf.

Der erste Katalog trägt 19 Einträge über 10 Sicherheitsregeln (Abschnitt A8.2 nennt, warum gerade diese).

## A6. Wie der Katalog neben `cargo mutants` läuft

Beide laufen, keiner ersetzt den anderen.

| | `cargo mutants` (`gates.sh mutants`) | Mutationskatalog |
|---|---|---|
| Frage | Überlebt irgendein erzeugter Mutant? | Hat jede Sicherheitsregel eine Gegenprobe, und trifft sie noch? |
| Umfang | Der ganze Arbeitsbereich, Mutanten erzeugt das Werkzeug | Die Sicherheitsregeln, Mutanten schreibt der Mensch |
| Lauf | Baut und testet je Mutant, Minuten bis Stunden | Liest Text, Sekunden |
| Rot | Ein Mutant überlebt, läuft in die Zeit, der Grundlauf fällt, oder kein Mutant wurde gefangen (`gates.sh:514`) | Abschnitt A4 |

**Einschluss-Pflicht.** Code, der löscht, Signale sendet oder behandelt, Rechte ändert oder Pfade schreibt,
läuft unter `cargo mutants` und bei jeder Gegenprobe mit verändertem Code **nur** in einem Einschluss, in dem
außer einem eigenen Wurzelordner nichts schreibbar ist und keine fremden Prozesse sichtbar sind
(Projektentscheid, 21.11.2026; Vorfall vom 21.11.2026: ein Mutant stieg über `..` aus seinem Testordner und
löschte fremde Dateien). Betroffen sind heute mindestens:

| Art | Stellen |
|---|---|
| Löschen über Deskriptoren | `crates/formate/src/kette/loeschen.rs`, `crates/modellweg/src/loeschen.rs`, `crates/rangkern/src/sitzung/loeschen.rs` |
| Rechte und Pfade | `crates/modellweg/src/schluessel.rs`, `crates/modellweg/src/ca.rs`, `crates/modellweg/src/suche.rs`, `crates/formate/src/kette/monate.rs`, `crates/formate/src/kette/schreiber.rs`, `crates/rangkern/src/sitzung/mac.rs` |
| Signale und Prozessgrenzen | `crates/kernabbild/src/lib.rs` (`setrlimit`, `prctl`), `crates/lauf/` |

Der Katalog selbst braucht keinen Einschluss: er liest Text und ändert keinen Code. Nur das **Ausführen**
eines Eintrags (den Mutanten wirklich einsetzen und den roten Test messen) fällt unter die Einschluss-Pflicht.

**Reihenfolge eines vollen Mutationsbelegs.** Drei Läufe, und die Meldung nennt alle drei (Lehre vom
13.10.2026: eine Mutationsprobe ohne Kontrolllauf misst den Prüfaufbau):

1. Der unmutierte Lauf. Er muss grün sein, bevor eine Mutationszahl etwas bedeutet.
2. Die Katalogprüfung nach A5. Sie muss 0 ergeben.
3. Der Mutationslauf. Eine Zahl ohne 1 und 2 ist keine Verifikation.

## A7. Abnahme (A23)

Der Katalog ist abgenommen, wenn alles davon gemessen vorliegt:

1. `katalog-pruefen.py` auf `katalog.json` endet mit 0 und nennt je Eintrag eine Zeile.
2. Die Gegenprobe: `gegenprobe.sh` endet mit 0; jede der drei Dateien in `gegenproben/` ergibt 3, darunter
   der absichtlich verschobene Mutant.
3. Jede Regel in `regeln` hat mindestens einen Eintrag, und jede Sicherheitsregel der erfassten Crates steht in `regeln`.
4. Für die fünf Teile des Agenten-Trägers (Ticketkern, Freigaben, Herkunft, Sperren, Modellproxy) steht je
   mindestens ein Eintrag, dessen `roter_test` wirklich rot wird, wenn der Mutant eingesetzt wird.
5. Der Eintrag eines neuen Sicherheitscodes kommt mit dem Code, nicht nach ihm.

Punkt 4 ist in dieser Fassung **nicht** erfüllt: Ticketkern und Modellproxy des Trägers haben in
`katalog.json` noch keinen Eintrag (Abschnitt A8).

## A8. Bestand am 23.11.2026 (gemessen durch Lesen)

### A8.1 Was heute Mutanten hat

| Sicherheitsregel | Mutanten heute | Fundstelle | Art |
|---|---|---|---|
| Gesamtbau | `cargo mutants --workspace`, grün erst mit mindestens einem gefangenen Mutanten | `gates.sh:504-519` | Werkzeug, ohne Katalog |
| Ausnahme des Mutationstors | `exclude_re = ["portable_key"]`, offen bis zur Windows-Prüfung | `.cargo/mutants.toml:5`, `README.md:187-194` | Ausnahme mit Bedingung |
| Lecksperre im Modellweg (WebSocket, Umgebung, Schlüsselsuche, Zählung) | M-W1 bis M-W7, M-Z0 bis M-Z4, M-F1a bis M-F1c, M-S1 bis M-S4 über `BAU_MUTANT=<Name>` | `spec/modellweg/README.md:629-692`, `spec/modellweg/bau-faelle.mjs:60-63` | Handmutanten im **Erzeuger**, nicht im Crate |
| Lecksperre, Gegenprobe E5 | Mutanten 1 bis 5 (Einbindung und Pfadliste) | `spec/modellweg/README.md:642-650` | Außentest, Linux |
| Ordnermarke des Protokolls | M-O1 bis M-O3 über `ORDNERMARKE_MUTANT=O1|O2|O3` | `spec/protokollzeile/README.md:411`, `spec/protokollzeile/bau-faelle.mjs:145-169` | Handmutanten im **Erzeuger** |
| Kern-Dienst, Brücke, Träger, Rasterauszug, Zustandskern | Mutanten der **Referenz** über `REF_GEGENPROBE=<name>`, geprüft von `gegenprobe.sh` | `spec/dienst/VERTRAG.md:420-425`, `spec/bruecke/VERTRAG.md:254-269`, `spec/traeger/VERTRAG.md:359`, `spec/rasterauszug/VERTRAG.md:278`, `spec/zustandskern/VERTRAG.md:446` | Mutanten des Python-Modells, nicht des Rust-Codes |
| Kernabbild im Dienst | `cargo mutants -p dienst --in-diff` im Einschluss, 36 Mutanten, 0 verpasst | `spec/dienst/VERTRAG.md:796` | Lauf, im Vertrag zitiert |
| macOS-Einschluss | Mutanten nur als Text in vier Gegenprobe-Fällen (M14 bis M17) | `spec/macos-einschluss/README.md:553-555`, `spec/macos-einschluss/bau-faelle.py:473-508` | Beschreibung, kein Mechanismus |
| Datentor (Klassen, Person, Zonen) | „voller Mutationslauf ohne überlebende Mutanten“ verlangt, keine benannten Mutanten | `spec/datentor/VERTRAG.md:150`, `klassen/VERTRAG.md:245`, `person/VERTRAG.md:164`, `zonen/VERTRAG.md:203` | Auflage ohne Katalog |
| Einstellungen, Halter | „Mutanten folgen mit dem Bau“ | `spec/einstellungen/README.md:9`, `spec/halter/README.md:11` | offen |
| Stelle (Fassung 1 und 2), Formate, Kette, Kernabbild als Crate, Rangkern | kein benannter Mutant | — | **Lücke** |

Gemessen: `git grep -n -i "mutant"` über `spec/`, `crates/*/README.md`, `README.md`; `.cargo/mutants.toml`;
`gates.sh`. Das Wort „Mutationskatalog“ kommt auf `development` außer in diesem Vertrag nicht vor.

### A8.2 Was daraus folgt

1. Es gibt heute **keinen** Katalog, der einen Mutanten an eine Stelle im Rust-Quelltext bindet. Die
   benannten Mutanten liegen in zwei Erzeugern (`bau-faelle.mjs`) und in Python-Referenzen; sie prüfen das
   Modell, nicht den Bau.
2. `gates.sh mutants` läuft ohne Katalog. Er fängt, dass ein erzeugter Mutant überlebt, nicht, dass eine
   Sicherheitsregel gar keine Gegenprobe hat.
3. Die Sicherheitsregeln der Crates `stelle`, `formate`, `kernabbild` und `rangkern` hängen allein an
   `cargo mutants`. Für sie legt `katalog.json` die ersten Einträge an.

## A9. Welche Anforderungen einen Eintrag verlangen

Neun A-Nummern verlangen einen Mutationslauf wörtlich. Jede beschreibt denselben Fall: eine Prüfung wird
entfernt, und ein benannter Test wird dadurch rot. Genau das ist ein Katalogeintrag, also gehört jede dieser
Nummern in den Katalog, sobald ihr Code auf `development` liegt.

| Anforderung | Was der Mutationslauf entfernt | Eigentümer | Eintrag heute |
|---|---|---|---|
| A23 | — (die Anforderung des Katalogs selbst) | Prüfwerkzeuge | der ganze Katalog |
| A118 | 37 Mutanten der Verträge R5 und R13 des Zustandskerns, alle machen Fälle rot | Zustandskern | nein, Code nicht in den Crates dieses Vertrags |
| A139 | die Prüfung auf doppelte Schlüssel in **jedem** Lader | Einstellungen, Halter | `K18` (`einstellungen`); die anderen Lader fehlen |
| A140 | die Vorprüfung im Sicherungsbefehl, so dass eine Sicherung den Stand ersetzt | Prüfwerkzeuge | nein, kein Vertrag (B36) |
| A141 | die Prüfung gegen die Sperrliste im Lader des Halters | Halter | nein, kein Code auf `development` |
| A142 | das macOS-Profil, das den Heimordner lesbar macht (alter Stack: 39 Mutationen) | Einschluss | nein, nur als Text in M14 bis M17 |
| A152 | die Prüfung von Branchname und Ticketschlüssel im Push-Tor | Push-Tor | nein, kein Vertrag (B07) |
| A153 | die Profilprüfung der Netzgrenze, so dass CUSTOMER fremde Ziele erreicht | Einschluss | nein, kein Vertrag (B35) |
| A154 | eine Gruppe aus der Review-Vorgabe, so dass ein Paket ohne Review abschließt | Paket | nein, kommt nach Schritt II |
| A155 | die Prüfung der Basis-Branches im Push-Tor; ein zweiter Mutant bindet das Referenzrepo schreibbar ein | Push-Tor | nein, kein Vertrag (B07) |

Daraus folgt die Pflicht für jede Änderung, die eine dieser Prüfungen baut: der Katalogeintrag kommt mit dem
Code, nicht nach ihm (Abnahme A7, Punkt 5). Der Katalog kann die Zeile nicht erzwingen, solange der Code
fehlt; diese Tabelle ist die Liste, gegen die eine Änderung beim Bau prüft.

---

# Teil B: Prüfung jeder gelesenen Datei (A25)

## B1. Die Regel

Jede Datei, die ein Werkzeug aus dem Satz der Stelle liest, wird **vor dem Lesen** auf Eigentümer und Modus
geprüft, nicht nur ihr Ordner. Die Prüfung läuft auf dem **offenen Deskriptor**, nicht auf dem Pfad: ein
Pfad, der zwischen Prüfung und Lesen getauscht wird, ist keine Sperre. Jede Ausnahme von der Grenze „kein
Eingriff in die Stelle“ ist eine benannte Regel mit Test; eine Ausnahme ohne benannte Regel macht die Prüfung
rot.

Fail closed: lässt sich eine der Eigenschaften nicht feststellen, wird die Datei abgelehnt.

## B2. Der Satz von Schritten, je System

Reihenfolge ist Teil der Regel.

**Linux und jedes andere Unix**

1. Öffnen mit `O_NOFOLLOW`, `O_CLOEXEC` und `O_NONBLOCK` (eine Verknüpfung wird abgelehnt, eine FIFO blockiert
   den Start nicht).
2. `fstat` auf dem **offenen** Deskriptor, nie `stat` auf dem Pfad.
3. Reguläre Datei: alles andere (FIFO, Socket, Gerät, Ordner) wird abgelehnt.
4. `uid` gleich der eigenen euid.
5. Modus genau der erwartete, `modus & 0o7777` verglichen, also auch ohne setuid-, setgid- und sticky-Bit.
   Für eine Datei des Satzes ist der erwartete Modus `0600`, für einen Ordner `0700`.
6. Lesegrenze: die Datei wird mit `take(grenze + 1)` gelesen, damit „genau an der Grenze“ und „zu groß“
   unterscheidbar bleiben.
7. Einen Ordner betritt das Werkzeug mit `openat` relativ zum Deskriptor des Elternordners; ein Name mit
   Schrägstrich oder `..` wird vorab abgewiesen.

Vorbild im Bestand: `crates/modellweg/src/schluessel.rs:73-92` mit `datei_ok` (`:96-99`).

**macOS: zusätzlich zu Linux**

8. Zugriffsliste (ACL): Eine Datei, die über eine ACL einer fremden Kennung lesbar oder schreibbar ist, wird
   abgelehnt, auch wenn der Modus stimmt (`chmod +a`). Projektentscheid, 20.11.2026; das Verhalten der ACLs
   ist hergeleitet, der Test ist neu.
9. Gruppe: Die Gruppe der Datei wird geprüft. Eine Datei, deren Gruppe über die eigene hinausgeht und die für
   die Gruppe Rechte trägt, wird abgelehnt. Mit dem Modus `0600` ist Punkt 9 schon erfüllt; er gilt für jeden
   Satz, der einer Gruppe etwas erlaubt.
10. Der Elternordner wird weiter geprüft (Ordner, keine Verknüpfung, eigene uid, Modus `0700`) —
    **zusätzlich** zur Datei, nicht an ihrer Stelle. Vorbild: `crates/rangkern/src/sitzung/mac.rs:57-75`.

**Windows**

11. Der Satz steht noch nicht fest (B40: die Orte der Dateien je Betriebssystem sind offen). Bis dahin gilt:
    eigener Besitzer (`OWNER` des Sicherheitsdeskriptors gleich dem eigenen Konto), keine DACL-Einträge für
    andere Konten außer `SYSTEM` und `Administrators`, kein Weiterleiten über einen Reparse Point (Öffnen mit
    `FILE_FLAG_OPEN_REPARSE_POINT` und Ablehnen, wenn das Ziel einer ist), Prüfung auf dem Handle. Diese
    Fassung ist ein Entwurf und braucht eine Messung in der Windows-VM.

## B3. Bestand und Lücken am 23.11.2026 (gemessen durch Lesen)

Jede Zeile ist eine Stelle, die im Betrieb eine Datei oder einen Ordner liest; Teststellen sind nicht
aufgeführt. „fstat nach open“ heißt: die Prüfung läuft auf dem offenen Deskriptor.

| Crate | Datei:Zeile | Funktion | Liest | `O_NOFOLLOW` | fstat nach open | uid | Modus | ACL/Gruppe | Befund |
|---|---|---|---|---|---|---|---|---|---|
| modellweg | `schluessel.rs:73-92` | `lies` | Schlüsseldatei | ja | ja | ja | `0600` genau | nein | **Vorbild**; Lücke nur macOS (B2 Punkt 8, 9) |
| modellweg | `ca.rs:42-53` | `lies_datei` | Schlüssel und Zertifikat der Sitzung | ja | ja | ja | `0600` genau | nein | wie oben |
| modellweg | `suche.rs:181-193` | `liste_lesen` | Zielliste | ja | ja | ja | `0600` genau | nein | wie oben |
| modellweg | `suche.rs:316-330` | `eintrag` | Baum der Suche | ja (`statat` + `openat`) | ja (`is_file`) | nein | nein | nein | Absicht: die Suche liest fremde Dateien. Als Ausnahme benennen (B4) |
| kernabbild | `lib.rs:240-250` | `muster_aus` | `/proc/sys/kernel/core_pattern` | **nein** | nein | nein | nein | nein | **Lücke**: `File::open` auf dem Pfad; ein Verweis wird verfolgt |
| formate | `kette/pruefen.rs:415` | `datei_mit` | Kettendatei eines Monats | **nein** | nein | nein | nein | nein | **Lücke** |
| formate | `kette/quelle.rs:32` | `anker_lesen` | Ankerdatei | **nein** | nein | nein | nein | nein | **Lücke** |
| formate | `kette/monate.rs:196` | `anker_aus_datei` | Monatsdatei | **nein** | nein | nein | nein | nein | **Lücke** |
| formate | `kette/monate.rs:293` | `wechsel_entscheiden` | Monatsdatei | **nein** | nein | nein | nein | nein | **Lücke** |
| formate | `kette/monate.rs:208-214` | `monat_oeffnen` | Monatsdatei zum Anhängen | **nein** | nein | nein | nur beim Anlegen `0600` | nein | **Lücke**: der Modus einer **vorhandenen** Datei wird nicht geprüft |
| formate | `kette/monate.rs:43` | `monatsnamen` | Ordner der Kette | nein (`read_dir` auf dem Pfad) | nein | nein | nein | nein | **Lücke** |
| formate | `kette/monate.rs:336` | `unter_sperre` | Längen der Monatsdateien | nein (`fs::metadata`) | nein | nein | nein | nein | **Lücke**: Prüfung über den Pfad |
| formate | `kette/schreiber.rs:88-96` | `anhaengen_gesperrt` | Kettendatei | **nein** | nein | nein | nur beim Anlegen `0600` | nein | **Lücke** wie `monat_oeffnen`; die Sperre (`flock`) ersetzt die Prüfung nicht |
| rangkern | `sitzung/einst.rs:76` | `lesen_datei` | Einstellungsdatei des Laufs | **nein** | nein | nein | nein | nein | **Lücke**, die schwerste: `mac.rs` prüft den **Ordner**, niemand die Datei. Genau der Fall von A25 |
| rangkern | `sitzung/mac.rs:57-75` | `ordner_pruefen` | Elternordner des Sockets | Verweis abgelehnt (`symlink_metadata`) | nein | ja | `0700` genau | nein | Prüft nur den Ordner; ACL und Gruppe fehlen |
| rangkern | `vault.rs:45, 147, 199, 391, 405` | `is_source_space`, `read_lock`, `read_contradictions`, `load_query_vector`, `load_vectors` | Dateien des Speichers | nein (`fs::read`) | nein | nein | nein | nein | **Lücke** |
| rangkern | `serve.rs:342, 354` | `refresh` | Vektordatei | nein | nein (`fs::metadata` auf dem Pfad) | nein | nein | nein | **Lücke**; die Marke (`Stamp`) prüft Änderung, nicht Eigentum |
| rangkern | `fulltext.rs:66` | Rückfall der Volltextsuche | Notiz | nein | nein | nein | nein | nein | **Lücke** |
| rangkern | `vdb.rs:58, 194, 200, 202` | `is_wal`, `stamp_of`, `bytes` | Index und Datenbank | nein | nein | nein | nein | nein | **Lücke** |
| rangkern | `warm.rs:95` | Aufwärmen | Marke der Datei | Verweis abgelehnt | nein | nein | nein | nein | **Lücke** |
| stelle | — | — | **keine Datei** | — | — | — | — | — | Gemessen: kein `fs::`, kein `File::open` im Crate. Der Aufrufer liest; A25 liegt bei ihm |
| einstellungen | — | — | **keine Datei** | — | — | — | — | — | wie `stelle`: nur Bytes hinein (`lesen(bytes)`) |
| datentor | `programm.rs:64` | `lauf` | nur `stdin` | — | — | — | — | — | keine Datei; A25 trifft nicht zu |

Gemessen mit einem Durchlauf über `crates/{stelle,formate,modellweg,kernabbild,einstellungen,datentor,rangkern}/src`
nach `read_to_string`, `fs::read`, `fs::read_dir`, `File::open`, `OpenOptions::new`, `rustix::fs::open`,
`openat`, `fs::metadata`, `symlink_metadata` und `statat`, ohne Teile unter `#[cfg(test)]`.

### Zusammenfassung der Lücken nach Eigentümer

| Baustein | Crate | Lücken |
|---|---|---|
| Kette | `formate` | 8 Stellen: `pruefen.rs`, `quelle.rs`, `monate.rs` (5), `schreiber.rs` |
| Modellweg | `modellweg` | macOS (ACL, Gruppe) an 3 geprüften Stellen; `suche.rs` braucht eine benannte Ausnahme |
| Kern-Dienst | `kernabbild` | 1 Stelle: `muster_aus` |
| Tor der Suche | `rangkern` | 11 Stellen, davon `sitzung/einst.rs:76` als Fall von A25 im Wortlaut |
| — | `stelle`, `einstellungen`, `datentor` | keine; die Regel trifft den Aufrufer |

## B4. Benannte Ausnahmen

Eine Ausnahme braucht eine `id`, einen Grund, die Stelle und einen Test. Ohne Test ist sie keine Ausnahme,
sondern eine Lücke. Heute gibt es eine begründete Ausnahme und keine benannte:

| id | Stelle | Grund | Test |
|---|---|---|---|
| `AUS-SUCHE-BAUM` | `crates/modellweg/src/suche.rs:316-330` | Die Schlüsselsuche durchsucht fremde Dateien im Einschluss; Eigentümer und Modus sind dort gerade nicht das Kriterium. Verknüpfungen werden nicht verfolgt, ein Wurzelpfad anderer Art zählt als nicht lesbar. | `modellweg: M-S1`, `M-S2` (`spec/modellweg/README.md:683`) |

Diese Ausnahme ist mit diesem Vertrag **vorgeschlagen**, nicht beschlossen.

## B5. Abnahme (A25)

**Ausgangslage:** Der Satz der Stelle im richtigen Ordner unter `/srv/beispiel/satz/`; darin eine Datei mit
fremdem Eigentümer, eine mit zu offenem Modus (`0644`), eine Verknüpfung auf eine Datei außerhalb und unter
macOS eine Datei mit ACL für eine fremde Kennung (`chmod +a`).

**Handlung:** Ein Werkzeug aus dem Satz liest jede Datei.

**Erwartet:** Die vier werden abgelehnt, die übrigen gelesen; jede Ablehnung mit ihrem Fehlercode. Eine
Ausnahme ohne benannte Regel nach B4 macht die Prüfung rot.

**Gemessen:** Ergebnis und Code je Datei; zusätzlich je Stelle aus B3 ein Eintrag im Katalog nach Teil A,
dessen Anker auf die Prüfung zeigt.

Die Abnahme von A25 ist heute **nicht** erfüllt. Sie wird erfüllt, wenn die Lücken aus B3 geschlossen sind
und jede geschlossene Stelle einen Katalogeintrag trägt.

---

# OPEN

1. **Windows (B2 Punkt 11).** Der Satz ist ein Entwurf ohne Messung. Pflicht für Schritt II.
2. **ACL unter macOS.** Das Verhalten ist hergeleitet. Es braucht eine Messung mit `chmod +a` auf dem Mac,
   bevor Punkt 8 als geprüft gilt.
3. **Eigentümer der Lücken.** `rangkern` gehört dem Tor der Suche, `kernabbild` dem Kern-Dienst. Dieser
   Vertrag nennt die Lücken; er schließt sie nicht.
4. **Ticketkern und Modellproxy des Trägers** (A23, Abnahme Punkt 4) haben noch keinen Katalogeintrag: der
   Code liegt nicht in den Crates dieses Vertrags.
5. **Ausnahme `AUS-SUCHE-BAUM`** braucht ein Ja.
6. **`portable_key`** bleibt eine Ausnahme des Mutationstors, bis die Windows-Prüfung von `rangkern` sie
   deckt (`README.md:187-194`). Der Katalog führt sie nicht als Eintrag.
