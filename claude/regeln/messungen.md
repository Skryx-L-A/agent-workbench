# regeln/messungen.md

Inhalt: wohin gemessene Modellzahlen gehören und wo das Werkzeug liegt, das sie erzeugt hat.
Ausgelagert aus `~/.claude/CLAUDE.md` am 2026-08-10, weil die Datei über ihre Größengrenze
gewachsen war; beide Regeln gelten unverändert weiter.

Auslöser: bevor Du eine Modellzahl misst oder festhältst — lokal wie Cloud, in jedem Projekt —,
bevor Du ein Skript schreibst, das eine Messzahl erzeugt, und bevor Du den Verbrauch oder die
Kosten eines Laufs angibst.

- **Modellmessungen wachsen an EINEM Ort, projektübergreifend (2026-08-09, seine Anweisung):**
  jede gemessene Modellzahl — lokal wie Cloud, aus jedem Projekt und jeder Session — wird in
  `~/Knowledge/_meta/messungen/modelle/daten.json` eingetragen (Werkzeug: `wb-messung`), danach
  die Vergleichsseite neu gerendert. Eine Messung, die nur in einer Session-Notiz steht, ist für
  die nächste Session verloren. Ältere Reihen werden nie gelöscht, nur als `veraltet` markiert.
- **Verbrauch immer in BEIDEN Einheiten (2026-08-10, seine Anweisung):** jede Kosten- oder
  Verbrauchsangabe nennt Tokens samt API-Äquivalent UND den Anteil am Anthropic-Limit (5 h und
  Woche). Die Tokenzahl braucht er, falls dieselbe Arbeit später über die API läuft; der
  Limit-Anteil beantwortet, ob der Lauf heute noch hineinpasst. Der Umrechnungsfaktor wird nie
  hartkodiert, sondern empirisch aus `~/.claude/workbench/limits.jsonl` gerechnet — er hängt am
  Modellmix und veraltet. Kalibrierung und Verfahren: [[limit-prozent-je-token]].
- **Eine Limit-Zahl gilt nur für einen Plan und ein Fenster (2026-08-10, seine Anweisung):** das
  Wochenlimit setzt montags 14:00 Ortszeit zurück, und der Nutzer hatte über die Zeit verschiedene
  Abos (Pro, Max 100, Max 200) mit verschieden großen Töpfen. Ein Prozentpunkt bedeutet vor und
  nach einem Wechsel etwas anderes; über eine Plangrenze hinweg wird nie gerechnet. Welcher Plan
  wann galt, steht in `~/.claude/workbench/plan-historie.json` — steht der Zeitraum nicht drin,
  ist die Zahl unbrauchbar, nicht ungefähr.
- **Eine Zahl gilt erst, wenn der messende Prozess identifiziert ist (2026-08-11):** ein
  Prüfstand, der sich an einen festen Debug-Port bindet, verbindet sich stillschweigend mit
  einem Browser aus einem früheren Lauf, wenn der Port belegt ist — gemessen wurde dann ein
  eine Headless-Chrome-Instanz ohne GPU-Pfad, und dieselbe Szene kostete 16 ms statt 0,35 ms, bei
  völlig plausibel aussehenden Zahlen. Gegenmittel: Port 0 plus Auslesen des tatsächlichen
  Ports, und eine eigene Prüfung, die den Renderer-String ausliest und durchfällt, wenn ein
  Software-Rasterizer dahintersteht. Verallgemeinert: **eine Millisekundenzahl ohne Angabe der
  ausführenden Einheit ist wertlos** — dieselbe Verwechslung wie bei
  [[hf-download-xet-stillstand]], nur an der Rechenseite statt an der Leitung.
- **Ein Messwerkzeug gehört ins Repo, nie ins Scratchpad (2026-08-09):** Skripte, die eine Zahl
  erzeugen, überleben sonst ihre Session nicht — die Nebenläufigkeits-Messstrecke des Gardeners
  war mit dem Scratchpad ihres Workers weg, und die offenen Zellen sind seither nicht
  nachmessbar. Wer misst, checkt die Messstrecke mit ein.
