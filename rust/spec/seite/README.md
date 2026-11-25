# Vertrag `seite`

Dieser Ordner hält den Vertrag der Browser-Seite des Neubaus. Die Seite ist heute die Ansicht der Stufe 2a (`rust/crates/ansicht/`, `rust/ansicht/`),
die `wb-bruecke` unter `GET /` ausliefert. Der Vertrag hat zwei Teile:

| Datei | Inhalt |
|---|---|
| `rust/spec/VERTRAEGE.md` | Verhalten der Seite: Tastatur, Fokus, Vorleser, gleicher Text, Handy-Browser, Netz und Start, feste Selektoren (SE1 bis SE7). Kopie des Vertragspakets; nur die Verweise auf Ordner außerhalb des Repos sind Repo-Pfade oder „außerhalb des Repos“. |
| `rust/spec/VERTRAEGE.md` | Aussehen und Bedienwege: Code-Ansicht der heutigen Werkbank, Karten und Maße, hell und dunkel, Breiten, 66 Bedienwege mit Zustand und Fundstelle, Vorgaben der Optionen, Browser unter Windows (OF1 bis OF13). |
| `faelle/` | Sichtbarer Fallsatz des Vertrags `seite` (Datensatz A, 41 Fälle `se1_*` bis `se7_*`). |

## Anforderungen

| Nr | Titel | Regel |
|---|---|---|
| A157 | Die Oberfläche ist die heutige Werkbank | `rust/spec/VERTRAEGE.md` OF1 bis OF9, OF13 |
| A63 | Die Oberfläche läuft unter Windows im Browser (Ergänzung macOS: Safari) | `rust/spec/VERTRAEGE.md` OF12 (Prüfplan, noch nicht gemessen) |
| A31 | Das Produkt fragt nichts Rechtliches ab | `rust/spec/VERTRAEGE.md` OF10 |
| A169 | Gesprächsansicht über OpenWebUI im Browser | `rust/spec/VERTRAEGE.md` OF7 Punkt 4 (keine eigene Gesprächsansicht) |
| A112 | Zwischenablage aus einem Programm: Schreiben mit Hinweis | `rust/spec/VERTRAEGE.md` SE3 Punkt 3 (Ansage `osc52_geschrieben`; zu groß: `zwischenablage_zu_gross`, `nachricht_zu_gross`), `rust/spec/VERTRAEGE.md` B67 |
| A114 | Umschalt+Enter sendet der Träger als `ESC CR` | `rust/spec/VERTRAEGE.md` SE1 Punkt 5 (die Seite schickt `Taste Enter` mit `umschalt`) |

Nicht aufgenommen, weil keine Regel sie trägt: A28, A37, A59, A90, A103, A105, A125, A162. A46 (Routenanzeige) und A129 (Zonenanzeige) nimmt
`rust/spec/VERTRAEGE.md` Abschnitt 1 ausdrücklich aus.

## Wo was liegt

- **Sichtbare Fälle:** `rust/spec/seite/faelle/` (41 Dateien). Vorher lagen sie nur im Vertragspaket; im Repo gab es sie nicht.
- **Läufer:** `laeufer/se_lauf.mjs` liegt nicht im Repo, sondern im Vertragspaket (außerhalb des Repos). Ebenso die Attrappe der Seite,
  der Fallbau, die Gegenprobe und der verdeckte Satz.
- **Gegenstand:** `rust/crates/ansicht/src/oberflaeche.rs` (Komponenten, `data-bw`), `rust/crates/ansicht/src/kern.rs` (Regeln ohne DOM),
  `rust/ansicht/ansicht.css` (Maße und Farben), `rust/ansicht/ausgabe/` (ausgelieferte Dateien).
- Die alte Seite `rust/seite/` wird nicht mehr ausgeliefert. Ihr README nennt den Läufer noch mit `rust/seite/ausgabe`.

## Prüfen

1. **Gleichheit der Ausgabe:** `bash rust/ansicht/test/gleichheit.sh`. Exit 0: Der frische Bau ist Byte für Byte gleich `rust/ansicht/ausgabe/`.
   Er braucht das Ziel `wasm32-unknown-unknown` und `wasm-bindgen-cli` in der Fassung aus `rust/Cargo.lock`.
2. **Vertrag `seite`:** im Vertragspaket
   `node laeufer/se_lauf.mjs --seite <worktree>/rust/ansicht/ausgabe --faelle <worktree>/rust/spec/seite/faelle --bericht <datei.json>`.
   Für die Ansicht braucht der Läufer drei Änderungen (`/ansicht.js` und `/ansicht_bg.wasm` ausliefern und in der Netzregel erlauben,
   `'wasm-unsafe-eval'` in der CSP). Exit 0 heißt alle Fälle grün. Ein Lauf dauert rund 30 s; Läufe über 20 s nur auf einer ruhigen Messmaschine.
3. **Bedienwege:** `grep -n 'data-bw' rust/crates/ansicht/src/oberflaeche.rs` gegen die Tabelle in `rust/spec/VERTRAEGE.md` Abschnitt 2 (66 Kennungen).
4. **Bilder und Auge:** Bilder bei 1280 px und 375 px, hell und dunkel, aus einem echten Lauf mit `wb-dienst` und `wb-bruecke`; danach die Prüfliste
   `rust/spec/VERTRAEGE.md` Abschnitt 7. Die Abnahme am Auge steht aus (`rust/spec/VERTRAEGE.md` OF13).
