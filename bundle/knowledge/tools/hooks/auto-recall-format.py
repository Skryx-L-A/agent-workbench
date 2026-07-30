"""Treffer von `brain search --json` in den Recall-Block formen.

Eigene Datei statt `python3 -c "..."`: in doppelten Anfuehrungszeichen
ersetzt die Shell `$`, Backticks und `\\` mitten im Code. Genau daran ist
der Hook am 2026-07-29 haengengeblieben.

Konfiguration kommt ueber die Umgebung (WB_*), die Treffer ueber stdin.
"""
import json, os, sys

THRESHOLD = float(os.environ["WB_THRESHOLD"])
RELATIVE_MARGIN = float(os.environ["WB_RELATIVE_MARGIN"])
MAX_HITS = int(os.environ["WB_MAX_HITS"])
SNIPPET_CHARS = int(os.environ["WB_SNIPPET_CHARS"])

try:
    data = json.load(sys.stdin)
except Exception:
    sys.exit(0)

# Zwei Filter, nicht einer (2026-07-29, nach Messung).
#
# Bisher: filtern nach `cosine`, sortieren nach dem FUSIONIERTEN Score. Das ist
# inkohaerent, und es ist genau der Weg, auf dem themenfremde Notizen hereinkamen:
# ein Treffer, den BM25 wegen ein paar haeufiger Woerter auf Rang 1 hebt, hat oft
# eine cosine knapp ueber der Schwelle und wird trotzdem als bester Treffer
# eingespielt. Gemessen ueber die 37 Eval-Fragen: Praezision 36 %, also zwei von
# drei eingespielten Notizen falsch, bei 2,5 Einspielungen pro Frage.
#
# Deshalb zusaetzlich ein RELATIVER Filter: ein Treffer muss semantisch nah am
# besten Treffer liegen. Ein Feld schwacher Treffer (typisch fuer Gespraechs-
# eingaben wie -mach weiter-) faellt damit ganz weg, statt den schwachen Ersten
# einzuspielen. Der Hook laeuft bei JEDEM Prompt - was er einspielt, kostet in
# jedem Zug Tokens und kann in die Irre fuehren.
hits = [h for h in data.get('hits', []) if h.get('cosine', 0) >= THRESHOLD]
if hits:
    best = max(h.get('cosine', 0) for h in hits)
    hits = [h for h in hits if h.get('cosine', 0) >= best - RELATIVE_MARGIN]
    # Nach semantischer Naehe sortieren, nicht nach der Fusion: der Hook soll
    # thematisch passendes Material zeigen, keine Stichwort-Zufallstreffer.
    hits.sort(key=lambda h: h.get('cosine', 0), reverse=True)
hits = hits[:MAX_HITS]
if not hits:
    sys.exit(0)

lines = ['Brain-Recall (automatisch, ungeprueft - Naehe in Klammern; niedrige Werte',
         'bedeuten: vermutlich unpassend, ignorieren):']
for h in hits:
    title = h.get('title') or h.get('rel', '')
    rel = h.get('rel', '')
    cos = h.get('cosine', 0)
    snippet = (h.get('snippet') or '').strip()
    if len(snippet) > SNIPPET_CHARS:
        snippet = snippet[:SNIPPET_CHARS].rstrip() + '...'
    line = f'- {title} ({rel}, {cos:.2f})'
    if snippet:
        line += f': {snippet}'
    lines.append(line)
print('\n'.join(lines))
