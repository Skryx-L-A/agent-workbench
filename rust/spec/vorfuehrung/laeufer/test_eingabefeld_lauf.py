"""Ganzer Lauf gegen die Attrappen, in dem keine Sitzung ein Eingabefeld zeigt (Entscheidung, Punkt b): Die Merkmale der Harness
(Eingabefeld und Vertrauensfrage) stehen auf einem Muster, das nie passt. Dann geht im ganzen Lauf keine einzige Eingabe an die Bruecke,
auch kein Enter, und jeder Schritt, der senden wuerde, endet mit einem festen Code, nie mit `.ausnahme`.
Gegenprobe ohne Fix (Laeufer vor dieser Korrektur): die Eingaben gehen raus, dieser Test wird rot.
Zweiter Fall: nur das Eingabefeld passt nie, die Vertrauensfrage der Attrappe wird erkannt. Dann geht hoechstens ein Enter je Cloud-Start
raus (die Antwort auf eine ERKANNTE Vertrauensfrage, Entscheidung Punkt a), nie ein Text.

Benutzt keine neuen Namen des Laeufers, damit die Gegenprobe mit dem alten Laeufer aussagekraeftig ist. Nur Standardbibliothek.
Aufruf: python3 -I -B test_eingabefeld_lauf.py
"""
import os
import sys
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, bruecke, konfig, messen, probelauf  # noqa: E402

NIE = "NIEMALS-DIESES-MERKMAL-7Q2X"
SENDENDE = ("S1", "S5", "E4-lokal", "E4-cloud", "E5", "E6")


def lauf(vertrauen_auch):
    arbeit = messen.neuer_tempordner(prefix="dbl-")
    gesendet = []
    echt = bruecke.Bruecke.eingabe

    def zaehle(self, sid, text):
        gesendet.append(text)
        return echt(self, sid, text)
    try:
        k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
        k["bruecke"]["frist_s"] = 2
        for h in k["harness"].values():
            h["eingabefeld_muster"] = [NIE]
            if vertrauen_auch and h.get("vertrauensfrage_muster"):
                h["vertrauensfrage_muster"] = NIE
        with mock.patch.object(bruecke.Bruecke, "eingabe", autospec=True, side_effect=zaehle):
            ergebnisse, _, info = ablauf.fuehre_aus(k, arbeit, [], [s for s in SENDENDE if s != "S1"])
        return {r["schritt"]: r for r in ergebnisse}, gesendet, info
    finally:
        messen.sicher_loeschen(arbeit)


class OhneEingabefeld(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ganz = lauf(vertrauen_auch=True)
        cls.nur_feld = lauf(vertrauen_auch=False)

    def test_keine_eingabe_im_ganzen_lauf(self):
        _, gesendet, info = self.ganz
        self.assertEqual(gesendet, [], "ohne erkanntes Merkmal geht nichts an die Bruecke, auch kein Enter")
        self.assertEqual(info["cloud_eingaben"], 0)

    def test_jeder_sendende_schritt_hat_festen_code(self):
        schritte_, _, _ = self.ganz
        for s in SENDENDE:
            with self.subTest(schritt=s):
                r = schritte_[s]
                code = probelauf.fehlercode(r)
                self.assertNotEqual(r["ergebnis"], "gruen")
                self.assertIn(code, probelauf.FEHLERCODES)
                self.assertFalse(code.endswith(".ausnahme"), code)
        self.assertEqual(probelauf.fehlercode(schritte_["S1"]), "s1.kein_eingabefeld")

    def test_s1_behaelt_seine_teile(self):
        r = self.ganz[0]["S1"]
        namen = {t["name"]: t["ok"] for t in r["teile"]}
        self.assertIs(namen.get("Start angenommen"), True)
        self.assertIs(namen.get("Eingabefeld erscheint"), False)

    def test_nur_enter_auf_erkannte_vertrauensfrage(self):
        _, gesendet, info = self.nur_feld
        self.assertTrue(all(t == "" for t in gesendet), "nur Enter, nie ein Text: %r" % [len(t) for t in gesendet])
        self.assertLessEqual(len(gesendet), info["starts"], "hoechstens ein Enter je Cloud-Start")
        self.assertEqual(info["cloud_eingaben"], 0)


if __name__ == "__main__":
    unittest.main()
