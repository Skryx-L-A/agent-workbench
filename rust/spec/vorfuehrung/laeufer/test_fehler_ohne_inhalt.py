"""Fehler ohne Inhalt (22.11.): Ergebnisdatei, Bericht und Konsole tragen nur feste Codes, Zahlen und feste Saetze.
Text mit veraenderlichem Inhalt (Ende des Verlaufs einer Sitzung, Antwort der Bruecke, Text einer Ausnahme) steht allein in
`<ergebnis>.diagnose` (0600, O_EXCL, O_NOFOLLOW); die Konsole nennt nur die Zeile `diagnose: angelegt`.

Die Testwerte werden zur Laufzeit gebaut, nie als Literal: ein Token (`gho_` plus 36 Zeichen) und ein Kontoname (`konto-` plus 8 Zeichen).
Fall a: ein Zug laeuft in die Frist, der Verlauf traegt beide Werte. Fall b: ein Start wird abgelehnt, die Antwort der Bruecke traegt das
Token. Fall c: ein Lauf ohne Ausnahme legt keine Diagnosedatei an. Fall d: dasselbe wie a im Weg des Pakets (`probelauf.probelauf`, die
Ergebnisdatei behaelt genau fuenf Schluessel je Schritt). Fall e: ein vorhandener Name oder ein Symlink wird nie beschrieben.
Benutzt keine neuen Namen des Laeufers, damit die Gegenprobe mit dem alten Laeufer (lauf, ablauf, schritte vor dieser Korrektur) aussagekraeftig ist.
Nur Standardbibliothek. Aufruf: python3 -I -B test_fehler_ohne_inhalt.py   (startet die Attrappen: einige Sekunden je Lauf)
"""
import contextlib
import io
import json
import os
import secrets
import shutil
import stat
import string
import sys
import tempfile
import time
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

import drehbuch  # noqa: E402
from drehbuchlib import bruecke, probelauf, schritte  # noqa: E402

ALNUM = string.ascii_letters + string.digits


def zufall(n, alphabet=ALNUM):
    return "".join(secrets.choice(alphabet) for _ in range(n))


class Werte:
    def __init__(self):
        self.token = "gho_" + zufall(36)
        self.konto = "konto-" + zufall(8, string.ascii_lowercase + string.digits)


@contextlib.contextmanager
def zug_in_die_frist(werte):
    """Im Schritt S2 verliert jeder Verlauf das Wort FERTIG und traegt am Ende Token und Kontoname; die Frist des Zugs ist 1 s."""
    scharf = {"an": False}
    echt = bruecke.Bruecke.verlauf
    echt_raster = bruecke.Bruecke.raster

    def raster(self, sid):
        r = echt_raster(self, sid)
        if scharf["an"] and isinstance(r, dict):
            r = dict(r, zeilen=list(r.get("zeilen", [])) + ["zeile-mit-token %s" % werte.token, "zeile-mit-konto %s" % werte.konto])
        return r

    def verlauf(self, sid, ab=0):
        n, t = echt(self, sid, ab)
        if scharf["an"]:
            t = t.replace("FERTIG", "FERT1G") + " %s %s" % (werte.token, werte.konto)
        return n, t
    s2 = dict(schritte.REIHENFOLGE)["S2"]

    def s2_scharf(L):
        L.turn_s = 1.0
        scharf["an"] = True
        try:
            return s2(L)
        finally:
            scharf["an"] = False
    folge = [(n, s2_scharf if n == "S2" else f) for n, f in schritte.REIHENFOLGE]
    with mock.patch.object(bruecke.Bruecke, "verlauf", autospec=True, side_effect=verlauf), \
            mock.patch.object(bruecke.Bruecke, "raster", autospec=True, side_effect=raster), \
            mock.patch.object(schritte, "REIHENFOLGE", folge):
        yield


@contextlib.contextmanager
def start_abgelehnt(werte):
    """Jeder Start wird mit 422 abgelehnt; die Antwort der Bruecke traegt das Token und den Kontonamen."""
    def ablehnen(self, start, ohne_token=False):
        return 422, {"code": "start.schluessel_fehlt", "grund": "start_pruefung", "text": "%s %s" % (werte.token, werte.konto)}, time.monotonic()
    with mock.patch.object(bruecke.Bruecke, "starten", autospec=True, side_effect=ablehnen):
        yield


class Werkstatt(unittest.TestCase):
    def setUp(self):
        self.ordner = tempfile.mkdtemp(prefix="feo-")
        self.addCleanup(shutil.rmtree, self.ordner, True)
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        self.konfig = os.path.join(self.ordner, "k.json")
        with open(self.konfig, "w", encoding="utf-8") as f:
            json.dump(k, f)
        self.w = Werte()

    def bericht(self, wirkung=None, vorher=None):
        """`drehbuch.py --ziel attrappe --nur S2 --json`: gibt (Konsole, Pfad des Berichts, Bericht als Dict oder None)."""
        pfad = os.path.join(self.ordner, "bericht.json")
        if vorher:
            vorher(pfad + ".diagnose")
        aus = io.StringIO()
        with contextlib.ExitStack() as st:
            if wirkung:
                st.enter_context(wirkung(self.w))
            st.enter_context(contextlib.redirect_stdout(aus))
            st.enter_context(contextlib.redirect_stderr(aus))
            drehbuch.main(["--ziel", "attrappe", "--konfig", self.konfig, "--nur", "S2", "--json", pfad])
        with open(pfad, encoding="utf-8") as f:
            roh = f.read()
        return aus.getvalue(), pfad, roh

    def paket(self, wirkung=None):
        """Der Weg des Pakets (`drehbuch.py probelauf`, Aufruf von `werkbank-probelauf`): gibt (Konsole, Pfad der Ergebnisdatei, Datei)."""
        pfad = os.path.join(self.ordner, "ergebnis.json")
        aus = io.StringIO()
        with contextlib.ExitStack() as st:
            if wirkung:
                st.enter_context(wirkung(self.w))
            st.enter_context(contextlib.redirect_stdout(aus))
            st.enter_context(contextlib.redirect_stderr(aus))
            probelauf.probelauf(["--ergebnis", pfad, "--konfig", self.konfig, "--nur", "S2"], HIER)
        with open(pfad, encoding="utf-8") as f:
            roh = f.read()
        return aus.getvalue(), pfad, roh

    def keine_treffer(self, *texte):
        for t in texte:
            self.assertEqual(t.count(self.w.token), 0)
            self.assertEqual(t.count(self.w.konto), 0)

    def diagnose_ok(self, pfad, konsole, mit_konto=True, raster=False):
        d = pfad + ".diagnose"
        self.assertTrue(os.path.isfile(d), "Diagnosedatei fehlt")
        self.assertEqual(stat.S_IMODE(os.stat(d).st_mode), 0o600)
        with open(d, encoding="utf-8") as f:
            inhalt = f.read()
        self.assertIn(self.w.token, inhalt)
        if raster:  # Zug in die Frist: das ganze Raster und das Ende des Verlaufs stehen in der Datei
            self.assertIn("zeile-mit-token %s" % self.w.token, inhalt)
            self.assertIn("zeile-mit-konto %s" % self.w.konto, inhalt)
            self.assertIn("lauf.verlauf: letzte 4000 Zeichen", inhalt)
        if mit_konto:
            self.assertIn(self.w.konto, inhalt)
        self.assertIn("diagnose: angelegt", konsole.splitlines())
        return inhalt


class ZugInDieFrist(Werkstatt):
    def test_a_bericht(self):
        konsole, pfad, roh = self.bericht(zug_in_die_frist)
        self.keine_treffer(konsole, roh)
        s2 = {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S2"]
        self.assertEqual(s2["fehler"], "Zeitueberschreitung: s2.zug_frist")
        self.diagnose_ok(pfad, konsole, raster=True)

    def test_d_paket(self):
        konsole, pfad, roh = self.paket(zug_in_die_frist)
        self.keine_treffer(konsole, roh)
        d = json.loads(roh)
        s2 = {s["schritt"]: s for s in d["schritte"]}["S2"]
        self.assertEqual(s2["fehlercode"], "s2.zug_frist")
        for s in d["schritte"]:
            self.assertEqual(set(s), {"schritt", "bestanden", "fehlercode", "dauer_ms", "offen"})
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.diagnose_ok(pfad, konsole, raster=True)


class StartAbgelehnt(Werkstatt):
    def test_b_bericht(self):
        konsole, pfad, roh = self.bericht(start_abgelehnt)
        self.keine_treffer(konsole, roh)
        s1 = {s["schritt"]: s for s in json.loads(roh)["schritte"]}["S1"]
        self.assertTrue(s1["fehler"])
        self.diagnose_ok(pfad, konsole)

    def test_b_paket(self):
        konsole, pfad, roh = self.paket(start_abgelehnt)
        self.keine_treffer(konsole, roh)
        self.diagnose_ok(pfad, konsole)


class OhneAusnahme(Werkstatt):
    def test_c_keine_diagnosedatei(self):
        konsole, pfad, roh = self.bericht()
        self.assertFalse(os.path.lexists(pfad + ".diagnose"))
        self.assertNotIn("diagnose:", konsole)
        konsole2, pfad2, _ = self.paket()
        self.assertFalse(os.path.lexists(pfad2 + ".diagnose"))
        self.assertNotIn("diagnose:", konsole2)


class NieUeberschreiben(Werkstatt):
    def test_e_symlink_und_vorhandene_datei(self):
        opfer = os.path.join(self.ordner, "opfer.txt")
        with open(opfer, "w", encoding="utf-8") as f:
            f.write("unberuehrt")

        def link(ziel):
            os.symlink(opfer, ziel)
        konsole, pfad, roh = self.bericht(zug_in_die_frist, vorher=link)
        self.keine_treffer(konsole, roh)
        with open(opfer, encoding="utf-8") as f:
            self.assertEqual(f.read(), "unberuehrt")
        self.assertIn("diagnose: nicht angelegt", konsole.splitlines())


if __name__ == "__main__":
    unittest.main()
