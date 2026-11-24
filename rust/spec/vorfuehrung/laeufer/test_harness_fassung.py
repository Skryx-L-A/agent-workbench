"""Der Laeufer startet nie ein Programm eines Harness (Regel, Nachtrag 2 zur Paketform der Attrappe): Die Fassung der Copilot CLI
liest er aus der Paketform (`harness.copilot` aus `lauf.json`, davon `realpath`, daneben `package.json`).

Nur Python-Standardbibliothek. Aufruf: python3 -I -B test_harness_fassung.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)
sys.path.insert(0, os.path.join(HIER, "attrappen"))
from drehbuchlib import probelauf  # noqa: E402
import paket_legen  # noqa: E402

HAUPT = os.path.join("lib", "node_modules", "@github", "copilot")
MARKENSKRIPT = "#!/bin/sh\necho start >> '%s'\necho 'GitHub Copilot CLI 9.9.9'\n"


class Basis(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.mkdtemp(prefix="harness-fassung-")
        self.addCleanup(shutil.rmtree, self.t, True)
        self.prefix = os.path.join(self.t, ".npm-global")
        paket_legen.lege(self.prefix)
        os.makedirs(os.path.join(self.prefix, "bin"))
        os.symlink(os.path.join("..", HAUPT, "npm-loader.js"), os.path.join(self.prefix, "bin", "copilot"))
        self.cli = os.path.join(self.prefix, "bin", "copilot")  # der Pfad in lauf.json ist ein Verweis, der echte Pfad der Loader
        self.paket = os.path.join(self.prefix, HAUPT, "package.json")
        self.lauf_konfig = os.path.join(self.t, "lauf.json")
        self.schreibe_lauf_konfig({"fassung": 1, "harness": {"copilot": self.cli}})

    def schreibe_lauf_konfig(self, d):
        with open(self.lauf_konfig, "w", encoding="utf-8") as f:
            json.dump(d, f)

    def schreibe_paket(self, d):
        os.remove(self.paket) if os.path.lexists(self.paket) else None
        with open(self.paket, "w", encoding="utf-8") as f:
            json.dump(d, f)

    def fassung(self):
        return probelauf.harness_fassung(self.lauf_konfig, "copilot", "@github/copilot")


class FassungLesen(Basis):
    def test_fassung_aus_der_paketform(self):
        self.assertEqual(self.fassung(), "0.0.88")

    def test_package_json_fehlt(self):
        os.remove(self.paket)
        self.assertEqual(self.fassung(), "unbekannt")

    def test_anderer_name(self):
        self.schreibe_paket({"name": "@fremd/copilot", "version": "1.2.3", "bin": {"copilot": "npm-loader.js"}})
        self.assertEqual(self.fassung(), "unbekannt")

    def test_bin_passt_nicht_zum_dateinamen(self):
        self.schreibe_paket({"name": "@github/copilot", "version": "1.2.3", "bin": {"copilot": "anderes.js"}})
        self.assertEqual(self.fassung(), "unbekannt")
        self.schreibe_paket({"name": "@github/copilot", "version": "1.2.3"})
        self.assertEqual(self.fassung(), "unbekannt")

    def test_passendes_paket_gibt_die_fassung(self):
        self.schreibe_paket({"name": "@github/copilot", "version": "1.2.3", "bin": {"copilot": "npm-loader.js"}})
        self.assertEqual(self.fassung(), "1.2.3")

    def test_unbrauchbare_fassung_oder_form(self):
        for version in (None, 7, "", "1.2.3 und mehr", "x" * 100):
            self.schreibe_paket({"name": "@github/copilot", "version": version, "bin": {"copilot": "npm-loader.js"}})
            self.assertEqual(probelauf.versionen({"probelauf": {"lauf_konfig": self.lauf_konfig}}, HIER)["copilot_cli"], "unbekannt", version)
        with open(self.paket, "w") as f:
            f.write("{")
        self.assertEqual(self.fassung(), "unbekannt")
        with open(self.paket, "w") as f:
            f.write("[]")
        self.assertEqual(self.fassung(), "unbekannt")

    def test_package_json_als_verweis_wird_nicht_verfolgt(self):
        fremd = os.path.join(self.t, "fremd.json")
        with open(fremd, "w") as f:
            json.dump({"name": "@github/copilot", "version": "5.5.5", "bin": {"copilot": "npm-loader.js"}}, f)
        os.remove(self.paket)
        os.symlink(fremd, self.paket)
        self.assertEqual(self.fassung(), "unbekannt")

    def test_zu_grosse_datei(self):
        with open(self.paket, "w") as f:
            f.write('{"name": "@github/copilot", "version": "1.2.3", "bin": {"copilot": "npm-loader.js"}, "x": "%s"}' % ("a" * 70000))
        self.assertEqual(self.fassung(), "unbekannt")

    def test_lauf_konfig_fehlt_kaputt_oder_ohne_harness(self):
        for inhalt in (None, "{", "[]", {"harness": {}}, {"harness": {"copilot": 5}}, {"harness": {"copilot": "relativ/copilot"}},
                       {"harness": {"copilot": os.path.join(self.t, "gibt-es-nicht")}}):
            if inhalt is None:
                os.remove(self.lauf_konfig)
            elif isinstance(inhalt, str):
                with open(self.lauf_konfig, "w") as f:
                    f.write(inhalt)
            else:
                self.schreibe_lauf_konfig(inhalt)
            self.assertEqual(self.fassung(), "unbekannt", inhalt)
        self.assertEqual(probelauf.harness_fassung(None, "copilot", "@github/copilot"), "unbekannt")

    def test_versionen_nennt_die_fassung_und_ohne_konfiguration_unbekannt(self):
        self.assertEqual(probelauf.versionen({"probelauf": {"lauf_konfig": self.lauf_konfig}}, HIER)["copilot_cli"], "0.0.88")
        self.assertEqual(probelauf.versionen({"probelauf": {}}, HIER)["copilot_cli"], "unbekannt")
        self.assertEqual(probelauf.versionen({}, HIER)["copilot_cli"], "unbekannt")


class KeinStartDesHarness(Basis):
    """Gegenprobe: Eine CLI im PATH und unter `harness.copilot`, die bei jedem Start eine Marke schreibt. Ein ganzer Probelauf gegen die Attrappen
    startet sie nicht. Der Laeufer vor Nachtrag 2 (`copilot_version_befehl`) startet sie als erste Handlung."""

    def test_ganzer_probelauf_startet_kein_programm_des_harness(self):
        marken = os.path.join(self.t, "marken")
        bin_ordner = os.path.join(self.t, "pfad")
        os.makedirs(bin_ordner)
        skript = MARKENSKRIPT % marken
        for pfad in (os.path.join(bin_ordner, "copilot"), os.path.join(self.prefix, HAUPT, "npm-loader.js")):  # PATH und harness.copilot
            with open(pfad, "w") as f:
                f.write(skript)
            os.chmod(pfad, 0o755)
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        # `copilot_version_befehl` kennt der Laeufer seit Nachtrag 2 nicht mehr; ein Laeufer, der ihn noch liest, startet damit die CLI
        k["probelauf"] = {"copilot_version_befehl": ["copilot", "--version"], "lauf_konfig": self.lauf_konfig}
        konfig = os.path.join(self.t, "konfig.json")
        with open(konfig, "w", encoding="utf-8") as f:
            json.dump(k, f)
        ergebnis = os.path.join(self.t, "ergebnis.json")
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--konfig", konfig, "--ergebnis", ergebnis],
                           capture_output=True, timeout=180, env={"PATH": bin_ordner + os.pathsep + "/usr/bin:/bin", "HOME": self.t,
                                                                  "TMPDIR": self.t})
        self.assertFalse(os.path.exists(marken), "der Laeufer hat ein Programm des Harness gestartet (Marke da); rc=%s" % p.returncode)
        with open(ergebnis, encoding="utf-8") as f:
            d = json.load(f)
        self.assertEqual(d["fassung"], 2)
        self.assertEqual(sorted(d["versionen"]), ["copilot_cli", "kernel", "laeufer", "paket", "ubuntu"], "Feld und Fassung 2 bleiben")
        self.assertEqual(d["versionen"]["copilot_cli"], "0.0.88", "der Wert kommt aus package.json, nicht aus einem Start")


if __name__ == "__main__":
    unittest.main()
