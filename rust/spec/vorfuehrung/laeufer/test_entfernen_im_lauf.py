"""Der Weg "echter Lauf entfernt" laeuft gegen die Attrappen (Entscheidung vom 21.11.2026, nach einem Testlauf).

Im echten Lauf entfernt der Laeufer Dateien, die gegen die Attrappen liegen bleiben: die Zielliste der Cloud-Suche
(`schritte._zielliste_nach_suche`) und die Zaehldateien der eigenen Sitzungen (`ablauf.zaehldateien_entfernen`). In der frueheren Fassung lag die
Entfernung der Zielliste vor ihrem letzten Gebrauch (der Gegenprobe mit dem Verweis); die Tests liefen nur gegen die Attrappen und sahen
das nicht. Hier laeuft ein ganzer Lauf (S1 und E4-cloud) gegen die Attrappen dreimal:

- `ohne`: wie bisher, nichts wird entfernt.
- `echt-weg`: beide Entfernungen geschehen an ihrer Stelle im Ablauf wie im echten Lauf. Die Zaehldateien entfernt die echte Funktion
  (mit `ziel` = `echt`); die Zielliste der Attrappe traegt keine Kennung im Namen, darum entfernt der Test sie an der Stelle des Aufrufs
  selbst. Erwartet: dieselben Urteile, Gruende und Teile wie `ohne`, und beide Dateiarten sind danach weg.
- `alte-reihenfolge` (Gegenprobe): die Zielliste verschwindet direkt nach der Hauptsuche, vor der Gegenprobe, wie in der frueheren Fassung.
  Erwartet: E4-cloud `e4-cloud.schluessel_suche_fehler`. Die Attrappe der Suche (`attrappen/modellweg.py suche`) scheitert ohne
  Zielliste mit `mw.ziele_ungueltig` wie das echte Programm.

Jeder Lauf hat einen eigenen Arbeitsordner und loescht nur dort. Nur Standardbibliothek.
Aufruf: python3 -I -B test_entfernen_im_lauf.py
"""
import contextlib
import glob
import os
import sys
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, konfig, messen, probelauf, schritte  # noqa: E402

ZAEHL_MUSTER = ("s-*.suche.json", "s-*.modellweg.json")


def _bild(r):
    """Was ein Schritt nach aussen zeigt: Urteil, Code, offene Gruende als Codes, Teile."""
    return (r["ergebnis"], probelauf.fehlercode(r), probelauf.offen_codes(r), [(t["art"], t["name"], t["ok"]) for t in r["teile"]])


def lauf(art):
    """Ein Lauf S1 und E4-cloud gegen die Attrappen. Gibt (Schritte nach Kennung, Meldungen, Zielliste da, Zaehldateien, Aufrufe)."""
    arbeit = messen.neuer_tempordner(prefix="dbl-")
    try:
        k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
        zl = k["gateway"]["suche"]["zielliste"]
        zo = k["zaehlordner"]
        aufrufe = []
        with contextlib.ExitStack() as st:
            if art in ("echt-weg", "alte-reihenfolge"):
                echt_z = ablauf.zaehldateien_entfernen

                def zaehl(k_, sitzungen):
                    aufrufe.append(("zaehldateien", sorted(sitzungen)))
                    return echt_z(dict(k_, ziel="echt"), sitzungen)
                st.enter_context(mock.patch.object(ablauf, "zaehldateien_entfernen", side_effect=zaehl))
            if art == "echt-weg":
                def entfernen(L, pfad, sid, art_):
                    aufrufe.append(("zielliste", art_))
                    if art_ in ("gruen", "rot") and os.path.isfile(pfad) and not os.path.islink(pfad):
                        os.unlink(pfad)
                    return []
                st.enter_context(mock.patch.object(schritte, "_zielliste_nach_suche", side_effect=entfernen))
            if art == "alte-reihenfolge":
                echt_s = messen.schluessel_suche

                def suche(programm, quelle_option, quelle, ins, frist_s=60):
                    r = echt_s(programm, quelle_option, quelle, ins, frist_s)
                    if quelle_option == "--zielliste" and not any(a[0] == "suche" for a in aufrufe):
                        aufrufe.append(("suche", r[0]))
                        os.unlink(quelle)  # wie die fruehere Fassung: direkt nach der Hauptsuche, vor der Gegenprobe
                    return r
                st.enter_context(mock.patch.object(messen, "schluessel_suche", side_effect=suche))
            ergebnisse, meldungen, _ = ablauf.fuehre_aus(k, arbeit, [], ["E4-cloud"])
        rest = sorted(os.path.basename(p) for m in ZAEHL_MUSTER for p in glob.glob(os.path.join(zo, m)))
        return {r["schritt"]: r for r in ergebnisse}, meldungen, os.path.lexists(zl), rest, aufrufe
    finally:
        messen.sicher_loeschen(arbeit)


class EntfernenImLauf(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ohne = lauf("ohne")
        cls.weg = lauf("echt-weg")
        cls.alt = lauf("alte-reihenfolge")

    def test_ohne_entfernen_liegt_alles_und_e4_cloud_hat_alle_teile(self):
        schritte_, _, zl_da, rest, _ = self.ohne
        self.assertTrue(zl_da, "gegen die Attrappen bleibt die Zielliste")
        self.assertNotEqual(rest, [], "gegen die Attrappen bleiben die Zaehldateien")
        e4 = schritte_["E4-cloud"]
        self.assertNotIn("schluessel_suche", probelauf.offen_codes(e4))
        self.assertTrue(any(t["art"] == "gegen" and "Verweis" in t["name"] and t["ok"] for t in e4["teile"]), "Gegenprobe mit dem Verweis lief")

    def test_echt_weg_gleiches_bild_wie_ohne(self):
        for s in ("S1", "E4-cloud"):
            with self.subTest(schritt=s):
                self.assertEqual(_bild(self.weg[0][s]), _bild(self.ohne[0][s]))

    def test_echt_weg_beide_dateiarten_danach_weg(self):
        _, meldungen, zl_da, rest, aufrufe = self.weg
        self.assertFalse(zl_da, "die Zielliste ist nach dem Lauf weg")
        self.assertEqual(rest, [], "die Zaehldateien der eigenen Sitzungen sind weg")
        self.assertEqual([a for a in aufrufe if a[0] == "zielliste"], [("zielliste", "gruen")], "einmal, nach einer gelaufenen Suche")
        self.assertEqual(len([a for a in aufrufe if a[0] == "zaehldateien"]), 1)
        self.assertEqual(meldungen, [], "keine Meldung: alles war eigen und regulaer")

    def test_gegenprobe_alte_reihenfolge_ergibt_schluessel_suche_fehler(self):
        schritte_, _, zl_da, _, aufrufe = self.alt
        self.assertEqual(aufrufe[0], ("suche", "gruen"), "die Hauptsuche selbst lief gruen")
        self.assertFalse(zl_da)
        e4 = schritte_["E4-cloud"]
        self.assertEqual(probelauf.fehlercode(e4), "e4-cloud.schluessel_suche_fehler")
        self.assertEqual(e4["ergebnis"], "offen")
        self.assertIn("schluessel_suche", probelauf.offen_codes(e4))
        self.assertEqual(_bild(schritte_["S1"]), _bild(self.ohne[0]["S1"]), "S1 bleibt gleich")


if __name__ == "__main__":
    unittest.main()
