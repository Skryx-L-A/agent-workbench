"""Grund der Sitzung aus der Sitzungsliste (23.11.2026; Anlass B24: nur Code 73 lesbar, nicht `lauf.paket_cache`).

Vertrag `dienst`, DI12 Punkt 5 und Abschnitt 18 Punkt 5: Ein Eintrag der `Sitzungsliste` traegt `grund` nur bei `beendet` und nur nach einer
gueltigen Zeile von `lauf` (oder als `lauf.meldung_ungueltig`); sonst fehlt der Schluessel.

- `Bruecke.sitzung()` gibt `grund` zurueck, wenn der Eintrag ihn traegt, sonst None.
- Gegen die Attrappen (`harness.copilot.sim_ende` mit `grund`): `ende_grund` in den Messwerten von S1 ist der feste Code; ein Grund in
  anderer Form wird `abweichend`, und sein Text steht nirgends im Bericht. Ohne `grund` ist `ende_grund` None.
- Konsole von `drehbuch.py probelauf`: Die Zeile `<schritt> ende:` traegt `grund <code>`; die Ergebnisdatei bleibt Fassung 2 ohne den Grund.
Gegenprobe: mit `drehbuchlib/bruecke.py` im Stand vom 22.11.2026 (vor dieser Aenderung) werden die Tests der Bruecke und der Attrappe rot (der Grund kommt nie an).

Nur Standardbibliothek. Aufruf: python3 -I -B test_sitzung_grund.py   (startet die Attrappen der Bruecke: einige Sekunden je Lauf)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import bruecke, probelauf  # noqa: E402

CACHE_TEXT = "lauf.paket_cache: Paketcache nicht beschreibbar\n"
BOESER_GRUND = "Paket fehlt unter /pfad/geheim"


class FalscherStrom:
    """Nur das, was `Bruecke.sitzung` vom Strom liest: Sperre, Sitzungen, Zustand."""

    def __init__(self, sitzungen):
        self.lock = threading.Condition()
        self.sitzungen = {e["id"]: e for e in sitzungen}
        self.zu = False


def bruecke_mit(sitzungen):
    b = bruecke.Bruecke.__new__(bruecke.Bruecke)
    st = FalscherStrom(sitzungen)
    b._offen = lambda: st
    return b


class SitzungDerBruecke(unittest.TestCase):
    def test_grund_aus_dem_eintrag(self):
        b = bruecke_mit([{"id": "s-1", "harness": "copilot", "modell": "m", "route": "cloud", "kunde": "", "zustand": "beendet",
                          "code": 73, "grund": "lauf.paket_cache"}])
        info = b.sitzung("s-1", frist_s=0)
        self.assertEqual((info["zustand"], info["code"], info["grund"]), ("beendet", 73, "lauf.paket_cache"))

    def test_ohne_grund_none(self):
        b = bruecke_mit([{"id": "s-1", "harness": "pi", "modell": "m", "route": "lokal", "kunde": "", "zustand": "beendet", "code": 129},
                         {"id": "s-2", "harness": "pi", "modell": "m", "route": "lokal", "kunde": "", "zustand": "laeuft"}])
        self.assertIsNone(b.sitzung("s-1", frist_s=0)["grund"])
        self.assertIsNone(b.sitzung("s-2", frist_s=0)["grund"])
        self.assertEqual(b.sitzung("s-3", frist_s=0), {"error": "unbekannt"})


class EndeZeile(unittest.TestCase):
    MW = {"ende_zustand": "beendet", "ende_code": 73, "ende_lauf_codes": ["lauf.paket_cache"], "ende_texte": {"ENOSPC": 0}}

    def test_fester_code(self):
        self.assertEqual(probelauf.ende_zeile("S1", dict(self.MW, ende_grund="lauf.paket_cache")),
                         "  S1 ende: zustand beendet, code 73, grund lauf.paket_cache, lauf lauf.paket_cache, texte -")

    def test_ohne_grund_strich(self):
        self.assertEqual(probelauf.ende_zeile("S1", dict(self.MW, ende_grund=None)),
                         "  S1 ende: zustand beendet, code 73, grund -, lauf lauf.paket_cache, texte -")
        self.assertEqual(probelauf.ende_zeile("S1", self.MW), "  S1 ende: zustand beendet, code 73, grund -, lauf lauf.paket_cache, texte -")

    def test_kein_fester_code_wird_abweichend(self):
        for boese in (BOESER_GRUND, "Lauf.Gross", 73, "", "a" * 65, ["lauf.x"]):
            z = probelauf.ende_zeile("S1", dict(self.MW, ende_grund=boese))
            self.assertEqual(z, "  S1 ende: zustand beendet, code 73, grund abweichend, lauf lauf.paket_cache, texte -", repr(boese))


class Werkstatt(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ordner = tempfile.mkdtemp(prefix="sitzgrund-")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.ordner, ignore_errors=True)

    @classmethod
    def konfig(cls, name, ende):
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        k["harness"]["copilot"]["sim_ende"] = ende
        pfad = os.path.join(cls.ordner, "konfig-%s.json" % name)
        with open(pfad, "w", encoding="utf-8") as f:
            json.dump(k, f)
        return pfad

    @classmethod
    def s1(cls, name, ende):
        """S1 allein gegen die Attrappen, JSON-Bericht: (Bericht als Text, Eintrag von S1)."""
        js = os.path.join(cls.ordner, "bericht-%s.json" % name)
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "attrappe", "--json", js,
                            "--konfig", cls.konfig(name, ende), "--nur", "S1"], capture_output=True, timeout=120)
        if not os.path.exists(js):
            raise AssertionError("kein Bericht, Exit %s: %s" % (p.returncode, p.stderr.decode("utf-8", "replace")[-2000:]))
        with open(js, encoding="utf-8") as f:
            roh = f.read()
        return roh, {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S1"]


class GrundGegenDieAttrappe(Werkstatt):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.roh_fest, cls.fest = cls.s1("fest", {"code": 73, "text": CACHE_TEXT, "grund": "lauf.paket_cache"})
        cls.roh_boese, cls.boese = cls.s1("boese", {"code": 73, "text": CACHE_TEXT, "grund": BOESER_GRUND})
        cls.roh_ohne, cls.ohne = cls.s1("ohne", {"code": 73, "text": CACHE_TEXT})

    def test_fester_grund_steht_in_den_messwerten(self):
        m = self.fest["messwerte"]
        self.assertEqual(self.fest["code_fest"], "s1.harness_beendet")
        self.assertEqual((m["ende_zustand"], m["ende_code"], m["ende_grund"]), ("beendet", 73, "lauf.paket_cache"))

    def test_grund_ohne_form_wird_abweichend(self):
        self.assertEqual(self.boese["messwerte"]["ende_grund"], "abweichend")
        for inhalt in (BOESER_GRUND, "/pfad/geheim", "Paket fehlt"):
            self.assertNotIn(inhalt, self.roh_boese, "kein Text aus der Antwort im Bericht")

    def test_ohne_grund_none(self):
        self.assertIn("ende_grund", self.ohne["messwerte"])
        self.assertIsNone(self.ohne["messwerte"]["ende_grund"])


class GrundAufDerKonsole(Werkstatt):
    def test_zeile_traegt_grund_und_datei_nicht(self):
        erg = os.path.join(self.ordner, "ergebnis-konsole.json")
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig",
                            self.konfig("konsole", {"code": 73, "text": CACHE_TEXT, "grund": BOESER_GRUND}), "--ergebnis", erg],
                           capture_output=True, timeout=300)
        konsole = p.stdout.decode("utf-8", "replace") + p.stderr.decode("utf-8", "replace")
        ende = [z for z in konsole.splitlines() if " ende: " in z]
        self.assertIn("  S1 ende: zustand beendet, code 73, grund abweichend, lauf lauf.paket_cache, texte -", ende)
        self.assertNotIn("/pfad/geheim", konsole)
        with open(erg, encoding="utf-8") as f:
            roh = f.read()
        d = json.loads(roh)
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.assertNotIn("ende_", roh, "die Ergebnisdatei bleibt Fassung 2 ohne den Grund")
        self.assertNotIn("abweichend", roh)
        self.assertNotIn("/pfad/geheim", roh)

    def test_zeile_mit_festem_grund(self):
        erg = os.path.join(self.ordner, "ergebnis-fest.json")
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig",
                            self.konfig("konsole-fest", {"code": 73, "text": CACHE_TEXT, "grund": "lauf.paket_cache"}), "--ergebnis", erg],
                           capture_output=True, timeout=300)
        ende = [z for z in p.stdout.decode("utf-8", "replace").splitlines() if " ende: " in z]
        self.assertIn("  S1 ende: zustand beendet, code 73, grund lauf.paket_cache, lauf lauf.paket_cache, texte -", ende)


if __name__ == "__main__":
    unittest.main()
