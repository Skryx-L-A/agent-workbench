"""S1 klickt den neuen Eintrag `sitzung-<id>` an, wie die echte Seite es verlangt (Vertrag der Seite: kein selbsttaetiger Wechsel).

Die Seite der Attrappe hat den Schalter `attrappe.kein_selbstwechsel` (Konfiguration, Vorgabe aus; Kommandozeile `--kein-selbstwechsel`).
Ist er gesetzt, wechselt sie nicht selbst in die neue Sitzung. Der Test faehrt S1 mit Playwright gegen die Attrappe:
  - Schalter an, neues seite.py: S1 gruen.
  - Schalter an, seite.py ohne den Klick (Stand vor dieser Korrektur): S1 rot mit `KeinEingabefeld` (Gegenprobe).
  - Schalter aus: S1 mit beiden gruen.
Der alte Stand entsteht aus dem neuen seite.py, indem die zwei Zeilen des Klicks durch `self.aktuell = neu[0]` ersetzt werden; gebaut
wird in einer Kopie des Laeuferordners, nie am Original. Braucht Node und Playwright (`browser.playwright_pfad`); fehlen sie, scheitert
der Test mit klarem Text, er wird nie uebersprungen. Nur Standardbibliothek. Aufruf: python3 -I -B test_sitzung_klick.py
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

from drehbuchlib import seite as seite_modul  # noqa: E402

NEU = """                self.helfer.rufe("klicken", testid=s["eintrag"].replace("{id}", neu[0]))
                self.aktuell = neu[0]
"""
ALT = """                self.aktuell = neu[0]
"""
VAULT = os.path.normpath(os.path.join(HIER, "..", "..", "such-socket", "vorfuehr-vault"))


def kopie(wurzel, alt):
    ziel = os.path.join(wurzel, "alt" if alt else "neu")
    shutil.copytree(HIER, ziel, ignore=shutil.ignore_patterns("._*", "__pycache__"))
    if alt:
        pfad = os.path.join(ziel, "drehbuchlib", "seite.py")
        with open(pfad, encoding="utf-8") as f:
            q = f.read()
        assert NEU in q, "seite.py hat den Klick nicht in der erwarteten Form"
        with open(pfad, "w", encoding="utf-8") as f:
            f.write(q.replace(NEU, ALT))
    return ziel


def s1(baum, schalter, wurzel):
    """Faehrt nur S1 mit Browser. Gibt (Rueckgabewert, Ergebnis von S1 aus dem JSON-Bericht, Text von stdout und stderr)."""
    bericht = os.path.join(wurzel, "bericht-%s-%s.json" % (os.path.basename(baum), schalter))
    arbeit = tempfile.mkdtemp(prefix="dbl-", dir=wurzel)
    arg = [sys.executable, "-I", "-B", os.path.join(baum, "drehbuch.py"), "--ziel", "attrappe", "--browser", "playwright", "--nur", "S1",
           "--vault", VAULT, "--arbeit", arbeit, "--json", bericht]
    if schalter:
        arg.append("--kein-selbstwechsel")
    p = subprocess.run(arg, capture_output=True, text=True, timeout=240, cwd=baum, env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"),
                       "HOME": os.environ.get("HOME", "/"), "TMPDIR": wurzel})
    ausgabe = p.stdout + p.stderr
    try:
        with open(bericht, encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        return p.returncode, None, ausgabe
    erg = next((r for r in d.get("schritte", d.get("ergebnisse", [])) if r.get("schritt") == "S1"), None)
    return p.returncode, erg, ausgabe


class SitzungKlick(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        ok, grund = seite_modul.verfuegbar(json.load(open(os.path.join(HIER, "konfig.attrappe.json"))))
        if not ok:
            raise AssertionError("Browser nicht verfuegbar (%s): dieser Test braucht Playwright" % grund)
        cls.wurzel = tempfile.mkdtemp(prefix="sk-")
        cls.neu = kopie(cls.wurzel, False)
        cls.alt = kopie(cls.wurzel, True)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.wurzel, ignore_errors=True)

    def pruefe(self, baum, schalter, gruen):
        rc, erg, text = s1(baum, schalter, self.wurzel)
        self.assertIsNotNone(erg, "kein Ergebnis fuer S1: " + text[-800:])
        if gruen:
            self.assertEqual((rc, erg["ergebnis"]), (0, "gruen"), text[-1500:])
        else:
            self.assertEqual(erg["ergebnis"], "rot", text[-1500:])
            self.assertIn("KeinEingabefeld", json.dumps(erg) + text, text[-1500:])

    def test_mit_schalter_neu_gruen(self):
        self.pruefe(self.neu, True, True)

    def test_mit_schalter_alt_rot_kein_eingabefeld(self):
        self.pruefe(self.alt, True, False)

    def test_ohne_schalter_neu_gruen(self):
        self.pruefe(self.neu, False, True)

    def test_ohne_schalter_alt_gruen(self):
        self.pruefe(self.alt, False, True)


class FalscherHelfer:
    """Ersatz fuer den Node-Helfer: Das Fehlerfeld ist schon vor dem Klick sichtbar (Fehler eines frueheren Starts)."""

    def __init__(self, sel, neuer_eintrag, fehler_nachher):
        self.sel, self.neuer_eintrag, self.fehler_nachher = sel, neuer_eintrag, fehler_nachher
        self.liste_n, self.geklickt, self.gestartet, self.nach_klick = 0, [], False, 0

    def rufe(self, op, frist_s=30, **a):
        t = a.get("testid")
        if op == "klicken":
            self.geklickt.append(t)
            self.gestartet = self.gestartet or t == self.sel["start"]
            return None
        if op == "liste":
            self.liste_n += 1
            self.nach_klick += 1 if self.gestartet else 0
            # Der Eintrag erscheint erst nach der Antwort der Bruecke, also nach dem ersten Abruf im Warten (Fehlerfeld steht dann schon da)
            return [{"id": "s-1"}] + ([{"id": "s-2"}] if self.neuer_eintrag and self.nach_klick >= 2 else [])
        if op == "sichtbar":
            return True
        if op == "text":
            code, text = self.fehler_nachher if self.gestartet else ("start.route_unbekannt", "Route unbekannt")
            return code if t == self.sel["fehler_code"] else text
        return None


class AlterFehler(unittest.TestCase):
    """Die echte Seite blendet das Fehlerfeld erst bei einem Erfolg aus. Ein Fehler eines frueheren Starts darf den neuen Start nicht als
    Ablehnung erscheinen lassen (gemessen in S3: Antwort 200 der Bruecke, aber `status_gegenprobe=422`)."""

    def bruecke(self, neuer_eintrag, fehler_nachher):
        sel = json.load(open(os.path.join(HIER, "konfig.attrappe.json")))["browser"]["selektoren"]
        b = seite_modul.SeiteBruecke.__new__(seite_modul.SeiteBruecke)
        b.sel, b.frist, b.verlaeufe, b.aktuell = sel, 0.3, {}, None
        b.helfer = FalscherHelfer(sel, neuer_eintrag, fehler_nachher)
        start = {"harness": "copilot", "route": "copilot", "modell": "m", "kunde": None, "arbeitsordner": "/x"}
        return b, b.starten(start)

    def test_alter_fehler_neuer_eintrag_ist_ein_erfolg(self):
        b, (code, antw, _) = self.bruecke(True, ("start.route_unbekannt", "Route unbekannt"))
        self.assertEqual((code, antw), (200, {"sitzung": "s-2"}))
        self.assertIn("sitzung-s-2", b.helfer.geklickt)
        self.assertEqual(b.aktuell, "s-2")

    def test_anderer_fehler_nach_dem_klick_ist_die_ablehnung(self):
        _, (code, antw, _) = self.bruecke(False, ("start.kennung_form", "Kennung hat falsche Form"))
        self.assertEqual((code, antw["code"]), (422, "start.kennung_form"))

    def test_gleicher_fehler_ohne_neuen_eintrag_gilt_nach_der_frist(self):
        _, (code, antw, _) = self.bruecke(False, ("start.route_unbekannt", "Route unbekannt"))
        self.assertEqual((code, antw["code"]), (422, "start.route_unbekannt"))


class Quelle(unittest.TestCase):
    def test_vorgabe_aus(self):
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            self.assertIs(json.load(f)["attrappe"]["kein_selbstwechsel"], False)

    def test_aktuell_erst_nach_dem_klick(self):
        with open(os.path.join(HIER, "drehbuchlib", "seite.py"), encoding="utf-8") as f:
            q = f.read()
        self.assertIn(NEU, q)


if __name__ == "__main__":
    unittest.main()
