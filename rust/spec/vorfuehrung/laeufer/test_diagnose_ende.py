"""Diagnose bei Sitzungsende und Abbruchgrund der Attrappe (21.11.).

Teil 2: Endet eine Cloud-Sitzung vor dem ersten Bild (Attrappe der Bruecke, Konfiguration `harness.copilot.sim_ende`: Code und fester Text),
stehen in den Messwerten von S1 nur Codes und Zahlen: Zustand und Code der Sitzung (`ende_zustand`, `ende_code`), die `lauf.*`-Codes aus dem
letzten Bild (`ende_lauf_codes`) und die Zaehler fester Fehlertexte (`ende_texte`). Nie Inhalt des Bildes. Die Ergebnisdatei bleibt Fassung 2
(`pruefe_form`), der Code ist `s1.harness_beendet`, und es geht keine Eingabe an die Sitzung.
Gegenprobe mit dem Laeufer vor dieser Korrektur (lauf.py, schritte.py, ablauf.py): Die Messwerte fehlen, diese Tests werden rot.

Teil 3: Die Attrappe der CLI schreibt vor dem ersten Bild eine feste erste Zeile auf die Fehlerausgabe, und sie kommt im Pseudoterminal im Bild an:
`Failed to extract bundled package`, `Failed to find the platform package`, `Failed to start the platform package` und fuer eine Ausnahme in Python
`Unexpected error in the CLI attrappe`. Je Fall ein Test.

Nur Standardbibliothek. Aufruf: python3 -I -B test_diagnose_ende.py   (startet die Attrappen der Bruecke: einige Sekunden je Lauf)
"""
import json
import os
import pty
import select
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad
sys.path.insert(0, os.path.join(HIER, "attrappen"))

import paket_legen  # noqa: E402
from drehbuchlib import lauf, probelauf  # noqa: E402

ENTPACKEN = "Failed to extract bundled package"
GRUND = "No such file or directory"


def lade(name="konfig.attrappe.json"):
    with open(os.path.join(HIER, name), encoding="utf-8") as f:
        return json.load(f)


class Werkstatt(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ordner = tempfile.mkdtemp(prefix="diagende-")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.ordner, ignore_errors=True)

    @classmethod
    def konfig(cls, name, ende):
        k = lade()
        k["harness"]["copilot"]["sim_ende"] = ende
        pfad = os.path.join(cls.ordner, "konfig-%s.json" % name)
        with open(pfad, "w", encoding="utf-8") as f:
            json.dump(k, f)
        return pfad

    @classmethod
    def s1(cls, name, ende):
        """S1 allein gegen die Attrappen, JSON-Bericht: der Eintrag von S1."""
        js = os.path.join(cls.ordner, "bericht-%s.json" % name)
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "attrappe", "--json", js,
                            "--konfig", cls.konfig(name, ende), "--nur", "S1"], capture_output=True, timeout=120)
        with open(js, encoding="utf-8") as f:
            b = json.load(f)
        return p, {s["schritt"]: s for s in b["schritte"]}["S1"]


class DiagnoseBeiSitzungsende(Werkstatt):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.p1, cls.s1_extract = cls.s1("extract", {"code": 1, "text": ENTPACKEN + "\n"})
        cls.p2, cls.s1_codes = cls.s1("codes", {"code": 65, "text": "lauf.paket_entpacken: Paket nicht entpackt\nlauf.paket_teil: 1 kein-platz\n"
                                                      "ENOSPC: no space left\nPermission denied\nPermission denied\n" + GRUND + "\nTraceback (most recent)\n"})

    def test_code_und_zaehler_stehen_in_den_messwerten_von_s1(self):
        m = self.s1_extract["messwerte"]
        self.assertEqual(self.s1_extract["code_fest"], "s1.harness_beendet")
        self.assertEqual(m["ende_zustand"], "beendet")
        self.assertEqual(m["ende_code"], 1)
        self.assertEqual(m["ende_lauf_codes"], [])
        self.assertEqual(m["ende_texte"][ENTPACKEN], 1)
        self.assertEqual({t: n for t, n in m["ende_texte"].items() if t != ENTPACKEN}, {t: 0 for t in lauf.ENDE_TEXTE if t != ENTPACKEN})
        self.assertEqual(set(m["ende_texte"]), set(lauf.ENDE_TEXTE), "alle acht festen Texte haben einen Zaehler")

    def test_lauf_codes_und_mehrere_zaehler(self):
        m = self.s1_codes["messwerte"]
        self.assertEqual(m["ende_code"], 65)
        self.assertEqual(m["ende_lauf_codes"], ["lauf.paket_entpacken", "lauf.paket_teil"])
        self.assertEqual((m["ende_texte"]["ENOSPC"], m["ende_texte"]["Permission denied"], m["ende_texte"]["No such file"], m["ende_texte"]["Traceback"]),
                         (1, 2, 1, 1))
        self.assertEqual(m["ende_texte"][ENTPACKEN], 0)

    def test_nur_codes_und_zahlen_nie_inhalt(self):
        for s in (self.s1_extract, self.s1_codes):
            roh = json.dumps({k: v for k, v in s["messwerte"].items() if k.startswith("ende_")})
            for inhalt in ("Paket nicht entpackt", "no space left", "kein-platz", "most recent"):
                self.assertNotIn(inhalt, roh)
            self.assertTrue(all(isinstance(n, int) for n in s["messwerte"]["ende_texte"].values()))

    def test_ohne_eingabefeld_geht_keine_eingabe_raus(self):
        for s in (self.s1_extract, self.s1_codes):
            namen = {t["name"]: t["ok"] for t in s["teile"]}
            self.assertIs(namen.get("Eingabefeld erscheint"), False)
            self.assertIsNone(s["messwerte"].get("rc_env"), "kein Befehl ging an die Sitzung")

    def test_ergebnisdatei_bleibt_fassung_2(self):
        erg = os.path.join(self.ordner, "ergebnis.json")
        subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig",
                        self.konfig("probe", {"code": 1, "text": ENTPACKEN + "\n"}), "--ergebnis", erg], capture_output=True, timeout=300)
        with open(erg, encoding="utf-8") as f:
            d = json.load(f)
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.assertEqual(d["fassung"], 2)
        s1 = {s["schritt"]: s for s in d["schritte"]}["S1"]
        self.assertEqual(s1["fehlercode"], "s1.harness_beendet")
        self.assertEqual(set(s1), {"schritt", "bestanden", "fehlercode", "dauer_ms", "offen"}, "keine neuen Felder in der Datei")
        self.assertNotIn("ende_", json.dumps(d))


class EndeZeileAufDerKonsole(Werkstatt):
    """Nachtrag 3: je Schritt mit `ende_*` genau eine Zeile `  <schritt> ende: zustand <z>, code <c>, grund <g|->, lauf <codes|->, texte <name=n,...|->` auf der Konsole
    von `drehbuch.py probelauf` (lauf.log). Nur Codes und Zahlen; ein gruener Lauf hat sie nie. Die Ergebnisdatei bleibt Fassung 2.
    Dass erwartung-m1.py sie nicht anders entscheiden laesst, wird ausserhalb des Repos belegt (kein Test mit Pfad ausserhalb)."""

    def konsole(self, name, ende):
        erg = os.path.join(self.ordner, "ergebnis-%s.json" % name)
        k = lade()
        if ende:
            k["harness"]["copilot"]["sim_ende"] = ende
        pfad = os.path.join(self.ordner, "konfig-k-%s.json" % name)
        with open(pfad, "w", encoding="utf-8") as f:
            json.dump(k, f)
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig", pfad,
                            "--ergebnis", erg], capture_output=True, timeout=300)
        with open(erg, encoding="utf-8") as f:
            d = json.load(f)
        return p.stdout.decode("utf-8", "replace").splitlines(), d

    def test_zeile_mit_code_und_zaehlern(self):
        zeilen, d = self.konsole("ende", {"code": 1, "text": ENTPACKEN + "\n"})
        ende = [z for z in zeilen if " ende: " in z]
        # jeder sendende Schritt, dessen Sitzung ohne Eingabefeld endete, hat genau eine Zeile (hier S1, S2, S3, E1, E2, E3: gleiche Sitzung, gleicher Text)
        self.assertEqual(ende.count("  S1 ende: zustand beendet, code 1, grund -, lauf -, texte Failed_to_extract_bundled_package=1"), 1)
        schritte_ = [z.split()[0] for z in ende]
        self.assertEqual(len(schritte_), len(set(schritte_)), "je Schritt hoechstens eine Zeile")
        self.assertTrue(all(z.endswith("code 1, grund -, lauf -, texte Failed_to_extract_bundled_package=1") for z in ende))
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.assertNotIn("ende_", json.dumps(d), "nichts davon in der Ergebnisdatei")

    def test_gruener_lauf_hat_sie_nie(self):
        zeilen, _ = self.konsole("gruen", None)
        self.assertEqual([z for z in zeilen if " ende:" in z], [])

    def test_form_und_inhalt_der_zeile(self):
        mw = {"ende_zustand": "beendet", "ende_code": 65, "ende_lauf_codes": ["lauf.paket_entpacken", "lauf.paket_teil"],
              "ende_texte": {"ENOSPC": 1, "Permission denied": 2, "No such file": 0}}
        self.assertEqual(probelauf.ende_zeile("S1", mw),
                         "  S1 ende: zustand beendet, code 65, grund -, lauf lauf.paket_entpacken,lauf.paket_teil, texte ENOSPC=1,Permission_denied=2")
        leer = {"ende_zustand": None, "ende_code": None, "ende_lauf_codes": [], "ende_texte": {t: 0 for t in lauf.ENDE_TEXTE}}
        self.assertEqual(probelauf.ende_zeile("E5", leer), "  E5 ende: zustand -, code -, grund -, lauf -, texte -")
        self.assertIsNone(probelauf.ende_zeile("S1", {"rc_env": 0}))
        # Inhalt des Bildes oder ein Pfad kommt nie hinein: Codes werden auf die Form `lauf.<wort>` begrenzt
        boese = dict(mw, ende_lauf_codes=["lauf.x", "/pfad/geheim", "Inhalt mit Leerzeichen"], ende_zustand="/pfad")
        z = probelauf.ende_zeile("S1", boese)
        self.assertEqual(z, "  S1 ende: zustand -, code 65, grund -, lauf lauf.x, texte ENOSPC=1,Permission_denied=2")


def im_pty(argv, env, frist=10.0):
    """Startet `argv` in einem Pseudoterminal; gibt (Exit, Bild als Text). stdout und stderr stehen beide im Bild."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(argv[0], argv, env)
    bild, ende = b"", time.monotonic() + frist
    while time.monotonic() < ende:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                b = os.read(fd, 4096)
            except OSError:
                break
            if not b:
                break
            bild += b
    done, st = os.waitpid(pid, os.WNOHANG)
    if not done:
        os.kill(pid, 9)
        _, st = os.waitpid(pid, 0)
    return os.waitstatus_to_exitcode(st), bild.decode("utf-8", "replace").replace("\r\n", "\n")


class AbbruchgrundDerAttrappe(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.mkdtemp(prefix="abbruch-")
        self.heim = os.path.join(self.t, "heim")
        os.makedirs(self.heim)
        self.prefix = os.path.join(self.t, "npm")
        paket_legen.lege(self.prefix)
        self.haupt = os.path.join(self.prefix, "lib", "node_modules", "@github", "copilot")
        self.loader = os.path.join(self.haupt, "npm-loader.js")
        self.programm = os.path.join(self.haupt, "node_modules", "@github", "copilot-linux-x64", "copilot")
        self.env = {"HOME": self.heim, "PATH": "/usr/local/bin:/usr/bin:/bin", "TERM": "xterm-256color",
                    "COPILOT_GITHUB_TOKEN": "gho_" + "0" * 36}

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def cache_anlegen(self):
        subprocess.run([self.loader, "--version"], env=self.env, check=True, capture_output=True, timeout=20)

    def test_entpackfehler_ohne_cache_kommt_im_bild_an(self):
        rc, bild = im_pty([self.loader], self.env)
        self.assertEqual(rc, 1)
        self.assertEqual(bild.splitlines()[0], ENTPACKEN)
        self.assertEqual(len(bild.strip().splitlines()), 1, "sonst nichts: keine Sitzung, kein Bild")

    def test_loader_ohne_plattformpaket_kommt_im_bild_an(self):
        shutil.rmtree(os.path.dirname(self.programm))
        rc, bild = im_pty([self.loader], self.env)
        self.assertEqual(rc, 1)
        self.assertEqual(bild.splitlines()[0], "Failed to find the platform package @github/copilot-linux-x64")

    def test_loader_exec_scheitert_hat_feste_zeile(self):
        os.chmod(self.programm, 0o644)  # eine gewoehnliche Datei, aber nicht ausfuehrbar: `os.execv` wirft PermissionError
        rc, bild = im_pty([self.loader], self.env)
        self.assertEqual(rc, 1)
        self.assertEqual(bild.splitlines()[0], "Failed to start the platform package")

    def test_ausnahme_in_python_hat_feste_erste_zeile(self):
        self.cache_anlegen()
        env = dict(self.env, COPILOT_ATTRAPPE_PROTOKOLL="/dev/null/nicht-da/protokoll.json")  # `makedirs` wirft vor dem ersten Bild
        rc, bild = im_pty([self.loader], env)
        zeilen = bild.splitlines()
        self.assertEqual(rc, 70)
        self.assertEqual(zeilen[0], "Unexpected error in the CLI attrappe")
        self.assertRegex(zeilen[1], r"^[A-Za-z]+(Error|Exit)$|^[A-Za-z]+$")
        self.assertNotIn("Traceback", bild)
        self.assertNotIn("/dev/null", bild, "nur der Name der Ausnahme, nie ihr Text")



class EndeWerteNurFest(unittest.TestCase):
    """Befund 4: `ende_zustand` steht nur als `laeuft`, `beendet` oder `abweichend` da, `ende_code` nur als `int` (kein
    `bool`), sonst `abweichend`; None (nicht gelesen) bleibt None. Der Weg geht ueber `Lauf.sitzung_starten` (Sitzungsliste der Bruecke)."""

    def befund(self, zustand, code):
        from unittest import mock
        from test_einstart import FalscheBruecke
        from test_enter_abstand import lauf_mit

        class Liste(FalscheBruecke):
            def sitzung(self, sid):
                return {"zustand": zustand, "code": code}
        arbeit = tempfile.mkdtemp(prefix="ende-fest-")
        self.addCleanup(shutil.rmtree, arbeit, True)
        L = lauf_mit(arbeit, Liste())
        with mock.patch.object(lauf.messen, "start_bis_eingabefeld", lambda *a, **k: {"sitzung": "s-9", "t1": None}):
            L.sitzung_starten("cloud")
        b = L.ende_befund["s-9"]
        return b["ende_zustand"], b["ende_code"]

    def test_feste_werte_bleiben(self):
        self.assertEqual(self.befund("beendet", 65), ("beendet", 65))
        self.assertEqual(self.befund("laeuft", 0), ("laeuft", 0))
        self.assertEqual(self.befund(None, None), (None, None))

    def test_anderes_wird_abweichend(self):
        text = "Fehlertext der Bruecke /pfad"
        self.assertEqual(self.befund(text, text), ("abweichend", "abweichend"))
        self.assertEqual(self.befund("beendet", True), ("beendet", "abweichend"), "bool ist kein Code")
        self.assertEqual(self.befund(["laeuft"], 1.5), ("abweichend", "abweichend"))
        self.assertEqual(self.befund({"x": 1}, "65"), ("abweichend", "abweichend"))


if __name__ == "__main__":
    unittest.main()
