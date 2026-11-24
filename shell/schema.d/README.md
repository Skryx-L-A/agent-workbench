# schema.d

Schema-Fragmente anderer Pakete, je Paket eine Datei `<praefix>.json` im Format von `../einstellungen-schema.json`
(`{"version": 1, "einstellungen": [...]}`, optional `"herkunft"`). `wb_konfig.py` liest sie nach Dateiname sortiert.
Ein Fragment legt nur Schlüssel unter seinem Präfix an und überschreibt nie einen Schlüssel des Hauptschemas. Ein
kaputtes Fragment fällt als Ganzes aus und erscheint in `meldungen`. Regeln: Kopf von `../wb_konfig.py`.
Die Pakete legen ihre Fragmente selbst hierher (installiert: `~/.local/bin/schema.d/`); im Repo liegt nur diese Datei.
