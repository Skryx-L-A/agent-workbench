"""Prueft `drehbuchlib/messen.py`: `neuer_tempordner` und `sicher_loeschen` (Auflage vom 21.11.2026).

Der Laeufer loescht einen Ordner nur, wenn der aufgeloeste Pfad gleich einer selbst angelegten Wurzel ist oder darunter liegt.
Jeder Fall legt seine Ordner unter einem eigenen `mkdtemp` des Tests an; der Test loescht nichts ausserhalb davon.
Nur Standardbibliothek.
Aufruf: python3 -I -B test_loeschen.py
"""
import contextlib
import io
import os
import shutil
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import messen  # noqa: E402

FESTER_SATZ = "Loeschen abgelehnt: Der Pfad liegt nicht in einem eigenen Arbeitsordner. Nichts geloescht.\n"


def schreibe(pfad, text="x"):
    with open(pfad, "w", encoding="utf-8") as f:
        f.write(text)


class SicherLoeschen(unittest.TestCase):
    def setUp(self):
        self.basis = tempfile.mkdtemp(prefix="loeschen-test-")
        self.addCleanup(shutil.rmtree, self.basis, True)

    def loesche(self, pfad):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            geloescht = messen.sicher_loeschen(pfad)
        return geloescht, err.getvalue()

    def fremder_ordner(self):
        fremd = tempfile.mkdtemp(prefix="fremd-", dir=self.basis)
        schreibe(os.path.join(fremd, "datei"))
        return fremd

    def test_gemerkte_wurzel_wird_geloescht(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        schreibe(os.path.join(wurzel, "datei"))
        geloescht, err = self.loesche(wurzel)
        self.assertTrue(geloescht)
        self.assertFalse(os.path.exists(wurzel))
        self.assertEqual(err, "")

    def test_unterordner_einer_wurzel_wird_geloescht(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        unter = os.path.join(wurzel, "a", "b")
        os.makedirs(unter)
        schreibe(os.path.join(unter, "datei"))
        geloescht, err = self.loesche(os.path.join(wurzel, "a"))
        self.assertTrue(geloescht)
        self.assertFalse(os.path.exists(os.path.join(wurzel, "a")))
        self.assertTrue(os.path.isdir(wurzel))
        self.assertEqual(err, "")

    def test_fremder_ordner_bleibt(self):
        fremd = self.fremder_ordner()
        geloescht, err = self.loesche(fremd)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertNotIn(fremd, err)
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))

    def test_pfad_mit_dotdot_aus_der_wurzel_bleibt(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        fremd = self.fremder_ordner()
        pfad = os.path.join(wurzel, "..", os.path.basename(fremd))
        geloescht, err = self.loesche(pfad)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))
        self.assertTrue(os.path.isdir(wurzel))

    def test_symlink_in_wurzel_auf_fremden_ordner_bleibt(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        fremd = self.fremder_ordner()
        link = os.path.join(wurzel, "link")
        os.symlink(fremd, link)

        geloescht, err = self.loesche(link)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.islink(link))
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))

        # Ein Pfad durch den Symlink hinaus wird ebenfalls abgewiesen.
        geloescht, err = self.loesche(os.path.join(link, "datei"))
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))

        # Das Loeschen der Wurzel folgt dem Symlink nicht: der fremde Ordner bleibt mit Inhalt.
        geloescht, err = self.loesche(wurzel)
        self.assertTrue(geloescht)
        self.assertFalse(os.path.lexists(wurzel))
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))

    def test_wurzel_selbst_ist_symlink_bleibt(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        schreibe(os.path.join(wurzel, "datei"))
        fremd = self.fremder_ordner()

        # Fall 1: die Wurzel wird durch einen Symlink auf einen fremden Ordner ersetzt.
        os.rename(wurzel, os.path.join(self.basis, "wurzel-beiseite"))
        os.symlink(fremd, wurzel)
        geloescht, err = self.loesche(wurzel)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.islink(wurzel))
        self.assertTrue(os.path.isfile(os.path.join(fremd, "datei")))

        # Fall 2: ein Symlink auf eine gemerkte Wurzel bleibt ebenfalls; die Wurzel bleibt mit Inhalt.
        echt = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        schreibe(os.path.join(echt, "datei"))
        link = os.path.join(self.basis, "link-auf-wurzel")
        os.symlink(echt, link)
        geloescht, err = self.loesche(link)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.islink(link))
        self.assertTrue(os.path.isfile(os.path.join(echt, "datei")))

    def test_praefix_ist_kein_ganzer_pfadteil(self):
        wurzel = messen.neuer_tempordner(prefix="wbtest-", dir=self.basis)
        nachbar = wurzel + "x"  # gleicher Textanfang, aber kein Ordner darunter
        os.mkdir(nachbar)
        schreibe(os.path.join(nachbar, "datei"))
        geloescht, err = self.loesche(nachbar)
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)
        self.assertTrue(os.path.isfile(os.path.join(nachbar, "datei")))

    def test_leerer_pfad_loescht_nichts(self):
        geloescht, err = self.loesche("")
        self.assertFalse(geloescht)
        self.assertEqual(err, FESTER_SATZ)


if __name__ == "__main__":
    unittest.main()
