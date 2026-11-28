# Vertrag `seite`

Dieser Ordner hält den Vertrag der Oberfläche des Neubaus. Er gilt für zwei Ansichten mit derselben Liste der Bedienwege: das App-Fenster (egui),
die Oberfläche des Pilots, und die Browser-Ansicht der Stufe 2a (`rust/crates/ansicht/`, `rust/ansicht/`) für M1, die `wb-bruecke` unter `GET /`
ausliefert (Entscheide des Betreibers vom 26.11.2026, `rust/spec/VERTRAEGE.md` Fassung 2). Der Vertrag hat zwei Teile:

| Datei | Inhalt |
|---|---|
| `rust/spec/VERTRAEGE.md` | Verhalten der Seite: Tastatur, Fokus, Vorleser, gleicher Text, Handy-Browser, Netz und Start, feste Selektoren (SE1 bis SE7). Kopie des Vertragspakets; nur die Verweise auf Ordner außerhalb des Repos sind Repo-Pfade oder „außerhalb des Repos“. |
| `rust/spec/VERTRAEGE.md` | Aussehen und Bedienwege nach der heutigen Mac-App: Aufbau, flächig, Farbwahl, Breiten, Einstellungen von Nutzer und Admin, 69 Bedienwege mit Umfang Pilot, nach dem Pilot oder entfällt, App-Fenster (OF1 bis OF14), dazu die Lücken in Kern und Dienst (L1 bis L11). |
| `faelle/` | Sichtbarer Fallsatz des Vertrags `seite` (Datensatz A, 44 Fälle `se1_*` bis `se7_*`, einer davon ruht). |

## Anforderungen

| Nr | Titel | Regel |
|---|---|---|
| A157 | Die Oberfläche ist die heutige Werkbank | `rust/spec/VERTRAEGE.md` OF1 bis OF9, OF13 |
| A118 | Zwei Ansichten mit derselben Liste der Bedienwege; Pilot im App-Fenster, M1 im Browser (geändert 26.11.2026, E5, E6) | `rust/spec/VERTRAEGE.md` Abschnitt 0, OF14 |
| A63 | Die Oberfläche läuft unter Windows im Browser (Ergänzung macOS: Safari) | `rust/spec/VERTRAEGE.md` OF12 (Prüfplan, noch nicht gemessen) |
| A31 | Das Produkt fragt nichts Rechtliches ab | `rust/spec/VERTRAEGE.md` OF10 |
| A169 | Gesprächsansicht über OpenWebUI im Browser | `rust/spec/VERTRAEGE.md` OF7 Punkt 4 (keine eigene Gesprächsansicht) |
| A112 | Zwischenablage aus einem Programm: Schreiben mit Hinweis | `rust/spec/VERTRAEGE.md` SE3 Punkt 3 (Ansage `osc52_geschrieben`; zu groß: `zwischenablage_zu_gross`, `nachricht_zu_gross`), `rust/spec/VERTRAEGE.md` B67 |
| A114 | Umschalt+Enter sendet der Träger als `ESC CR` | `rust/spec/VERTRAEGE.md` SE1 Punkt 5 (die Seite schickt `Taste Enter` mit `umschalt`) |

Nicht aufgenommen, weil keine Regel sie trägt: A28, A37, A59, A90, A103, A105, A125, A162. A46 (Routenanzeige) und A129 (Zonenanzeige) nimmt
`rust/spec/VERTRAEGE.md` Abschnitt 1 ausdrücklich aus.

## Wo was liegt

- **Sichtbare Fälle:** `rust/spec/seite/faelle/` (44 Dateien). Vorher lagen sie nur im Vertragspaket; im Repo gab es sie nicht.
- **Läufer:** `laeufer/se_lauf.mjs` liegt nicht im Repo, sondern im Vertragspaket (außerhalb des Repos). Ebenso die Attrappe der Seite,
  der Fallbau, die Gegenprobe und der verdeckte Satz.
- **Kanonisches Werkzeug des Vertrags:** Fallbau, verdeckter Satz, Attrappe, Läufer und Gegenprobe liegen außerhalb des Repos. Es dient der Abnahme, die Tore nutzen es nicht.
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
3. **Bedienwege:** `grep -n 'data-bw' rust/crates/ansicht/src/oberflaeche.rs` gegen die Tabelle in `rust/spec/VERTRAEGE.md` Abschnitt 2 (47 Kennungen mit Umfang Pilot; bis zum Umbau zählt der gebaute Stand noch nach der Fassung vom 23.11.2026).
4. **Bilder und Auge:** Bilder bei 1280 px und 375 px, hell und dunkel, aus einem echten Lauf mit `wb-dienst` und `wb-bruecke`; danach die Prüfliste
   `rust/spec/VERTRAEGE.md` Abschnitt 7. Die Abnahme am Auge steht aus (`rust/spec/VERTRAEGE.md` OF13).
