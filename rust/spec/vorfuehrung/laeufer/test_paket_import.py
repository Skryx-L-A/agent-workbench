"""Test: Der Laeufer laeuft aus dem Paket, nicht nur im Repo-Baum. Nur Standardbibliothek. Aufruf: python3 -I -B test_paket_import.py

Der Test liest `rust/paket/ubuntu-26.04-x64/lieferumfang.txt` bei jedem Lauf neu und legt jede Zeile, deren Quelle unter
`spec/vorfuehrung/laeufer/` oder `spec/einstellungen/` liegt, wie im Paket unter einem leeren Testordner ab. Die Attrappen
(`attrappen/`) kommen nicht mit, denn das Paket liefert sie nicht aus. Eine kuenftige Datei im Laeufer, die im Paket fehlt, macht ihn rot.
"""
import importlib
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
RUST = os.path.abspath(os.path.join(HIER, "..", "..", ".."))
LISTE = os.path.join(RUST, "paket", "ubuntu-26.04-x64", "lieferumfang.txt")
SPEC = os.path.join(RUST, "spec")
LAEUFER = "spec/vorfuehrung/laeufer/"
EINST = "spec/einstellungen/"


def zeilen():
    """Alle Zeilen `datei:` der Liste mit Quelle im Laeufer oder unter spec/einstellungen: (art, ziel, quelle)."""
    ergebnis = []
    with open(LISTE, encoding="utf-8") as f:
        for z in f:
            z = z.strip()
            if not z or z.startswith("#"):
                continue
            art, ziel, quelle = z.split(None, 2)
            if quelle.startswith("datei:" + LAEUFER) or quelle.startswith("datei:" + EINST):
                ergebnis.append((art, ziel, quelle[len("datei:"):]))
    return ergebnis


def lege_ab(wurzel):
    for _, ziel, quelle in zeilen():
        src = os.path.join(RUST, quelle)
        dst = os.path.join(wurzel, ziel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(src, dst)


class PaketImport(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wurzel = tempfile.mkdtemp(prefix="pi-")
        cls.addClassCleanup(shutil.rmtree, cls.wurzel, True)
        lege_ab(cls.wurzel)
        cls.lib = os.path.join(cls.wurzel, "usr", "lib", "werkbank", "probelauf")
        cls.drehbuch = os.path.join(cls.lib, "drehbuch.py")
        cls.befehl = os.path.join(cls.wurzel, "usr", "bin", "werkbank-probelauf")
        cls.share = os.path.join(cls.wurzel, "usr", "share", "werkbank", "probelauf")

    def test_liste_nennt_den_laeufer(self):
        ziele = [z for _, z, _ in zeilen()]
        for erwartet in ("usr/bin/werkbank-probelauf", "usr/lib/werkbank/probelauf/drehbuch.py",
                         "usr/share/werkbank/probelauf/konfig.json", "usr/share/werkbank/probelauf/einstellungen.json"):
            self.assertIn(erwartet, ziele)
        self.assertFalse([z for z in ziele if "attrappen" in z], "die Attrappe gehoert nicht ins Paket")
        self.assertFalse(os.path.exists(os.path.join(self.lib, "attrappen")))

    def test_a_drehbuch_help_endet_mit_0(self):
        p = subprocess.run([sys.executable, "-I", "-B", self.drehbuch, "--help"], capture_output=True, text=True, cwd=self.wurzel, timeout=60)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertNotIn("Traceback", p.stderr)

    def test_b_jedes_modul_der_bibliothek_laesst_sich_importieren(self):
        ordner = os.path.join(self.lib, "drehbuchlib")
        module = sorted(n[:-3] for n in os.listdir(ordner) if n.endswith(".py") and n != "__init__.py")
        self.assertTrue(module)
        for m in module:
            with self.subTest(modul=m):
                p = subprocess.run([sys.executable, "-I", "-B", "-c", "import sys, importlib; sys.path.insert(0, sys.argv[1]); importlib.import_module('drehbuchlib.' + sys.argv[2])",
                                    self.lib, m], capture_output=True, text=True, cwd=self.wurzel, timeout=60)
                self.assertEqual(p.returncode, 0, p.stderr)

    def test_c_werkbank_probelauf_kommt_bis_zur_ersten_eigenen_pruefung(self):
        # Ohne Datenordner endet der Befehl mit dem festen Satz und Exit 3 (erste eigene Pruefung).
        daten = os.path.join(self.wurzel, "nicht-da")
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "WERKBANK_DATEN": daten, "TMPDIR": self.wurzel,
               "WERKBANK_DREHBUCH": self.drehbuch, "WERKBANK_KONFIG": os.path.join(self.share, "konfig.json")}
        p = subprocess.run([sys.executable, "-B", self.befehl, "--ergebnis", os.path.join(self.wurzel, "erg.json")],
                           capture_output=True, text=True, env=env, cwd=self.wurzel, timeout=60)
        self.assertEqual(p.returncode, 3, p.stdout + p.stderr)
        self.assertIn("Der Datenordner fehlt oder gehoert nicht dem Nutzer.", p.stderr)
        self.assertNotIn("Traceback", p.stderr)
        self.assertNotIn("ImportError", p.stderr)
        self.assertNotIn("ModuleNotFoundError", p.stderr)

    def test_c2_befehl_startet_das_drehbuch_aus_dem_paket(self):
        # Mit fertigem Datenordner startet der Befehl das abgelegte drehbuch.py; es darf kein Importfehler und kein Traceback entstehen.
        daten = os.path.join(self.wurzel, "daten")
        os.mkdir(daten, 0o700)
        with open(os.path.join(daten, "modellweg.schluessel"), "w") as f:
            f.write("x\n")
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "WERKBANK_DATEN": daten, "TMPDIR": self.wurzel,
               "WERKBANK_DREHBUCH": self.drehbuch, "WERKBANK_KONFIG": os.path.join(self.share, "konfig.json")}
        p = subprocess.run([sys.executable, "-B", self.befehl, "--ergebnis", os.path.join(self.wurzel, "erg.json")],
                           capture_output=True, text=True, env=env, cwd=self.wurzel, timeout=120)
        aus = p.stdout + p.stderr
        for verboten in ("Traceback", "ImportError", "ModuleNotFoundError", "No module named"):
            self.assertNotIn(verboten, aus)
        self.assertIn(p.returncode, (0, 1, 2, 3, 4), aus)


if __name__ == "__main__":
    unittest.main()
