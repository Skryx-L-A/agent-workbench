"""Der Laeufer bedient jedes Element der Seite mit der Art, die es in `rust/seite/seite.html` hat.

`fuellen` (page.fill) geht nur an input und textarea, `waehlen` und `warteoption` nur an ein select. Im echten Start scheiterte S1 an
`modell-wahl`: die Seite hat dort ein select, der Laeufer rief `fuellen`. Der Test liest die Aufrufe aus `drehbuchlib/seite.py`, loest
jede Kennung ueber `browser.selektoren` aus `konfig.attrappe.json` auf und sucht das Element in `seite.html` (die alte Seite) und
in `attrappen/seite.html` (die Attrappe). Gegenprobe: Mit dem seite.py vor dieser Korrektur wird der Test rot.

Nur Standardbibliothek. Aufruf: python3 -I -B test_seite_art.py
"""
import json
import os
import re
import sys
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
SEITEN = {
    "seite.html (echte Seite)": os.path.normpath(os.path.join(HIER, "..", "..", "..", "seite", "seite.html")),
    "attrappen/seite.html": os.path.join(HIER, "attrappen", "seite.html"),
}
ERLAUBT = {"fuellen": {"input", "textarea"}, "waehlen": {"select"}, "warteoption": {"select"}}


def lies(pfad):
    with open(pfad, encoding="utf-8") as f:
        return f.read()


def aufrufe(quelle):
    """[(Zeilennummer, Operation, Schluessel in browser.selektoren)] aller Aufrufe von fuellen, waehlen und warteoption."""
    kennung = dict(re.findall(r'(\w+)\s*=\s*self\.sel\["(\w+)"\]', quelle))
    erg = []
    for n, zeile in enumerate(quelle.splitlines(), 1):
        m = re.search(r'\.rufe\("(fuellen|waehlen|warteoption)".*?testid=([^,)]+)', zeile)
        if not m:
            continue
        ziel = m.group(2).strip()
        if ziel == "kennung" and "kennung" not in kennung:
            continue  # Parameter einer Hilfsmethode; die Aufrufer stehen als Hilfsaufruf in der Liste
        k = re.fullmatch(r'(?:s|self\.sel)\["(\w+)"\]', ziel)
        schluessel = k.group(1) if k else kennung.get(ziel)
        erg.append((n, m.group(1), schluessel, ziel))
    for n, zeile in enumerate(quelle.splitlines(), 1):
        # Hilfsmethode, die waehlt: `self._waehle_nach_laden(s["harness"], ...)` ist ein `waehlen` an dieser Kennung
        h = re.search(r'self\._waehle_nach_laden\((?:s|self\.sel)\["(\w+)"\]', zeile)
        if h:
            erg.append((n, "waehlen", h.group(1), h.group(0)))
    return erg


def element_art(html, testid):
    """Tag des Elements mit diesem data-testid, oder None."""
    m = re.search(r'<(\w+)\b[^>]*\bdata-testid="%s"' % re.escape(testid), html)
    return m.group(1).lower() if m else None


class ArtDerBedienung(unittest.TestCase):
    def setUp(self):
        self.sel = json.loads(lies(os.path.join(HIER, "konfig.attrappe.json")))["browser"]["selektoren"]
        self.aufrufe = aufrufe(lies(os.path.join(HIER, "drehbuchlib", "seite.py")))

    def test_alle_aufrufe_gefunden(self):
        # Schutz gegen einen Test, der nichts prueft: Start waehlt harness, route, modell, kunde und fuellt arbeitsordner; Eingabe fuellt eingabe
        schluessel = {(op, k) for _, op, k, _ in self.aufrufe}
        for erwartet in (("waehlen", "harness"), ("waehlen", "route"), ("waehlen", "kunde"), ("fuellen", "arbeitsordner"), ("fuellen", "eingabe")):
            self.assertIn(erwartet, schluessel)
        self.assertTrue(any(k == "modell" for _, _, k, _ in self.aufrufe))

    def test_jede_kennung_hat_die_passende_art(self):
        for name, pfad in SEITEN.items():
            html = lies(pfad)
            for n, op, schluessel, ziel in self.aufrufe:
                with self.subTest(seite=name, zeile=n, op=op, kennung=schluessel):
                    self.assertIn(schluessel, self.sel, "seite.py:%d: Ziel %s nicht aus browser.selektoren" % (n, ziel))
                    art = element_art(html, self.sel[schluessel])
                    self.assertIsNotNone(art, "%s: Kennung %s fehlt" % (name, self.sel[schluessel]))
                    self.assertIn(art, ERLAUBT[op], "seite.py:%d: %s an %s, aber das Element ist ein <%s>" % (n, op, self.sel[schluessel], art))

    def test_modell_wird_gewaehlt_nicht_gefuellt(self):
        ops = {op for _, op, k, _ in self.aufrufe if k == "modell"}
        self.assertEqual(ops, {"waehlen", "warteoption"})


if __name__ == "__main__":
    unittest.main()
