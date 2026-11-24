"""Die Bruecken-Attrappe nimmt die Art `Groesse` der Ansicht an (Vertrag dienst DI3, erlaubte Arten der Ansicht).

Die neue Seite schickt bei jeder neuen Groesse `{"art": "Groesse", "sitzung", "spalten", "zeilen"}` ueber `POST /eingabe` (gemessen am Zeitprotokoll
der Attrappe). Die Attrappe lehnte sie mit `art_nicht_erlaubt` ab, und die Seite zeigte diese Ablehnung als Fehler des Starts.
Pruefung ohne Netz: `Kern.ereignis` mit einer Verbindung, die ihre Ablehnungen sammelt. `spalten` unter 2 gibt `groesse_ungueltig` (DI3, Nachtrag
20.11.); ohne Abo `kein_abo`. Gegenprobe: mit der Attrappe vor dieser Korrektur gibt der erste Fall `art_nicht_erlaubt`. Nur Standardbibliothek.
Aufruf: python3 -I -B test_attrappe_groesse.py
"""
import os
import sys
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HIER, "attrappen"))
import bruecke_attrappe as ba  # noqa: E402


class Verb:
    def __init__(self, abo):
        self.abos = {"s-1": {}} if abo else {}
        self.grund = []

    def ablehnung(self, g):
        self.grund.append(g)


def sende(abo, **e):
    k = ba.Kern.__new__(ba.Kern)
    k.sitzungen = {"s-1": object()}
    v = Verb(abo)
    k.ereignis(v, dict({"art": "Groesse", "sitzung": "s-1", "spalten": 82, "zeilen": 25}, **e))
    return v.grund


class Groesse(unittest.TestCase):
    def test_gueltig_mit_abo_keine_ablehnung(self):
        self.assertEqual(sende(True), [])

    def test_eine_spalte_ist_ungueltig(self):
        self.assertEqual(sende(True, spalten=1), ["groesse_ungueltig"])

    def test_zeilen_ausser_bereich_ist_ungueltig(self):
        self.assertEqual(sende(True, zeilen=0), ["groesse_ungueltig"])

    def test_ohne_abo_kein_abo(self):
        self.assertEqual(sende(False), ["kein_abo"])

    def test_andere_art_bleibt_nicht_erlaubt(self):
        self.assertEqual(sende(True, art="Takt"), ["art_nicht_erlaubt"])


if __name__ == "__main__":
    unittest.main()
