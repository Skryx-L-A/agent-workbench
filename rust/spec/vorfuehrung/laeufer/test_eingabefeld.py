"""Der Laeufer sendet nie in eine Sitzung ohne erkanntes Eingabefeld (Abschluss 21.11., Entscheidung).

Im echten Start scheiterte die Copilot CLI vor jeder Oberflaeche (ENOSPC beim Entpacken), und der Laeufer sandte trotzdem die erste
Eingabe: `messen.start_bis_eingabefeld` gab nach der Frist nur `t1 = None` zurueck, und `schritte.s1` pruefte das nicht. Jetzt merkt
`Lauf.sitzung_starten` je Sitzung, ob das Eingabefeld erkannt wurde, und `Lauf.zug` wirft `KeinEingabefeld`, BEVOR die Bruecke etwas
sieht; ein spaetes Feld, das jetzt im Bild steht, zaehlt. `ablauf.ausnahme_code` gibt feste Codes, und `ablauf.schritt_nach_ausnahme`
behaelt die vor der Ausnahme vermerkten Teile. Den ganzen Lauf gegen die Attrappen prueft `test_eingabefeld_lauf.py`.

Nur Standardbibliothek. Aufruf: python3 -I -B test_eingabefeld.py
"""
import os
import shutil
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, konfig, lauf, probelauf, schritte  # noqa: E402
from test_einstart import FalscheBruecke  # noqa: E402


class BrueckeMitBild(FalscheBruecke):
    """Wie FalscheBruecke, dazu ein Raster mit dem Eingabefeld der Attrappe und eine Sitzungsliste."""

    def __init__(self, zeilen, zustand="laeuft", code=0):
        super().__init__()
        self.zeilen, self.zustand, self.code = zeilen, zustand, code

    def raster(self, sid):
        return {"zeilen": list(self.zeilen)}

    def sitzung(self, sid, frist_s=2.0):
        return {"zustand": self.zustand, "code": self.code}


class Sperre(unittest.TestCase):
    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="eingabefeld-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)

    def lauf_mit(self, bruecke):
        """Ein Lauf wie `test_einstart.neuer_lauf`, aber um die Bruecke dieses Tests."""
        k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, self.arbeit)
        for a in ("cloud", "lokal"):
            k["modell"][a]["zaehlen"] = False
        L = lauf.Lauf(k, bruecke, {"cloud": None, "lokal": None}, None, self.arbeit)
        h = L.start_dict("cloud")["harness"]
        L.k["harness"][h]["eingabefeld_muster"] = ["/commands"]
        L.art_von["s-1"] = "cloud"
        return L

    def test_ohne_eingabefeld_geht_nichts_raus(self):
        b = FalscheBruecke()
        L = self.lauf_mit(b)
        L.eingabefeld["s-1"] = False
        with self.assertRaises(lauf.KeinEingabefeld):
            L.zug("s-1", "zaehlen", "")
        self.assertEqual(b.eingaben, [], "keine Eingabe an die Bruecke")
        self.assertEqual((L.cloud_eingaben, L.turn), (0, 0), "nichts gezaehlt, kein Zug begonnen")

    def test_gegenprobe_mit_eingabefeld_geht_die_eingabe_raus(self):
        b = FalscheBruecke()
        L = self.lauf_mit(b)
        L.eingabefeld["s-1"] = True
        L.zug("s-1", "zaehlen", "")
        self.assertEqual(len(b.eingaben), 1)
        self.assertEqual(L.cloud_eingaben, 1)

    def test_spaetes_feld_im_bild_zaehlt(self):
        b = BrueckeMitBild(["Copilot CLI (Attrappe)", "  / commands · ? help"])
        L = self.lauf_mit(b)
        L.eingabefeld["s-1"] = False
        L.zug("s-1", "zaehlen", "")
        self.assertEqual(len(b.eingaben), 1)
        self.assertIs(L.eingabefeld["s-1"], True)

    def test_beendete_sitzung_traegt_zustand_und_code(self):
        b = BrueckeMitBild(["TAR_ENTRY_ERROR ..."], zustand="beendet", code=5)
        L = self.lauf_mit(b)
        L.eingabefeld["s-1"] = False
        with self.assertRaises(lauf.KeinEingabefeld) as f:
            L.zug("s-1", "zaehlen", "")
        self.assertEqual((f.exception.zustand, f.exception.code), ("beendet", 5))
        self.assertEqual(b.eingaben, [])
        self.assertEqual(ablauf.ausnahme_code("S1", f.exception), "s1.harness_beendet")

    def test_unbekannte_sitzung_bleibt_wie_bisher(self):
        """Sitzungen, die nicht ueber sitzung_starten kamen (Tests mit eigener Bruecke), werden nicht gesperrt."""
        b = FalscheBruecke()
        L = self.lauf_mit(b)
        L.zug("s-1", "zaehlen", "")
        self.assertEqual(len(b.eingaben), 1)

    def test_sitzung_starten_merkt_fehlendes_feld(self):
        class B(BrueckeMitBild):
            def starten(self, start, ohne_token=False):
                self.starts.append((start, ohne_token))
                return 200, {"sitzung": "s-7"}, 0.0
        b = B(["nichts"], zustand="beendet", code=5)
        L = self.lauf_mit(b)
        L.k["bruecke"]["frist_s"] = 0.2
        m = L.sitzung_starten("cloud")
        self.assertEqual((m["sitzung"], m["t1"]), ("s-7", None))
        self.assertIs(L.eingabefeld["s-7"], False)
        self.assertEqual(L.sitzung_zustand["s-7"], ("beendet", 5))


class Codes(unittest.TestCase):
    def test_feste_codes(self):
        self.assertEqual(ablauf.ausnahme_code("S1", lauf.KeinEingabefeld("s-1", "laeuft", 0)), "s1.kein_eingabefeld")
        self.assertEqual(ablauf.ausnahme_code("E4-cloud", lauf.KeinEingabefeld("s-1")), "e4-cloud.kein_eingabefeld")
        self.assertEqual(ablauf.ausnahme_code("E6", lauf.KeinEingabefeld("s-2", "beendet", 5)), "e6.harness_beendet")
        self.assertEqual(ablauf.ausnahme_code("S5", lauf.Zeitueberschreitung("x")), "s5.zug_frist")
        self.assertEqual(ablauf.ausnahme_code("S2", lauf.EingabeLimit("x")), "cloud.eingaben-limit")
        self.assertIsNone(ablauf.ausnahme_code("S2", ValueError("x")), "unbekannte Ausnahme bleibt .ausnahme")
        for s in probelauf.SCHRITTE:
            for w in ("kein_eingabefeld", "harness_beendet", "zug_frist"):
                self.assertIn("%s.%s" % (s.lower(), w), probelauf.FEHLERCODES)

    def test_teile_bleiben_nach_ausnahme(self):
        del schritte.ANGELEGT[:]
        marke = len(schritte.ANGELEGT)
        s = schritte.Schritt("S1", "x")
        s.haupt("Start angenommen", True)
        s.haupt("Eingabefeld erscheint", False)
        r = ablauf.schritt_nach_ausnahme("S1", marke)
        self.assertIs(r, s)
        self.assertEqual(len(r.teile), 2)
        neu = ablauf.schritt_nach_ausnahme("S2", marke)
        self.assertIsNot(neu, s)
        self.assertEqual(neu.teile, [])


if __name__ == "__main__":
    unittest.main()
