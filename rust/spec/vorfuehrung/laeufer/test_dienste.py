"""Tests fuer Start und Stopp der Dienste im Probelauf (`probelauf.dienste_bereit`, `dienste_beenden`, Nachtrag 21.11., 16).

Nur Standardbibliothek. Aufruf: python3 -I -B test_dienste.py

`systemctl` ist ein Stub im PATH: ein Skript in einem Wegwerf-Ordner. Es schreibt jeden Aufruf nach `aufrufe.txt`, notiert die Namen
seiner Umgebung und ob Stdin /dev/null ist (`umgebung.txt`), und liest sein Verhalten aus `verhalten.json`. Den Zustand (welche
Einheiten laufen) haelt es in `zustand.json`. Den Pfad der Datei kann der Stub nicht aus der Umgebung holen, weil der Laeufer nur
vier Variablen weitergibt; daher steht er im Skript.
"""

import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, probelauf  # noqa: E402

KERN, BRUECKE = "werkbank-kern", "werkbank-bruecke"
ERLAUBT = {"PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"}
# Der Interpreter des Stubs setzt diese beiden selbst (macOS und PEP 538); der Laeufer gibt sie nicht weiter.
VOM_INTERPRETER = {"LC_CTYPE", "__CF_USER_TEXT_ENCODING"}
MARKER = "WERKBANK_TESTWERT_GEHEIM"  # steht in der Testumgebung; der Stub darf ihn nie sehen

STUB = """#!{python}
import json
import os
import sys

HIER = {hier!r}
args = sys.argv[1:]
with open(os.path.join(HIER, "aufrufe.txt"), "a", encoding="utf-8") as f:
    f.write(" ".join(args) + "\\n")
with open(os.path.join(HIER, "umgebung.txt"), "a", encoding="utf-8") as f:
    null = os.path.samestat(os.fstat(0), os.stat("/dev/null"))
    f.write(",".join(sorted(os.environ)) + " stdin_null=" + str(null) + "\\n")
with open(os.path.join(HIER, "verhalten.json"), encoding="utf-8") as f:
    v = json.load(f)
with open(os.path.join(HIER, "zustand.json"), encoding="utf-8") as f:
    laeuft = json.load(f)
if len(args) != 3 or args[0] != "--user":
    sys.exit(2)
verb, einheit = args[1], args[2]
rc = 2
if verb == "is-active":
    rc = 0 if einheit in laeuft else 3
elif verb in ("start", "restart"):
    if einheit in v["start_scheitert"]:
        rc = 1
    else:
        if einheit not in laeuft:
            laeuft.append(einheit)
        rc = 0
        if einheit == v["dateien_bei"]:
            for pfad, text in v["dateien"].items():
                os.makedirs(os.path.dirname(pfad), exist_ok=True)
                with open(pfad, "w", encoding="utf-8") as f:
                    f.write(text)
elif verb == "stop":
    if einheit in laeuft:
        laeuft.remove(einheit)
    rc = 0
with open(os.path.join(HIER, "zustand.json"), "w", encoding="utf-8") as f:
    json.dump(laeuft, f)
sys.exit(rc)
"""


class Stub:
    """Wegwerf-Ordner mit dem Stub, eigenem XDG_RUNTIME_DIR und den Dateien der Bruecke."""

    def __init__(self, laeuft=(), start_scheitert=(), dateien_bei=None, dateien=False):
        self.ordner = tempfile.mkdtemp(prefix="dienste-stub-")
        self.xdg = os.path.join(self.ordner, "xdg")
        os.makedirs(self.xdg)
        self.port = os.path.join(self.xdg, "werkbank", "bruecke.port")
        self.token = os.path.join(self.xdg, "werkbank", "bruecke.token")
        dat = {self.port: "127.0.0.1:4711\n", self.token: "tok-test\n"} if dateien else {}
        self._schreibe("verhalten.json", {"start_scheitert": list(start_scheitert), "dateien_bei": dateien_bei, "dateien": dat})
        if dateien and dateien_bei is None:  # die Bruecke lief schon: ihre Dateien liegen vor dem Lauf da
            for pfad, text in dat.items():
                os.makedirs(os.path.dirname(pfad), exist_ok=True)
                with open(pfad, "w", encoding="utf-8") as f:
                    f.write(text)
        self._schreibe("zustand.json", list(laeuft))
        self.stub = os.path.join(self.ordner, "systemctl")
        with open(self.stub, "w", encoding="utf-8") as f:
            f.write(STUB.format(python=sys.executable, hier=self.ordner))
        os.chmod(self.stub, 0o755)

    def _schreibe(self, name, obj):
        with open(os.path.join(self.ordner, name), "w", encoding="utf-8") as f:
            json.dump(obj, f)

    def aufrufe(self):
        p = os.path.join(self.ordner, "aufrufe.txt")
        if not os.path.exists(p):
            return []
        with open(p, encoding="utf-8") as f:
            return [z.strip() for z in f if z.strip()]

    def aufrufe_mit(self, verb):
        """Einheiten der Aufrufe mit diesem Verb, in der Reihenfolge der Aufrufe."""
        return [z.split()[-1] for z in self.aufrufe() if z.split()[1] == verb]

    def umgebung(self):
        p = os.path.join(self.ordner, "umgebung.txt")
        with open(p, encoding="utf-8") as f:
            return [z.strip() for z in f if z.strip()]

    def k(self, frist=0.5, dienste=(KERN, BRUECKE)):
        return {"ziel": "echt", "dienste": list(dienste),
                "bruecke": {"port_datei": self.port, "token_datei": self.token, "frist_s": frist}}

    def lauf(self, k):
        """Wie `lauf`: Start, dann Stopp in `finally`. Gibt (code, gestartet, meldungen)."""
        env = dict(os.environ, PATH=self.ordner + os.pathsep + os.environ.get("PATH", ""), XDG_RUNTIME_DIR=self.xdg)
        with mock.patch.dict(os.environ, env, clear=False):
            gestartet = []
            try:
                code, gestartet = probelauf.dienste_bereit(k)
            finally:
                meld = probelauf.dienste_beenden(gestartet)
        return code, gestartet, meld

    def loesche(self):
        shutil.rmtree(self.ordner, ignore_errors=True)


class Dienste(unittest.TestCase):
    def setUp(self):
        self.s = None

    def tearDown(self):
        if self.s is not None:
            self.s.loesche()

    def test_beide_laufen_nicht(self):
        self.s = Stub(dateien_bei=BRUECKE, dateien=True)
        code, gestartet, meld = self.s.lauf(self.s.k())
        self.assertIsNone(code)
        self.assertEqual(gestartet, [KERN, BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("start"), [KERN, BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE, KERN])
        self.assertEqual(meld, [])

    def test_beide_laufen_schon(self):
        self.s = Stub(laeuft=[KERN, BRUECKE], dateien=True)
        code, gestartet, meld = self.s.lauf(self.s.k())
        self.assertIsNone(code)
        self.assertEqual(gestartet, [])
        self.assertEqual(self.s.aufrufe_mit("start"), [])
        self.assertEqual(self.s.aufrufe_mit("stop"), [])
        self.assertEqual(meld, [])

    def test_nur_kern_laeuft(self):
        self.s = Stub(laeuft=[KERN], dateien_bei=BRUECKE, dateien=True)
        code, gestartet, meld = self.s.lauf(self.s.k())
        self.assertIsNone(code)
        self.assertEqual(gestartet, [BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("start"), [BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE])
        self.assertEqual(meld, [])

    def test_start_der_bruecke_scheitert(self):
        self.s = Stub(start_scheitert=[BRUECKE], dateien=True)
        code, gestartet, meld = self.s.lauf(self.s.k())
        self.assertEqual(code, "bruecke.dienst-startet-nicht")
        self.assertEqual(gestartet, [KERN])
        self.assertEqual(self.s.aufrufe_mit("start"), [KERN, BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("stop"), [KERN], "der Kern wird gestoppt, die nicht gestartete Bruecke nicht")
        self.assertEqual(meld, [])

    def test_dateien_kommen_nicht(self):
        self.s = Stub(dateien_bei=None)  # Start gelingt, keine Datei entsteht
        t0 = time.monotonic()
        code, gestartet, meld = self.s.lauf(self.s.k(frist=0.5))
        self.assertLess(time.monotonic() - t0, 19)
        self.assertEqual(code, "bruecke.dienst-startet-nicht")
        self.assertEqual(gestartet, [KERN, BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE, KERN])
        self.assertEqual(meld, [])

    def test_leere_liste_ruft_nichts(self):
        self.s = Stub()
        code, gestartet, meld = self.s.lauf(self.s.k(dienste=[]))
        self.assertIsNone(code)
        self.assertEqual(gestartet, [])
        self.assertEqual(self.s.aufrufe(), [], "kein Aufruf von systemctl")
        self.assertEqual(meld, [])

    def test_umgebung_und_stdin(self):
        self.s = Stub(dateien_bei=BRUECKE, dateien=True)
        with mock.patch.dict(os.environ, {MARKER: "geheim"}):
            self.s.lauf(self.s.k())
        zeilen = self.s.umgebung()
        self.assertTrue(zeilen, "der Stub lief")
        for z in zeilen:
            namen, _, rest = z.partition(" stdin_null=")
            self.assertTrue(set(namen.split(",")) <= ERLAUBT | VOM_INTERPRETER, "Umgebung: %s" % namen)
            self.assertNotIn(MARKER, namen)
            self.assertEqual(rest, "True", "Stdin ist /dev/null")

    def test_kein_schritt_bei_fehlender_bruecke(self):
        self.s = Stub(start_scheitert=[BRUECKE], dateien=True)
        arbeit = tempfile.mkdtemp(prefix="dienste-arbeit-")
        ergebnis = os.path.join(arbeit, "ergebnis.json")
        try:
            with mock.patch.object(probelauf, "voraussetzungen", return_value=None), \
                    mock.patch.object(ablauf, "fuehre_aus") as fuehre:
                with mock.patch.dict(os.environ, {"PATH": self.s.ordner + os.pathsep + os.environ.get("PATH", ""),
                                                  "XDG_RUNTIME_DIR": self.s.xdg}):
                    code, d, _, _ = probelauf.lauf(self.s.k(), arbeit, ergebnis, HIER)
            self.assertEqual(code, 3)
            self.assertEqual(d["voraussetzung"], "bruecke.dienst-startet-nicht")
            self.assertEqual([x["schritt"] for x in d["schritte"]], probelauf.SCHRITTE)
            fuehre.assert_not_called()
            self.assertEqual(self.s.aufrufe_mit("stop"), [KERN])
        finally:
            shutil.rmtree(arbeit, ignore_errors=True)

    def test_stopp_auch_bei_fehler_im_lauf(self):
        self.s = Stub(dateien_bei=BRUECKE, dateien=True)
        arbeit = tempfile.mkdtemp(prefix="dienste-arbeit-")
        try:
            with mock.patch.object(probelauf, "voraussetzungen", return_value=None), \
                    mock.patch.object(ablauf, "fuehre_aus", side_effect=RuntimeError("Absturz im Lauf")):
                with mock.patch.dict(os.environ, {"PATH": self.s.ordner + os.pathsep + os.environ.get("PATH", ""),
                                                  "XDG_RUNTIME_DIR": self.s.xdg}):
                    with self.assertRaises(RuntimeError):
                        probelauf.lauf(self.s.k(), arbeit, os.path.join(arbeit, "ergebnis.json"), HIER)
            self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE, KERN], "auch bei einem Fehler im Lauf gestoppt")
        finally:
            shutil.rmtree(arbeit, ignore_errors=True)

    def test_ungueltiger_name(self):
        self.s = Stub()
        code, gestartet, _ = self.s.lauf(self.s.k(dienste=["-x", KERN]))
        self.assertEqual(code, "konfiguration.fehlt")
        self.assertEqual(self.s.aufrufe(), [], "kein Aufruf bei ungueltigem Namen")


if __name__ == "__main__":
    unittest.main()
