"""Teilzeiten von S1 in der Diagnosedatei (23.11.2026).

S1 misst seine Teile mit `time.monotonic()` unter festen Namen (`s1.bruecke_bereit`, `s1.start_gesamt`, `s1.start_eingabefeld`,
`s1.vertrauensfrage`, `s1.zug<n>.sichtbar|enter|abgesendet|fertig|gesamt`, `s1.ausgang_ca`). Sie stehen nur in der Diagnosedatei und nur,
wenn die ohnehin entsteht (Ausnahme oder Zeitueberschreitung). Ein gruener Lauf legt weiter keine Diagnosedatei an. Ergebnisdatei,
Bericht und Konsole tragen keine Teilzeit; die Ergebnisdatei behaelt fuenf Schluessel je Schritt.

Fall a: Zug 1 von S1 laeuft in die Frist (Bericht und Weg des Pakets). Fall b: Zug 2 des knappen Cloud-Laufs laeuft in die Frist.
Fall c: S1 gruen, keine Diagnosedatei. Fall d: `diagnose_schreibe` unmittelbar (Teilzeiten allein legen nichts an, die Grenze kuerzt sie nie).
Nur Standardbibliothek. Aufruf: python3 -I -B test_s1_teilzeiten.py   (startet die Attrappen: einige Sekunden je Lauf)
"""
import contextlib
import io
import json
import os
import re
import shutil
import stat
import sys
import tempfile
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

import drehbuch  # noqa: E402
from drehbuchlib import bruecke, lauf, messen, probelauf, schritte  # noqa: E402

ZEILE = re.compile(r"^teilzeit\.([a-z][a-z0-9._-]{0,63}): (\d+\.\d{3})$")
KEINE = re.compile(r"teilzeit\.|s1\.zug\d")  # weder Zeile noch Name einer Teilzeit (der Pfad des Worktrees darf `teilzeiten` enthalten)
VOR_DEM_ZUG = ("s1.bruecke_bereit", "s1.start_gesamt", "s1.start_eingabefeld")


@contextlib.contextmanager
def s1_zug_in_die_frist(nr):
    """In S1 verliert der Verlauf ab Zug `nr` das Wort FERTIG; die Frist jedes Zugs ist 1 s."""
    scharf = {"an": False, "zug": 0}
    echt = bruecke.Bruecke.verlauf
    echt_zug = lauf.Lauf.zug

    def verlauf(self, sid, ab=0):
        n, t = echt(self, sid, ab)
        return (n, t.replace("FERTIG", "FERT1G")) if scharf["an"] and scharf["zug"] >= nr else (n, t)

    def zug(self, sid, art, arg, vorlage=None, frist_s=None):
        scharf["zug"] += 1
        return echt_zug(self, sid, art, arg, vorlage, 1.0 if scharf["an"] and scharf["zug"] >= nr else frist_s)
    s1 = dict(schritte.REIHENFOLGE)["S1"]

    def s1_scharf(L):
        scharf["an"] = True
        try:
            return s1(L)
        finally:
            scharf["an"] = False
    folge = [(n, s1_scharf if n == "S1" else f) for n, f in schritte.REIHENFOLGE]
    with mock.patch.object(bruecke.Bruecke, "verlauf", autospec=True, side_effect=verlauf), \
            mock.patch.object(lauf.Lauf, "zug", zug), mock.patch.object(schritte, "REIHENFOLGE", folge):
        yield


class Werkstatt(unittest.TestCase):
    knapp = False

    def setUp(self):
        self.ordner = tempfile.mkdtemp(prefix="s1tz-")
        self.addCleanup(shutil.rmtree, self.ordner, True)
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        if self.knapp:  # knapper Cloud-Lauf: Zug 1 `zaehlen`, Zug 2 das Buendel
            k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        self.konfig = os.path.join(self.ordner, "k.json")
        with open(self.konfig, "w", encoding="utf-8") as f:
            json.dump(k, f)

    def bericht(self, wirkung=None):
        """`drehbuch.py --ziel attrappe --nur S1 --json`: gibt (Konsole, Pfad des Berichts, Bericht als Text)."""
        pfad = os.path.join(self.ordner, "bericht.json")
        aus = io.StringIO()
        with contextlib.ExitStack() as st:
            if wirkung:
                st.enter_context(wirkung)
            st.enter_context(contextlib.redirect_stdout(aus))
            st.enter_context(contextlib.redirect_stderr(aus))
            drehbuch.main(["--ziel", "attrappe", "--konfig", self.konfig, "--nur", "S1", "--json", pfad])
        with open(pfad, encoding="utf-8") as f:
            return aus.getvalue(), pfad, f.read()

    def paket(self, wirkung=None):
        """Der Weg des Pakets (`probelauf.probelauf`): gibt (Konsole, Pfad der Ergebnisdatei, Datei als Text)."""
        pfad = os.path.join(self.ordner, "ergebnis.json")
        aus = io.StringIO()
        with contextlib.ExitStack() as st:
            if wirkung:
                st.enter_context(wirkung)
            st.enter_context(contextlib.redirect_stdout(aus))
            st.enter_context(contextlib.redirect_stderr(aus))
            probelauf.probelauf(["--ergebnis", pfad, "--konfig", self.konfig, "--nur", "S1"], HIER)
        with open(pfad, encoding="utf-8") as f:
            return aus.getvalue(), pfad, f.read()

    def teilzeiten(self, pfad, konsole, roh):
        """Liest die Teilzeiten der Diagnosedatei als Dict und prueft, dass keine Teilzeit in Konsole oder Bericht steht."""
        self.assertIsNone(KEINE.search(konsole))
        self.assertIsNone(KEINE.search(roh))
        for name in VOR_DEM_ZUG:
            self.assertNotIn(name, konsole)
            self.assertNotIn(name, roh)
        d = pfad + ".diagnose"
        self.assertTrue(os.path.isfile(d), "Diagnosedatei fehlt")
        self.assertEqual(stat.S_IMODE(os.stat(d).st_mode), 0o600)
        with open(d, encoding="utf-8") as f:
            zeilen = f.read().splitlines()
        tz = [z for z in zeilen if z.startswith("teilzeit.")]
        self.assertTrue(tz, "keine Teilzeit in der Diagnosedatei")
        self.assertEqual(zeilen[-len(tz):], tz, "die Teilzeiten stehen am Ende")
        werte = {}
        for z in tz:
            m = ZEILE.match(z)
            self.assertIsNotNone(m, "Teilzeit hat die Form `teilzeit.<fester name>: <sekunden>`")
            werte[m.group(1)] = float(m.group(2))
        self.assertTrue(all(name.startswith("s1.") for name in werte))
        return werte


class Zug1InDieFrist(Werkstatt):
    def pruefe(self, werte):
        # `abgesendet` ist gegen die Attrappe der Bruecke "nicht feststellbar" (README, Enter mit Abstand) und fehlt dort; gemessen wird
        # es mit der nachgebildeten CLI unten (`Zugzeiten`).
        for name in VOR_DEM_ZUG + ("s1.zug1.sichtbar", "s1.zug1.enter", "s1.zug1.gesamt"):
            self.assertIn(name, werte)
        self.assertNotIn("s1.zug1.fertig", werte, "der Zug kam nie zu FERTIG")
        self.assertNotIn("s1.zug2.gesamt", werte)
        self.assertNotIn("s1.ausgang_ca", werte, "nach der Ausnahme laeuft der Ausgang nicht mehr")
        self.assertLessEqual(werte["s1.zug1.sichtbar"], werte["s1.zug1.enter"])
        self.assertGreaterEqual(werte["s1.zug1.gesamt"], 1.0, "die Frist von 1 s steckt im Zug")
        self.assertLessEqual(werte["s1.start_eingabefeld"], werte["s1.start_gesamt"])

    def test_a_bericht(self):
        konsole, pfad, roh = self.bericht(s1_zug_in_die_frist(1))
        s1 = {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S1"]
        self.assertEqual(s1["fehler"], "Zeitueberschreitung: s1.zug_frist")
        self.pruefe(self.teilzeiten(pfad, konsole, roh))
        self.assertIn("diagnose: angelegt", konsole.splitlines())

    def test_a_paket(self):
        konsole, pfad, roh = self.paket(s1_zug_in_die_frist(1))
        d = json.loads(roh)
        s1 = {s["schritt"]: s for s in d["schritte"]}["S1"]
        self.assertEqual(s1["fehlercode"], "s1.zug_frist")
        for s in d["schritte"]:
            self.assertEqual(set(s), {"schritt", "bestanden", "fehlercode", "dauer_ms", "offen"})
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.pruefe(self.teilzeiten(pfad, konsole, roh))


class Zug2InDieFrist(Werkstatt):
    knapp = True

    def test_b_knapp_zug2(self):
        konsole, pfad, roh = self.bericht(s1_zug_in_die_frist(2))
        s1 = {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S1"]
        self.assertEqual(s1["fehler"], "Zeitueberschreitung: s1.zug_frist")
        werte = self.teilzeiten(pfad, konsole, roh)
        for teil in ("sichtbar", "enter", "fertig", "gesamt"):
            self.assertIn("s1.zug1." + teil, werte)
        for teil in ("sichtbar", "enter", "gesamt"):
            self.assertIn("s1.zug2." + teil, werte)
        self.assertNotIn("s1.zug2.fertig", werte)
        self.assertLessEqual(werte["s1.zug1.fertig"], werte["s1.zug1.gesamt"])


class S1Gruen(Werkstatt):
    def test_c_keine_diagnosedatei(self):
        konsole, pfad, roh = self.bericht()
        s1 = {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S1"]
        self.assertEqual(s1["ergebnis"], "gruen")
        self.assertFalse(os.path.lexists(pfad + ".diagnose"))
        self.assertNotIn("diagnose:", konsole)
        self.assertIsNone(KEINE.search(konsole + roh))
        konsole2, pfad2, roh2 = self.paket()
        self.assertFalse(os.path.lexists(pfad2 + ".diagnose"))
        self.assertNotIn("diagnose:", konsole2)
        self.assertIsNone(KEINE.search(konsole2 + roh2))
        for s in json.loads(roh2)["schritte"]:
            self.assertEqual(set(s), {"schritt", "bestanden", "fehlercode", "dauer_ms", "offen"})


class Zugzeiten(unittest.TestCase):
    """Alle vier Teile eines Zugs mit der nachgebildeten CLI aus `test_enter_abstand` (dort leert sich das Feld nach dem Enter)."""
    def setUp(self):
        from test_enter_abstand import NachgebildeteCli, lauf_mit
        self.arbeit = tempfile.mkdtemp(prefix="s1tz-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)
        messen.diagnose_leeren()
        self.addCleanup(messen.diagnose_leeren)
        self.b = NachgebildeteCli("cli")
        self.L = lauf_mit(self.arbeit, self.b)

    def test_vier_teile_in_ihrer_reihenfolge(self):
        schritte._zug_gemessen(self.L, 1, self.L.zug, "s-1", "zaehlen", "")
        self.assertEqual(self.L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": True})
        werte = dict(messen.teilzeiten())
        self.assertEqual(list(werte), ["s1.zug1.sichtbar", "s1.zug1.enter", "s1.zug1.abgesendet", "s1.zug1.fertig", "s1.zug1.gesamt"])
        z = [werte[n] for n in werte]
        self.assertEqual(z, sorted(z), "jede Teilzeit zaehlt ab Beginn des Zugs")
        self.assertGreaterEqual(werte["s1.zug1.enter"] - werte["s1.zug1.sichtbar"], 0.29, "der Abstand vor dem Enter steckt darin")
        self.assertIsNone(messen.diagnose_schreibe(os.path.join(self.arbeit, "e.json")), "gruener Zug: keine Diagnosedatei")

    def test_zug_in_die_frist_behaelt_seine_teile(self):
        self.b.verlauf = lambda sid, ab=0: (0, "")  # kein FERTIG
        with self.assertRaises(lauf.Zeitueberschreitung):
            schritte._zug_gemessen(self.L, 2, self.L.zug, "s-1", "zaehlen", "", frist_s=0.5)
        werte = dict(messen.teilzeiten())
        self.assertEqual(list(werte), ["s1.zug2.sichtbar", "s1.zug2.enter", "s1.zug2.abgesendet", "s1.zug2.gesamt"])

    def test_gesamt_liegt_nie_unter_einem_gerundeten_teil(self):
        """Fester Fall zum Zeitfehler vom 23.11.: Teil `fertig` gerundet 0,416, Zug ungerundet 0,4157. `gesamt` rundet ebenso."""
        class Ein:
            zugzeiten = {"fertig": 0.416}
        with mock.patch.object(schritte.time, "monotonic", side_effect=[10.0, 10.4157]):
            schritte._zug_gemessen(Ein(), 1, lambda: None)
        werte = dict(messen.teilzeiten())
        self.assertGreaterEqual(werte["s1.zug1.gesamt"], werte["s1.zug1.fertig"])
        self.assertEqual(werte["s1.zug1.gesamt"], 0.416)


class DiagnoseSchreibe(unittest.TestCase):
    def setUp(self):
        self.ordner = tempfile.mkdtemp(prefix="s1tz-")
        self.addCleanup(shutil.rmtree, self.ordner, True)
        messen.diagnose_leeren()
        self.addCleanup(messen.diagnose_leeren)

    def test_d_teilzeiten_allein_legen_nichts_an(self):
        messen.teilzeit_notiere("s1.start_gesamt", 1.5)
        pfad = os.path.join(self.ordner, "e.json")
        self.assertIsNone(messen.diagnose_schreibe(pfad))
        self.assertFalse(os.path.lexists(pfad + ".diagnose"))

    def test_d_nur_feste_namen_und_zahlen(self):
        for name, wert in (("s1.a", 1), ("S1 frei", 2.0), ("s1.b", "3"), ("s1.c", None), ("s1.d", True), ("s1.e", float("nan")),
                           ("s1.a", 2.25)):
            messen.teilzeit_notiere(name, wert)
        self.assertEqual(messen.teilzeiten(), [("s1.a", 2.25)])

    def test_d_grenze_kuerzt_die_teilzeiten_nie(self):
        messen.diagnose_notiere("lauf.verlauf", "x" * (2 * messen.DIAGNOSE_GRENZE))
        messen.teilzeit_notiere("s1.zug1.fertig", 0.125)
        pfad = os.path.join(self.ordner, "e.json")
        self.assertEqual(messen.diagnose_schreibe(pfad), "angelegt")
        with open(pfad + ".diagnose", "rb") as f:
            roh = f.read()
        self.assertLessEqual(len(roh), messen.DIAGNOSE_GRENZE)
        self.assertTrue(roh.endswith(b"teilzeit.s1.zug1.fertig: 0.125\n"))
        messen.diagnose_leeren()
        self.assertEqual(messen.teilzeiten(), [])


if __name__ == "__main__":
    unittest.main()
