"""Prueft zwei Korrekturen aus einem Testlauf (Entscheidung vom 21.11.2026):

- Punkt 2: die Zaehldateien `<zaehlordner>/<sitzung>.suche.json` und `<sitzung>.modellweg.json` der Sitzungen, die ein Lauf selbst
  gestartet hat, werden nach dem Lauf entfernt (`ablauf.zaehldateien_entfernen`, `messen.sitzungsdatei_entfernen`). Nur im echten Lauf,
  nur regulaere eigene Dateien, ueber Ordnerdeskriptoren ohne Links, nur das feste Muster der Kennungen.
- Punkt 1: der Ort der Startdatei in `konfig.probelauf.json` liegt im Laufzeitordner des Kerns, `/run/user/<uid>/werkbank/{sitzung}/start.json`,
  unabhaengig vom `XDG_RUNTIME_DIR` des Laeufers.

Jeder Fall legt seine Dateien unter einem eigenen `mkdtemp` an und loescht nur dort. Nur Standardbibliothek.
Aufruf: python3 -I -B test_sitzungsdateien.py
"""
import os
import shutil
import sys
import tempfile
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, konfig, messen  # noqa: E402

HINWEIS = messen.ZIELLISTE_NICHT_ENTFERNT
ENDUNGEN = (".suche.json", ".modellweg.json")


def schreibe(pfad, text="{}"):
    with open(pfad, "w", encoding="utf-8") as f:
        f.write(text)


class KonfigStartDatei(unittest.TestCase):
    def test_marker_start_datei_liegt_im_laufzeitordner_des_kerns(self):
        arbeit = tempfile.mkdtemp(prefix="sitz-arbeit-")
        self.addCleanup(shutil.rmtree, arbeit, True)
        pfad = os.path.join(HIER, "konfig.probelauf.json")
        with mock.patch.dict(os.environ, {"XDG_RUNTIME_DIR": "/x/anderer-laufzeitordner"}):
            k = konfig.lade(pfad, HIER, arbeit, nutzer="test")
        self.assertEqual(k["marker_start_datei"], "/run/user/%d/werkbank/{sitzung}/start.json" % os.getuid())


class ZaehldateienNachDemLauf(unittest.TestCase):
    def setUp(self):
        self.basis = tempfile.mkdtemp(prefix="zaehl-test-")
        self.addCleanup(shutil.rmtree, self.basis, True)
        self.zo = os.path.join(self.basis, "z")
        os.mkdir(self.zo)
        os.chmod(self.zo, 0o700)

    def pfad(self, sid, endung):
        return os.path.join(self.zo, sid + endung)

    def lege(self, sid):
        for e in ENDUNGEN:
            schreibe(self.pfad(sid, e))

    def k(self, ziel="echt"):
        return {"ziel": ziel, "zaehlordner": self.zo}

    def test_eigene_sitzungen_beide_dateien_weg_fremde_bleiben(self):
        self.lege("s-1")  # Cloud-Sitzung dieses Laufs
        self.lege("s-3")  # lokale Sitzung dieses Laufs
        self.lege("s-2")  # fremde Sitzung
        meld = ablauf.zaehldateien_entfernen(self.k(), ["s-1", "s-3"])
        self.assertEqual(meld, [])
        for e in ENDUNGEN:
            self.assertFalse(os.path.lexists(self.pfad("s-1", e)), e)
            self.assertFalse(os.path.lexists(self.pfad("s-3", e)), e)
            self.assertTrue(os.path.isfile(self.pfad("s-2", e)), e)

    def test_fehlende_dateien_geben_keinen_hinweis(self):
        self.assertEqual(ablauf.zaehldateien_entfernen(self.k(), ["s-4"]), [])

    def test_link_bleibt_und_sein_ziel_unveraendert(self):
        ziel = os.path.join(self.basis, "fremd.txt")
        schreibe(ziel, "unveraendert")
        os.symlink(ziel, self.pfad("s-1", ".suche.json"))
        meld = ablauf.zaehldateien_entfernen(self.k(), ["s-1"])
        self.assertTrue(os.path.islink(self.pfad("s-1", ".suche.json")))
        with open(ziel, encoding="utf-8") as f:
            self.assertEqual(f.read(), "unveraendert")
        self.assertEqual(meld, [HINWEIS])

    def test_ordner_mit_dem_namen_bleibt(self):
        p = self.pfad("s-1", ".modellweg.json")
        os.mkdir(p)
        schreibe(os.path.join(p, "datei"))
        meld = ablauf.zaehldateien_entfernen(self.k(), ["s-1"])
        self.assertTrue(os.path.isdir(p))
        self.assertTrue(os.path.isfile(os.path.join(p, "datei")))
        self.assertEqual(meld, [HINWEIS])

    def test_kennung_nach_festem_muster_tut_nichts(self):
        ausserhalb = os.path.join(self.basis, "x.suche.json")
        schreibe(ausserhalb)
        zeilenumbruch = self.pfad("s-1\n", ".suche.json")  # `$` passte frueher davor; das Muster ist jetzt fest
        schreibe(zeilenumbruch)
        for sid in ("../x", "s-1234567890", "S-1", "s-", "s-x", "s-1 ", "s-1\n"):
            with self.subTest(sid=sid):
                meld = ablauf.zaehldateien_entfernen(self.k(), [sid])
                self.assertEqual(meld, [HINWEIS])
        self.assertTrue(os.path.isfile(ausserhalb))
        self.assertTrue(os.path.isfile(zeilenumbruch))

    def test_attrappe_entfernt_nichts(self):
        self.lege("s-1")
        self.assertEqual(ablauf.zaehldateien_entfernen(self.k("attrappe"), ["s-1"]), [])
        for e in ENDUNGEN:
            self.assertTrue(os.path.isfile(self.pfad("s-1", e)), e)

    def test_fremde_uid_bleibt_mit_hinweis(self):
        self.lege("s-1")
        with mock.patch("os.getuid", return_value=os.getuid() + 1):
            meld = ablauf.zaehldateien_entfernen(self.k(), ["s-1"])
        for e in ENDUNGEN:
            self.assertTrue(os.path.isfile(self.pfad("s-1", e)), e)
        self.assertEqual(meld, [HINWEIS])

    def test_elternordner_als_symlink_bleibt(self):
        self.lege("s-1")
        link = os.path.join(self.basis, "link-auf-z")
        os.symlink(self.zo, link)
        meld = ablauf.zaehldateien_entfernen({"ziel": "echt", "zaehlordner": link}, ["s-1"])
        for e in ENDUNGEN:
            self.assertTrue(os.path.isfile(self.pfad("s-1", e)), e)
        self.assertEqual(meld, [HINWEIS])

    def test_ohne_zaehlordner_tut_nichts(self):
        self.assertEqual(ablauf.zaehldateien_entfernen({"ziel": "echt"}, ["s-1"]), [])

    def test_endung_ausserhalb_der_liste_tut_nichts(self):
        self.lege("s-1")
        entfernt, hinweis = messen.sitzungsdatei_entfernen(self.pfad("s-1", ".suche.json"), "s-1", ".konfig.json")
        self.assertFalse(entfernt)
        self.assertEqual(hinweis, HINWEIS)
        self.assertTrue(os.path.isfile(self.pfad("s-1", ".suche.json")))

    def test_zielliste_wrapper_bleibt_gleich(self):
        """`zielliste_entfernen` ist der Sonderfall der allgemeinen Funktion mit der Endung der Zielliste."""
        p = os.path.join(self.zo, "s-1.zielliste.json")
        schreibe(p)
        self.assertEqual(messen.zielliste_entfernen(p, "s-1"), (True, None))
        self.assertFalse(os.path.lexists(p))


if __name__ == "__main__":
    unittest.main()
