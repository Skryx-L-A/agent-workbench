"""Prueft die Entfernung der Zielliste nach der Schluesselsuche (Auflage vom 21.11.2026, Nachtrag 1): `messen.zielliste_entfernen`,
`schritte._zielliste_nach_suche` und die Reihenfolge in `schritte._suche_route` (nach der Suche und nach ihrer Gegenprobe, die die
Zielliste noch einmal liest; Testlauf vom 21.11.2026). Den Weg im ganzen Lauf gegen die Attrappen prueft `test_entfernen_im_lauf.py`.

Die festen Meldungen kommen nicht in die Liste `offen` des Schritts: sie stehen in den Meldungen des Laufs. Das Urteil von E4-cloud
haengt nur an der Suche.

Jeder Fall legt seine Dateien unter einem eigenen `mkdtemp` des Tests an und loescht nur dort. Die Suche selbst wird nicht gestartet:
`messen.schluessel_suche` ist ersetzt. Nur Standardbibliothek.
Aufruf: python3 -I -B test_zielliste.py
"""
import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import messen, schritte  # noqa: E402

HINWEIS = messen.ZIELLISTE_NICHT_ENTFERNT
NICHT_GELAUFEN = messen.ZIELLISTE_SUCHE_NICHT_GELAUFEN


class FakeL:
    """Nur die Felder, die `_zielliste_nach_suche` und `_suche_route` lesen."""

    def __init__(self, ziel="echt", s1="s-1", art=None):
        self.k = {"ziel": ziel, "grenzen": {"suche_s": 60}}
        self.s1 = s1
        self.art_von = {s1: "cloud"} if art is None else art


def schreibe(pfad, text="x"):
    with open(pfad, "w", encoding="utf-8") as f:
        f.write(text)


class ZiellisteNachSuche(unittest.TestCase):
    def setUp(self):
        self.basis = tempfile.mkdtemp(prefix="zielliste-test-")
        self.addCleanup(shutil.rmtree, self.basis, True)
        self.sitz = os.path.join(self.basis, "sitzungen")
        os.mkdir(self.sitz)
        os.chmod(self.sitz, 0o700)

    def pfad(self, sid):
        return os.path.join(self.sitz, "%s.zielliste.json" % sid)

    def zielliste(self, sid="s-1"):
        p = self.pfad(sid)
        schreibe(p, json.dumps({"fassung": 1, "ziele": []}) + "\n")
        return p

    def schritt(self):
        return schritte.Schritt("E4-cloud", "Das Token der CLI bleibt draussen (cloud)")

    def test_eigene_sitzung_nach_gruener_suche_weg(self):
        p = self.zielliste()
        self.assertEqual(schritte._zielliste_nach_suche(FakeL(), p, "s-1", "gruen"), [])
        self.assertFalse(os.path.lexists(p))

    def test_rot_gilt_auch_als_gelaufen(self):
        p = self.zielliste()
        self.assertEqual(schritte._zielliste_nach_suche(FakeL(), p, "s-1", "rot"), [])
        self.assertFalse(os.path.lexists(p))

    def test_suche_nicht_gelaufen_bleibt_mit_festem_hinweis(self):
        for art in ("fehler", "unlesbar"):
            with self.subTest(art=art):
                p = self.zielliste()
                hinweise = schritte._zielliste_nach_suche(FakeL(), p, "s-1", art)
                self.assertTrue(os.path.isfile(p))
                self.assertEqual(hinweise, [NICHT_GELAUFEN])
                self.assertNotIn(self.basis, " ".join(hinweise))
                os.unlink(p)

    def test_fehlende_datei_gibt_keinen_hinweis(self):
        for art in ("fehler", "gruen"):
            with self.subTest(art=art):
                self.assertEqual(schritte._zielliste_nach_suche(FakeL(), self.pfad("s-1"), "s-1", art), [])

    def test_fremde_sitzung_bleibt(self):
        p2 = self.zielliste("s-2")
        L = FakeL(s1="s-1", art={"s-1": "cloud", "s-2": "cloud"})
        self.assertEqual(schritte._zielliste_nach_suche(L, p2, "s-2", "gruen"), [])
        self.assertTrue(os.path.isfile(p2))

    def test_lokale_sitzung_bleibt(self):
        p = self.zielliste()
        self.assertEqual(schritte._zielliste_nach_suche(FakeL(art={"s-1": "lokal"}), p, "s-1", "gruen"), [])
        self.assertTrue(os.path.isfile(p))

    def test_attrappe_entfernt_nichts(self):
        p = self.zielliste()
        self.assertEqual(schritte._zielliste_nach_suche(FakeL(ziel="attrappe"), p, "s-1", "gruen"), [])
        self.assertEqual(schritte._zielliste_nach_suche(FakeL(ziel="attrappe"), p, "s-1", "fehler"), [])
        self.assertTrue(os.path.isfile(p))

    def test_link_bleibt_und_sein_ziel_unveraendert(self):
        ziel = os.path.join(self.basis, "fremd.txt")
        schreibe(ziel, "unveraendert")
        p = self.pfad("s-1")
        os.symlink(ziel, p)
        hinweise = schritte._zielliste_nach_suche(FakeL(), p, "s-1", "gruen")
        self.assertTrue(os.path.islink(p))
        with open(ziel, encoding="utf-8") as f:
            self.assertEqual(f.read(), "unveraendert")
        self.assertEqual(hinweise, [HINWEIS])

    def test_ordner_mit_dem_namen_bleibt(self):
        p = self.pfad("s-1")
        os.mkdir(p)
        schreibe(os.path.join(p, "datei"))
        hinweise = schritte._zielliste_nach_suche(FakeL(), p, "s-1", "gruen")
        self.assertTrue(os.path.isdir(p))
        self.assertTrue(os.path.isfile(os.path.join(p, "datei")))
        self.assertEqual(hinweise, [HINWEIS])

    def test_fremde_uid_bleibt_mit_hinweis(self):
        p = self.zielliste()
        with mock.patch("os.getuid", return_value=os.getuid() + 1):
            hinweise = schritte._zielliste_nach_suche(FakeL(), p, "s-1", "gruen")
        self.assertTrue(os.path.isfile(p))
        self.assertEqual(hinweise, [HINWEIS])

    def test_kennung_mit_dotdot_tut_nichts(self):
        ausserhalb = os.path.join(self.basis, "x.zielliste.json")
        schreibe(ausserhalb)
        hinweise = schritte._zielliste_nach_suche(FakeL(s1="../x"), os.path.join(self.sitz, "../x.zielliste.json"), "../x", "gruen")
        self.assertTrue(os.path.isfile(ausserhalb))
        self.assertEqual(hinweise, [HINWEIS])

    def test_elternordner_als_symlink_bleibt(self):
        p = self.zielliste("s-1")
        link = os.path.join(self.basis, "link-auf-sitzungen")
        os.symlink(self.sitz, link)
        entfernt, hinweis = messen.zielliste_entfernen(os.path.join(link, "s-1.zielliste.json"), "s-1")
        self.assertFalse(entfernt)
        self.assertEqual(hinweis, HINWEIS)
        self.assertTrue(os.path.isfile(p))

    def test_kennung_nach_festem_muster(self):
        p = self.zielliste("s-1")
        for sid in ("s-1234567890", "S-1", "s-", "s-x", "x", "s-1 "):
            with self.subTest(sid=sid):
                entfernt, hinweis = messen.zielliste_entfernen(p, sid)
                self.assertFalse(entfernt)
                self.assertEqual(hinweis, HINWEIS)
        self.assertTrue(os.path.isfile(p))
        entfernt, hinweis = messen.zielliste_entfernen(p, "s-1")
        self.assertTrue(entfernt)
        self.assertIsNone(hinweis)
        self.assertFalse(os.path.lexists(p))

    def test_zielliste_erst_nach_der_gegenprobe_weg(self):
        """Die Suche und die Gegenprobe mit dem Verweis lesen beide die Zielliste (`--zielliste <datei>`). Die Datei liegt bei beiden
        Aufrufen noch da und ist erst danach weg (in der frueheren Fassung war sie bei der Gegenprobe schon weg)."""
        p = self.zielliste()
        verweis = os.path.join(self.basis, "schluessel")
        schreibe(verweis, "x")
        L = FakeL()
        s = self.schritt()
        gesehen, hinweise = [], []

        def suche(programm, quelle_option, quelle, ins, frist_s=60):
            gesehen.append(os.path.lexists(p))
            return "gruen", {"dateien": 0, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": len(gesehen) - 1}

        with mock.patch.object(messen, "schluessel_suche", side_effect=suche):
            schritte._suche_route(L, s, "cloud", "--zielliste", p, [self.basis], verweis, ["prog"], "E4-cloud",
                                  nach_suche=lambda art_: hinweise.extend(schritte._zielliste_nach_suche(L, p, "s-1", art_)))
        self.assertEqual(gesehen, [True, True])
        self.assertFalse(os.path.lexists(p))
        self.assertEqual(hinweise, [])
        self.assertEqual(s.offen, [])
        self.assertEqual(s.ergebnis(), "gruen")

    def test_entfernt_auch_wenn_die_gegenprobe_mit_ausnahme_endet(self):
        """Endet die Gegenprobe mit einer Ausnahme, laeuft `nach_suche` trotzdem (finally), und die Ausnahme bleibt sichtbar."""
        p = self.zielliste()
        verweis = os.path.join(self.basis, "schluessel")
        schreibe(verweis, "x")
        L = FakeL()
        aufrufe = []
        with mock.patch.object(messen, "schluessel_suche", return_value=("gruen", {"dateien": 0, "nicht_lesbar": 0, "treffer": 0,
                                                                                  "uebersprungen": 0})), \
                mock.patch.object(messen, "neuer_tempordner", side_effect=RuntimeError("Probe")):
            with self.assertRaises(RuntimeError):
                schritte._suche_route(L, self.schritt(), "cloud", "--zielliste", p, [self.basis], verweis, ["prog"], "E4-cloud",
                                      nach_suche=lambda art_: aufrufe.append(schritte._zielliste_nach_suche(L, p, "s-1", art_)))
        self.assertEqual(aufrufe, [[]])
        self.assertFalse(os.path.lexists(p))

    def test_link_hat_dasselbe_urteil_wie_ohne_zielliste(self):
        """Nachtrag 1: Ein Hinweis zur Zielliste aendert das Urteil von E4-cloud nicht. Gleiche Suche, gleicher Schritt, mit und ohne
        Link als Zielliste: der Schritt ist in beiden Faellen gleich (ergebnis, offen, Gegenprobe)."""
        ergebnisse = {}
        for fall in ("ohne", "link"):
            with self.subTest(fall=fall):
                L = FakeL()
                s = self.schritt()
                ziel = os.path.join(self.basis, "fremd-%s.txt" % fall)
                schreibe(ziel, "unveraendert")
                p = self.pfad("s-1")
                if fall == "link":
                    os.symlink(ziel, p)
                verweis = os.path.join(self.basis, "schluessel-%s" % fall)
                schreibe(verweis, "x")
                hinweise = []

                def suche(programm, quelle_option, quelle, ins, frist_s=60):
                    return "gruen", {"dateien": 0, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": 1}

                with mock.patch.object(messen, "schluessel_suche", side_effect=suche):
                    schritte._suche_route(L, s, "cloud", "--zielliste", p, [self.basis], verweis, ["prog"], "E4-cloud",
                                          nach_suche=lambda art_: hinweise.extend(schritte._zielliste_nach_suche(L, p, "s-1", art_)))
                ergebnisse[fall] = (s.ergebnis(), s.gegenprobe(), list(s.offen), hinweise)
        self.assertEqual(ergebnisse["link"][:3], ergebnisse["ohne"][:3])
        self.assertEqual(ergebnisse["link"][3], [HINWEIS])
        self.assertEqual(ergebnisse["ohne"][3], [])


if __name__ == "__main__":
    unittest.main()
