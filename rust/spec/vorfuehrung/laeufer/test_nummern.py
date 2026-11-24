"""Rahmennummern nach Weg A (P25; Projektentscheid, 23.11.2026, endgueltig): Feld `nr` je Eintrag der Sitzungsliste, Neuvergabe nur bei Bedarf.

Regel: Eine freie Nummer (Sitzung beendet, kein Abo) wird erst bei Bedarf neu vergeben: zuerst nie vergebene Nummern, dann die freie Nummer der am
laengsten beendeten Sitzung. Erst dann zeigt der alte Eintrag `nr: null`. Keine Uhr, keine Frist. Eine abonnierte Nummer wird nie vergeben.

Laeufer (`drehbuchlib/bruecke.py`, gegen einen Prueftisch-Server aus test_strom.py):
- Die Nummer kommt aus `nr`, nicht aus der Kennung; eine Kennung ueber 255 geht.
- Wiederverwendung: Sitzung A endet, B bekommt dieselbe Nummer. Der Verlauf von B traegt nichts von A, der von A bleibt; das Bild der Nummer ist
  vergessen (eine Aenderung ohne vollen Abzug ist danach ein Fehler des Stroms).
- `nr: null`: `raster` liefert `rahmen` 0 und leere `zeilen`, ohne Ausnahme und ohne Abo.
Gegenprobe: mit `drehbuchlib/bruecke.py` aus an earlier commit sind diese Tests rot.

Attrappe (`attrappen/bruecke_attrappe.py`, ohne Netz, `kanaele` klein):
- (a) Ein frueh beendeter Start (vor jedem Abo) bleibt ansehbar, bis seine Nummer neu vergeben ist; danach `nr: null` und das Abo wird abgelehnt.
- (b) Die Reihenfolge der Neuvergabe ist die des Endes.
- (c) Eine abonnierte Nummer wird nie vergeben.
Gegenprobe: mit sofortiger Freigabe beim Ende ist (a) rot.

E6 (`schritte._versuch`): Code und `grund` kommen aus der Sitzungsliste (`sitzung()`), nicht aus dem Verlauf (Projektentscheid, 23.11.2026). Der Verlauf ist
hier leer. Ein Grund in anderer Form wird `abweichend`. Gegenprobe: mit `drehbuchlib/schritte.py` aus an earlier commit rot.

Nur Standardbibliothek. Aufruf: python3 -I -B test_nummern.py
"""
import json
import os
import sys
import threading
import time
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad
sys.path.insert(0, os.path.join(HIER, "attrappen"))

import bruecke_attrappe as ba  # noqa: E402
from drehbuchlib import bruecke as br  # noqa: E402
from drehbuchlib import schritte  # noqa: E402
from test_strom import TOKEN, Tisch, rahmen, warte  # noqa: E402


def eintrag(sid, zustand, nr, **mehr):
    e = {"id": sid, "harness": "pi", "modell": "m", "route": "lokal", "kunde": "", "zustand": zustand, "nr": nr}
    e.update(mehr)
    return e


def liste(tisch, *eintraege):
    tisch.senden(6, json.dumps({"art": "Sitzungsliste", "sitzungen": list(eintraege)}).encode())


# --- Laeufer ---------------------------------------------------------------------------------------------------------------
class LaeuferNummern(unittest.TestCase):
    def client(self, auf_abo):
        tisch = Tisch(False, auf_abo)
        c = br.Bruecke({"url": "http://127.0.0.1:%d" % tisch.port, "token": TOKEN, "frist_s": 5, "abo": {"spalten": 20, "zeilen": 6}})
        self.addCleanup(tisch.zu)
        self.addCleanup(c.zumachen)
        return tisch, c

    def test_wiederverwendung_vergisst_bild_und_verlauf_der_alten_sitzung(self):
        def auf_abo(tisch, e):
            if e.get("art") != "Abo":
                return
            if e["sitzung"] == "s-1":
                tisch.raster(rahmen(1, 1, groesse=(20, 6), zeilen={0: "ZEILE-VON-A"}))
            elif e["sitzung"] == "s-2":
                tisch.raster(rahmen(1, 1, groesse=(20, 6), zeilen={0: "ZEILE-VON-B"}))  # erster Rahmen: ein voller Abzug
        tisch, c = self.client(auf_abo)
        c._offen()
        liste(tisch, eintrag("s-1", "laeuft", 1))
        self.assertTrue(warte(lambda: c.raster("s-1")["rahmen"] >= 1))
        self.assertIn("ZEILE-VON-A", c.verlauf("s-1")[1])
        # A endet ohne Betrachter, B bekommt Nummer 1
        liste(tisch, eintrag("s-1", "beendet", None, code=0), eintrag("s-2", "laeuft", 1))
        self.assertTrue(warte(lambda: c.sitzung("s-2").get("zustand") == "laeuft"))
        self.assertTrue(warte(lambda: c.raster("s-2")["rahmen"] >= 1))
        r = c.raster("s-2")
        self.assertEqual(r["zeilen"][0], "ZEILE-VON-B")
        self.assertEqual(r["rahmen"], 1, "der Zaehler der Nummer beginnt fuer B neu")
        vb = c.verlauf("s-2")[1]
        self.assertIn("ZEILE-VON-B", vb)
        self.assertNotIn("ZEILE-VON-A", vb, "der Verlauf von B traegt nichts von A")
        self.assertNotIn("ZEILE-VON-B", c.verlauf("s-1")[1], "der Verlauf von A traegt nichts von B")
        self.assertIn("ZEILE-VON-A", c.verlauf("s-1")[1], "der Verlauf von A bleibt")
        self.assertEqual(c.raster("s-1"), {"rahmen": 0, "zeilen": []}, "A hat kein Bild mehr")

    def test_nach_wiederverwendung_braucht_die_nummer_einen_vollen_abzug(self):
        def auf_abo(tisch, e):
            if e.get("art") == "Abo" and e["sitzung"] == "s-1":
                tisch.raster(rahmen(1, 1, groesse=(20, 6), zeilen={0: "a"}))
            elif e.get("art") == "Abo" and e["sitzung"] == "s-2":
                tisch.raster(rahmen(1, 2, zeilen={0: "b"}))  # nur eine Aenderung: ohne vollen Abzug fehlt das Bild der neuen Sitzung
        tisch, c = self.client(auf_abo)
        c._offen()
        liste(tisch, eintrag("s-1", "laeuft", 1))
        self.assertTrue(warte(lambda: c.raster("s-1")["rahmen"] >= 1))
        liste(tisch, eintrag("s-1", "beendet", None, code=0), eintrag("s-2", "laeuft", 1))
        self.assertTrue(warte(lambda: c.sitzung("s-2").get("zustand") == "laeuft"))
        try:
            c.raster("s-2")
        except br.StromFehler:
            pass
        self.assertTrue(warte(lambda: c._strom.fehler is not None), "die Aenderung ohne Abzug wurde auf das alte Bild angewandt")
        self.assertEqual(c._strom.fehler.code, "rahmen.kein_abzug")

    def test_nr_null_raster_leer_ohne_abo(self):
        tisch, c = self.client(lambda t, e: None)
        c._offen()
        liste(tisch, eintrag("s-1", "beendet", None, code=65, grund="lauf.ordner_fehlt"))
        self.assertTrue(warte(lambda: c.sitzung("s-1").get("zustand") == "beendet"))
        self.assertEqual(c.raster("s-1"), {"rahmen": 0, "zeilen": []})
        self.assertEqual(c.verlauf("s-1"), (0, ""))
        time.sleep(0.2)
        self.assertEqual([p["koerper"]["art"] for p in tisch.posts], [], "kein Abo auf eine Sitzung ohne Nummer")

    def test_ohne_feld_nr_gilt_null(self):
        """Fehlt der Schluessel `nr` (Einzelbetrieb ohne `lauf`), gilt 0; `nr: null` und eine falsche Form ergeben keine Nummer."""
        self.assertEqual(br.nr_aus_eintrag({"id": "s-1", "zustand": "laeuft"}), 0)
        self.assertIsNone(br.nr_aus_eintrag({"id": "s-1", "zustand": "beendet", "nr": None}))
        self.assertIsNone(br.nr_aus_eintrag({"id": "s-1", "nr": 256}))
        self.assertIsNone(br.nr_aus_eintrag({"id": "s-1", "nr": True}))
        self.assertEqual(br.nr_aus_eintrag({"id": "s-1", "nr": 7}), 7)

    def test_nummer_aus_dem_feld_nicht_aus_der_kennung(self):
        def auf_abo(tisch, e):
            if e.get("art") == "Abo":
                tisch.raster(rahmen(7, 1, groesse=(20, 6), zeilen={0: "SIEBEN"}))
        tisch, c = self.client(auf_abo)
        c._offen()
        liste(tisch, eintrag("s-300", "laeuft", 7))
        self.assertTrue(warte(lambda: c.raster("s-300")["rahmen"] >= 1), "Kennung ueber 255 mit Nummer 7")
        self.assertEqual(c.raster("s-300")["zeilen"][0], "SIEBEN")


# --- Attrappe --------------------------------------------------------------------------------------------------------------
def kern(kanaele):
    k = ba.Kern.__new__(ba.Kern)
    k.lock = threading.Lock()
    k.verb_lock = threading.Lock()
    k.sitzungen, k.nummern, k.verbindungen = {}, {}, []
    k.kanaele, k.ende_zaehler, k.n = kanaele, 0, 0
    return k


def neu(k, beenden=False):
    """Eine Sitzung wie `Kern.starten` (Nummer beim Start); mit `beenden` endet sie gleich, vor jedem Abo (wie `_abgewiesen`)."""
    with k.lock:
        k.n += 1
        sid = "s-%d" % k.n
        nr = k._nummer_nehmen(sid)
    s = ba.Sitzung(sid, {"harness": "pi", "modell": "m", "route": "lokal", "kunde": None}, None, {}, time.time(), (20, 6), lambda s: None)
    s.nr = nr
    s.schreibe("BILD-%s\n" % sid)
    if beenden:
        s.beendet, s.code = True, 65
    with k.lock:
        k.sitzungen[sid] = s
    if beenden:
        k._nach_ende(s)
    return s


def ende(k, s):
    s.beendet, s.code = True, 129
    k._nach_ende(s)


def verbindung(k):
    v = ba.Verbindung(False)
    k.verbindungen.append(v)
    return v


def abo(k, v, s):
    """Abo ueber `Kern.ereignis`. Gibt (Ablehnungen, Rasterrahmen als Bytes)."""
    k.ereignis(v, {"art": "Abo", "sitzung": s.sid, "an": True, "spalten": 20, "zeilen": 6})
    gruende, raster = [], []
    while not v.q.empty():
        d = v.q.get()
        art, nutz = d[4], d[5:]
        if art == 7:
            gruende.append(json.loads(nutz)["grund"])
        elif art == 5:
            raster.append(nutz)
    return gruende, raster


class AttrappeNummern(unittest.TestCase):
    def test_a_frueh_beendet_bleibt_ansehbar_bis_zur_neuvergabe(self):
        k = kern(2)
        a = neu(k, beenden=True)      # endet vor jedem Abo
        self.assertEqual(a.nr, 1)
        b = neu(k)                    # nie vergebene Nummer 2 zuerst: A behaelt 1
        self.assertEqual((a.nr, b.nr), (1, 2))
        v = verbindung(k)
        gruende, raster = abo(k, v, a)
        self.assertEqual(gruende, [])
        self.assertEqual(len(raster), 1)
        self.assertEqual((raster[0][0], raster[0][1], raster[0][2] & 4), (5, 1, 4), "voller Abzug auf Nummer 1")
        self.assertIn(b"BILD-s-1", raster[0])
        self.assertEqual({e["id"]: e["nr"] for e in k.liste_nachricht()["sitzungen"]}, {"s-1": 1, "s-2": 2})

    def test_a_nach_neuvergabe_nr_null_und_abo_abgelehnt(self):
        k = kern(1)
        a = neu(k, beenden=True)
        c = neu(k)                    # braucht eine Nummer: A gibt 1 ab
        self.assertEqual((a.nr, c.nr), (None, 1))
        self.assertEqual({e["id"]: e["nr"] for e in k.liste_nachricht()["sitzungen"]}, {"s-1": None, "s-2": 1})
        gruende, raster = abo(k, verbindung(k), a)
        self.assertEqual((gruende, raster), ([ba.ABO_OHNE_NUMMER], []))

    def test_b_neuvergabe_in_der_reihenfolge_des_endes(self):
        k = kern(2)
        a, b = neu(k), neu(k)
        ende(k, b)                    # B endet zuerst
        ende(k, a)
        c = neu(k)
        self.assertEqual((c.nr, b.nr, a.nr), (2, None, 1), "die Nummer der am laengsten beendeten Sitzung (B)")
        d = neu(k)
        self.assertEqual((d.nr, a.nr), (1, None))

    def test_c_abonnierte_nummer_wird_nie_vergeben(self):
        k = kern(2)
        a, b = neu(k), neu(k)
        v = verbindung(k)
        abo(k, v, a)
        ende(k, a)                    # A endet zuerst, ist aber abonniert
        ende(k, b)
        c = neu(k)
        self.assertEqual((c.nr, a.nr, b.nr), (2, 1, None))
        d = neu(k)                    # nur A ist beendet, aber abonniert: keine Nummer frei
        self.assertEqual((d.nr, a.nr), (None, 1))
        k.ereignis(v, {"art": "Abo", "sitzung": a.sid, "an": False})
        e = neu(k)                    # nach dem Ende des Abos ist die Nummer von A frei
        self.assertEqual((e.nr, a.nr), (1, None))

    def test_laufende_sitzung_behaelt_ihre_nummer(self):
        k = kern(1)
        a = neu(k)
        b = neu(k)
        self.assertEqual((a.nr, b.nr), (1, None), "eine laufende Sitzung gibt ihre Nummer nie ab")

    def test_zuerst_die_zahl_der_kennung(self):
        """Wie der Kern (23.11.2026): Eine nie vergebene Zahl der Kennung geht vor der kleinsten nie vergebenen Nummer."""
        k = kern(8)
        with k.lock:
            self.assertEqual(k._nummer_nehmen("s-5"), 5)
            self.assertEqual(k._nummer_nehmen("s-9"), 1, "9 liegt ueber kanaele: die kleinste nie vergebene")
            self.assertEqual(k._nummer_nehmen("s-2"), 2)
            self.assertEqual(k._nummer_nehmen("s-12"), 3)

    def test_vorgabe_der_kanaele(self):
        self.assertEqual(ba.NR_MAX, 255)


# --- E6 --------------------------------------------------------------------------------------------------------------------
class ListenBruecke:
    """Nur was `_versuch` braucht: Start, Sitzungsliste (beendet, Code, Grund) und ein leerer Verlauf (die Nummer ist schon neu vergeben)."""

    def __init__(self, grund):
        self.grund = grund

    def diagnose(self):
        return None

    def starten(self, start):
        return 200, {"sitzung": "s-9"}, 0.0

    def sitzung(self, sid, frist_s=2.0):
        e = {"sitzung": sid, "zustand": "beendet", "code": 65, "start": {}}
        if self.grund is not None:
            e["grund"] = self.grund
        return e

    def raster(self, sid):
        return {"rahmen": 0, "zeilen": []}

    def verlauf(self, sid, ab=0):
        return 0, ""


class Lf:
    def __init__(self, b):
        self.b, self.sitzungen, self.art_von, self.starts, self.max_starts = b, [], {}, 0, None
        self.k = {"harness": {"copilot": {"art": "cloud"}}}


class E6AusDerListe(unittest.TestCase):
    def versuch(self, grund):
        return schritte._versuch(Lf(ListenBruecke(grund)), {"harness": "copilot"})

    def test_code_aus_grund_der_liste(self):
        v = self.versuch("lauf.ordner_anderer_kunde")
        self.assertEqual((v["abgewiesen"], v["code"], v["laeuft"]), (True, "lauf.ordner_anderer_kunde", False))

    def test_grund_ohne_form_wird_abweichend(self):
        self.assertEqual(self.versuch("Ordner /pfad/geheim gehoert wem anders")["code"], "abweichend")

    def test_ohne_grund_kein_code(self):
        v = self.versuch(None)
        self.assertEqual((v["abgewiesen"], v["code"]), (True, None))


if __name__ == "__main__":
    unittest.main()
