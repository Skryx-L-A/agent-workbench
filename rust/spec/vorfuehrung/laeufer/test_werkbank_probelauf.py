"""Tests fuer `werkbank-probelauf`. Nur Standardbibliothek. Aufruf: python3 -I -B -m unittest test_werkbank_probelauf

Alles laeuft in Wegwerf-Ordnern unter $TMPDIR. `WERKBANK_BEFEHL` (Testhilfe) waehlt eine andere Fassung des
Befehls, etwa einen absichtlich fehlerhaften Mutanten fuer die Gegenprobe.
"""

import importlib.machinery
import importlib.util
import json
import os
import pty
import select
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
BEFEHL = os.environ.get("WERKBANK_BEFEHL") or os.path.join(HIER, "werkbank-probelauf")

TEST_SCHLUESSEL = "sk-TESTKEY-4711-Zq9xW"
TEST_ADRESSE = "https://gw-test.beispiel.test:8443/api/v1"
SCHLUESSEL_PROMPT = b"Schluessel der IT"
ADRESSE_PROMPT = b"Adresse des Gateways"

EINST = (
    '{\n "fassung": 2,\n "klassen": ["a", "b"],\n  "routen": [\n'
    '  {"schluessel":"cloud1","art":"cloud","klasse":"a","netzziele":[]},\n'
    '  {"schluessel" : "lokal",  "art":"lokal", "klasse":"b", "ziel":"http://127.0.0.1:1/v1",\n'
    '   "modell":"m", "netzziele":[]}\n ],\n "kunden":[]\t}\n'
)

FALSCHE_ADRESSEN = ["", "gw.firma.test", "ftp://gw.firma.test:443/v1", "http://gw.firma.test:80/v1",
                    "https://gw.firma.test/v1", "https://gw.firma.test:443/v2", "https://8.8.8.8:443/v1"]
FALSCHE_SCHLUESSEL = ["", "mit leerzeichen", "ae\u00e4", "x" * 900 + " y"]  # Zeilengrenze des Terminals: 1024; 4097 Zeichen siehe TestForm

ATTRAPPE = """\
import json, os, sys
with open(os.environ["ATTRAPPE_PROTOKOLL"], "w") as f:
    json.dump({"argv": sys.argv[1:], "isoliert": sys.flags.isolated, "kein_bytecode": sys.flags.dont_write_bytecode}, f)
print("attrappe laeuft")
sys.exit(int(os.environ.get("ATTRAPPE_EXIT", "0")))
"""


def lies(pfad, modus="rb"):
    with open(pfad, modus) as f:
        return f.read()


def lade_modul():
    loader = importlib.machinery.SourceFileLoader("werkbank_probelauf_test", BEFEHL)
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


def resource_unendlich():
    import resource
    return resource.RLIM_INFINITY


class Abschirmen(unittest.TestCase):
    """Offener Punkt vom 21.11.: Der Prozess, der den Schluessel der IT liest, ist nicht abzugsfaehig und hat RLIMIT_CORE weich 1 und hart 1
    (Vertrag `dienst`, Abschnitt 16; vorher 0/0)."""

    PROBE = ("import ctypes, importlib.machinery, importlib.util, resource, sys\n"
             "l = importlib.machinery.SourceFileLoader('wp', sys.argv[1])\n"
             "m = importlib.util.module_from_spec(importlib.util.spec_from_loader(l.name, l)); l.exec_module(m)\n"
             "if sys.argv[2] == 'ja': m.abschirmen()\n"
             "print(ctypes.CDLL(None).prctl(3, 0, 0, 0, 0), resource.getrlimit(resource.RLIMIT_CORE))\n"
             "import subprocess\n"
             "print(subprocess.run([sys.executable, '-I', '-c', 'import resource; print(resource.getrlimit(resource.RLIMIT_CORE))'],"
             " capture_output=True, text=True).stdout.strip())\n")

    @unittest.skipUnless(sys.platform.startswith("linux"), "PR_SET_DUMPABLE gibt es nur unter Linux")
    def test_abschirmen_macht_den_prozess_nicht_abzugsfaehig(self):
        p = subprocess.run([sys.executable, "-I", "-B", "-c", self.PROBE, BEFEHL, "ja"], capture_output=True, text=True, timeout=30)
        self.assertEqual(p.stdout.strip().splitlines(), ["0 (1, 1)", "(1, 1)"], p.stderr)  # nicht abzugsfaehig, Grenze 1/1, das Kind erbt 1/1
        # Gegenprobe: ohne abschirmen ist derselbe Prozess abzugsfaehig, die Messung unterscheidet also
        p = subprocess.run([sys.executable, "-I", "-B", "-c", self.PROBE, BEFEHL, "nein"], capture_output=True, text=True, timeout=30)
        self.assertTrue(p.stdout.strip().startswith("1 "), p.stdout + p.stderr)

    def test_falltabelle_vertrag_dienst_16(self):
        mod = lade_modul()
        nl = mod.NICHT_LESBAR
        faelle = [(1, "|/x", ("setzen", None)), (1, None, ("setzen", None)), (1, nl, ("setzen", None)), (-1, "|/x", ("setzen", None)),
                  (0, None, ("weiter", "muster_fehlt")), (0, "core", ("weiter", "muster_datei")), (0, "/tmp/core.%p", ("weiter", "muster_datei")),
                  (0, "", ("weiter", "muster_datei")), (0, "|/usr/share/apport/apport %p", ("abbruch", "muster_pipe")),
                  (0, nl, ("abbruch", "muster_nicht_lesbar"))]
        for harte, muster, soll in faelle:
            with self.subTest(harte=harte, muster=muster if muster is not nl else "nicht lesbar"):
                self.assertEqual(mod.kernabbild_entscheiden(harte, muster), soll)

    def test_harte_grenze_0_wird_nicht_gesetzt_oder_bricht_ab(self):
        import resource
        mod = lade_modul()
        for muster, erwartet in (("core", None), ("|/x", 5), (mod.NICHT_LESBAR, 5), (None, None)):
            with self.subTest(muster=str(muster)), mock.patch("resource.getrlimit", return_value=(0, 0)), \
                    mock.patch("resource.setrlimit") as gesetzt, mock.patch.object(mod, "core_pattern_lesen", return_value=muster), \
                    mock.patch("ctypes.CDLL", return_value=mock.Mock(prctl=mock.Mock(return_value=0))), mock.patch("sys.stderr"):
                if erwartet == 5:
                    with self.assertRaises(mod.Ende) as e:
                        mod.abschirmen()
                    self.assertEqual((e.exception.code if hasattr(e.exception, "code") else None, e.exception.satz), (5, mod.S_SCHUTZ))
                else:
                    try:
                        mod.abschirmen()
                    except mod.Ende:
                        self.fail("Dateimuster oder kein Muster laesst den Lauf weiterlaufen")
                gesetzt.assert_not_called()

    def test_core_pattern_wird_hoechstens_4096_byte_gelesen(self):
        mod = lade_modul()
        daten = mock.mock_open(read_data=b"x" * 5000)
        with mock.patch("builtins.open", daten):
            mod.core_pattern_lesen()
        daten().read.assert_called_once_with(4096)

    def test_gescheiterter_schutz_endet_vor_jeder_eingabe(self):
        mod = lade_modul()
        with mock.patch("resource.getrlimit", return_value=(1, resource_unendlich())), \
                mock.patch("resource.setrlimit", side_effect=OSError("verweigert")), \
                mock.patch.object(mod, "einrichten", side_effect=AssertionError("keine Eingabe ohne Schutz")):
            self.assertEqual(mod.main(["--ergebnis", "/nicht/da"]), 5)

    def test_andere_voraussetzung_bleibt_exit_3(self):
        # Schutz gelingt, der Datenordner fehlt: Exit 3, nicht 5
        mod = lade_modul()
        with mock.patch.object(mod, "abschirmen"), mock.patch.dict(os.environ, {"WERKBANK_DATEN": "/nicht/da/wp"}), \
                mock.patch("sys.stderr"):
            self.assertEqual(mod.main(["--ergebnis", "/nicht/da"]), 3)


class Basis(unittest.TestCase):
    def setUp(self):
        self.wurzel = tempfile.mkdtemp(prefix="wp-test-")
        self.addCleanup(shutil.rmtree, self.wurzel, True)
        self.daten = os.path.join(self.wurzel, "daten")
        os.mkdir(self.daten, 0o700)
        self.einst = os.path.join(self.daten, "einstellungen.json")
        self.schl = os.path.join(self.daten, "modellweg.schluessel")
        self.erg = os.path.join(self.wurzel, "ergebnis.json")
        self.attrappe = os.path.join(self.wurzel, "drehbuch.py")
        self.protokoll = os.path.join(self.wurzel, "attrappe.json")
        with open(self.attrappe, "w") as f:
            f.write(ATTRAPPE)
        self.konfig = os.path.join(self.wurzel, "konfig.json")
        self.schreibe_einst(EINST)

    def schreibe_einst(self, text):
        with open(self.einst, "wb") as f:
            f.write(text.encode("utf-8"))

    def umgebung(self, **extra):
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "WERKBANK_DATEN": self.daten,
               "WERKBANK_DREHBUCH": self.attrappe, "WERKBANK_KONFIG": self.konfig,
               "ATTRAPPE_PROTOKOLL": self.protokoll, "TMPDIR": self.wurzel}
        env.update(extra)
        return env

    def lauf_pty(self, argv, eingaben, env=None, umask=0o022, frist=15.0):
        """Startet den Befehl mit Pseudoterminal. eingaben: Liste (Prompt-Bytes, Antwort). Ergebnis: (exit, ausgabe)."""
        env = env or self.umgebung()
        pid, master = pty.fork()
        if pid == 0:
            try:
                os.umask(umask)
                os.execve(sys.executable, [sys.executable, "-B", BEFEHL] + argv, env)
            finally:
                os._exit(127)
        puffer = b""
        neu = b""
        offen = list(eingaben)
        ende = time.monotonic() + frist
        while time.monotonic() < ende:
            r, _, _ = select.select([master], [], [], 0.2)
            if not r:
                continue
            try:
                teil = os.read(master, 4096)
            except OSError:
                teil = b""
            if not teil:
                break
            puffer += teil
            neu += teil
            if offen and offen[0][0] in neu:
                antwort = offen.pop(0)[1]
                neu = b""
                os.write(master, antwort.encode("utf-8") + b"\n")
        else:
            os.kill(pid, 9)
        _, status = os.waitpid(pid, 0)
        os.close(master)
        self.assertEqual(offen, [], "nicht alle Eingaben wurden angefordert")
        return os.waitstatus_to_exitcode(status), puffer

    def lauf_rohr(self, argv, eingabe="", env=None):
        """Ohne Terminal: stdin aus Rohr, stdout und stderr getrennt (getpass faellt auf stdin zurueck)."""
        p = subprocess.run([sys.executable, "-B", BEFEHL] + argv, input=eingabe.encode(), capture_output=True,
                           env=env or self.umgebung(), start_new_session=True, timeout=15)
        return p.returncode, p.stdout, p.stderr

    def lies_protokoll(self):
        with open(self.protokoll) as f:
            return json.load(f)

    def einrichtung_eingaben(self, adresse=TEST_ADRESSE, schluessel=TEST_SCHLUESSEL):
        return [(ADRESSE_PROMPT, adresse), (SCHLUESSEL_PROMPT, schluessel)]


class TestForm(unittest.TestCase):
    def test_grenzen_von_schluessel_und_adresse(self):
        mod = lade_modul()
        self.assertTrue(mod.schluessel_ok("x" * 4096))
        self.assertFalse(mod.schluessel_ok("x" * 4097))
        self.assertFalse(mod.schluessel_ok("a\nb"))
        self.assertFalse(mod.schluessel_ok("abc\n"))
        self.assertTrue(mod.adresse_ok("https://gw.firma.test:443/v1"))
        self.assertTrue(mod.adresse_ok("http://127.0.0.1:8080/v1"))
        self.assertFalse(mod.adresse_ok("https://gw.firma.test:443/v1\n"))


class TestAufrufUndOrdner(Basis):
    def test_falscher_aufruf_exit_2_ohne_datei(self):
        for argv in ([], ["--ergebnis"], ["--unbekannt"], ["--ergebnis", self.erg, "extra"], ["--ergebnis", ""],
                     ["--erg", self.erg]):
            vorher = sorted(os.listdir(self.daten))
            code, aus, fehler = self.lauf_rohr(argv)
            self.assertEqual(code, 2, argv)
            self.assertFalse(os.path.exists(self.erg))
            self.assertEqual(sorted(os.listdir(self.daten)), vorher)

    def test_ordner_fehlt_oder_fremd_exit_3(self):
        env = self.umgebung(WERKBANK_DATEN=os.path.join(self.wurzel, "gibt-es-nicht"))
        code, aus, fehler = self.lauf_rohr(["--ergebnis", self.erg], env=env)
        self.assertEqual(code, 3)
        self.assertIn(b"Der Datenordner fehlt oder gehoert nicht dem Nutzer.", fehler)
        # Link auf einen Ordner zaehlt nicht
        link = os.path.join(self.wurzel, "link")
        os.symlink(self.daten, link)
        code, _, fehler = self.lauf_rohr(["--ergebnis", self.erg], env=self.umgebung(WERKBANK_DATEN=link))
        self.assertEqual(code, 3)
        # Ordner eines anderen Nutzers (uid-Vergleich ersetzt durch Attrappe im Modul)
        mod = lade_modul()
        with mock.patch.dict(os.environ, {"WERKBANK_DATEN": self.daten}), \
                mock.patch.object(mod.os, "getuid", return_value=os.getuid() + 1):
            with self.assertRaises(mod.Ende) as cm:
                mod.datenordner()
            self.assertEqual(cm.exception.code, 3)
        self.assertFalse(os.path.exists(self.erg))


class TestEinrichtung(Basis):
    def test_einrichtung_mit_pty_ohne_echo_und_modus_0600(self):
        code, aus = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben(), umask=0o022)
        self.assertEqual(code, 0, aus)
        self.assertNotIn(TEST_SCHLUESSEL.encode(), aus)
        self.assertNotIn(TEST_ADRESSE.encode(), aus)
        st = os.lstat(self.schl)
        self.assertTrue(stat.S_ISREG(st.st_mode))
        self.assertEqual(stat.S_IMODE(st.st_mode), 0o600)
        with open(self.schl) as f:
            self.assertEqual(f.read(), TEST_SCHLUESSEL + "\n")
        self.assertEqual(stat.S_IMODE(os.stat(self.einst).st_mode), 0o600)
        self.assertEqual(sorted(os.listdir(self.daten)), ["einstellungen.json", "modellweg.schluessel"])

    def test_einstellungsdatei_gueltig_ziel_gesetzt_rest_byte_gleich(self):
        vorher = lies(self.einst)
        code, aus = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())
        self.assertEqual(code, 0)
        nachher = lies(self.einst)
        d = json.loads(nachher)
        lokal = [r for r in d["routen"] if r["schluessel"] == "lokal"]
        self.assertEqual(len(lokal), 1)
        self.assertEqual(lokal[0]["ziel"], TEST_ADRESSE)
        alt = '"http://127.0.0.1:1/v1"'.encode()
        neu = json.dumps(TEST_ADRESSE).encode()
        self.assertEqual(vorher.count(alt), 1)
        self.assertEqual(nachher, vorher.replace(alt, neu))

    def test_route_wird_nach_schluessel_gefunden_nicht_nach_index(self):
        text = EINST.replace('{"schluessel":"cloud1"', '{"schluessel":"lokal2","art":"lokal","klasse":"b",'
                             '"ziel":"http://127.0.0.1:2/v1","modell":"m","netzziele":[]},\n  {"schluessel":"cloud1"', 1)
        self.schreibe_einst(text)
        code, _ = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())
        self.assertEqual(code, 0)
        d = json.loads(lies(self.einst))
        ziele = {r["schluessel"]: r.get("ziel") for r in d["routen"]}
        self.assertEqual(ziele["lokal"], TEST_ADRESSE)
        self.assertEqual(ziele["lokal2"], "http://127.0.0.1:2/v1")

    def test_einstellungsdatei_ohne_lokale_route_exit_3_ohne_schluessel(self):
        self.schreibe_einst(EINST.replace('"lokal",  "art"', '"nein",  "art"'))
        vorher = lies(self.einst)
        code, aus = self.lauf_pty(["--ergebnis", self.erg], [])
        self.assertEqual(code, 3)
        self.assertFalse(os.path.exists(self.schl))
        self.assertEqual(lies(self.einst), vorher)

    def test_falsche_adresse_fester_satz_kein_echo_dreimal_dann_exit_3(self):
        for falsch in FALSCHE_ADRESSEN:
            with self.subTest(falsch=falsch):
                vorher = lies(self.einst)
                code, aus = self.lauf_pty(["--ergebnis", self.erg], [(ADRESSE_PROMPT, falsch)] * 3)
                self.assertEqual(code, 3)
                self.assertEqual(aus.count(b"Die Adresse hat nicht die Form"), 3)
                if falsch:
                    self.assertNotIn(falsch.encode(), aus)
                self.assertIn(b"Zu viele falsche Eingaben", aus)
                self.assertFalse(os.path.exists(self.schl))
                self.assertEqual(lies(self.einst), vorher)
                self.assertFalse(os.path.exists(self.erg))

    def test_falscher_schluessel_fester_satz_kein_echo_dreimal_dann_exit_3(self):
        for falsch in FALSCHE_SCHLUESSEL:
            with self.subTest(falsch=falsch[:12]):
                vorher = lies(self.einst)
                code, aus = self.lauf_pty(["--ergebnis", self.erg],
                                          [(ADRESSE_PROMPT, TEST_ADRESSE)] + [(SCHLUESSEL_PROMPT, falsch)] * 3)
                self.assertEqual(code, 3)
                self.assertEqual(aus.count(b"Der Schluessel hat nicht die erlaubte Form"), 3)
                if falsch.strip():
                    self.assertNotIn(falsch[:40].encode("utf-8"), aus)
                self.assertFalse(os.path.exists(self.schl))
                self.assertEqual(lies(self.einst), vorher, "Datei darf nicht halb geschrieben sein")

    def test_zweiter_versuch_gelingt(self):
        code, aus = self.lauf_pty(["--ergebnis", self.erg],
                                  [(ADRESSE_PROMPT, "kaputt"), (ADRESSE_PROMPT, TEST_ADRESSE),
                                   (SCHLUESSEL_PROMPT, "mit leerzeichen"), (SCHLUESSEL_PROMPT, TEST_SCHLUESSEL)])
        self.assertEqual(code, 0, aus)
        self.assertEqual(lies(self.schl, "r"), TEST_SCHLUESSEL + "\n")

    def test_neu_einrichten_fragt_erneut_sonst_nicht(self):
        self.assertEqual(self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())[0], 0)
        # vorhanden, ohne Schalter: keine Frage (mit leerer Eingabeliste; eine Frage wuerde haengen bis zur Frist)
        t0 = time.monotonic()
        code, aus = self.lauf_pty(["--ergebnis", self.erg], [], frist=10)
        self.assertEqual(code, 0)
        self.assertNotIn(b"Adresse des Gateways", aus)
        self.assertNotIn(b"Schluessel der IT", aus)
        self.assertLess(time.monotonic() - t0, 8)
        # mit Schalter: fragt erneut und ersetzt beides
        neu_s, neu_a = "sk-NEU-98765", "https://gw2-test.beispiel.test:9443/v1"
        code, aus = self.lauf_pty(["--ergebnis", self.erg, "--neu-einrichten"],
                                  self.einrichtung_eingaben(neu_a, neu_s))
        self.assertEqual(code, 0, aus)
        self.assertEqual(lies(self.schl, "r"), neu_s + "\n")
        self.assertEqual(stat.S_IMODE(os.stat(self.schl).st_mode), 0o600)
        ziel = [r for r in json.loads(lies(self.einst))["routen"] if r["schluessel"] == "lokal"][0]["ziel"]
        self.assertEqual(ziel, neu_a)
        self.assertNotIn(neu_s.encode(), aus)

    def test_nie_eine_datei_mit_weiteren_rechten_im_code(self):
        """Im Code geprueft, nicht mit der Uhr: jedes os.open im Datenordner legt mit O_EXCL|O_NOFOLLOW und 0600 an,
        die Datei hat zum Zeitpunkt von os.replace Modus 0600, und die umask ist 077 (der Test setzt 022)."""
        mod = lade_modul()
        oeffnungen, ersetzungen = [], []
        echt_open, echt_replace = os.open, os.replace

        def spion_open(pfad, flags, mode=0o777, *a, **k):
            oeffnungen.append((pfad, flags, mode))
            return echt_open(pfad, flags, mode, *a, **k)

        def spion_replace(quelle, ziel):
            ersetzungen.append((quelle, ziel, stat.S_IMODE(os.lstat(quelle).st_mode),
                                stat.S_ISREG(os.lstat(quelle).st_mode)))
            return echt_replace(quelle, ziel)

        alte_umask = os.umask(0o022)
        try:
            with mock.patch.object(mod.os, "open", spion_open), mock.patch.object(mod.os, "replace", spion_replace), \
                    mock.patch.object(mod.getpass, "getpass", side_effect=[TEST_ADRESSE, TEST_SCHLUESSEL]):
                mod.einrichten(self.daten)
            neue_umask = os.umask(0o022)
        finally:
            os.umask(alte_umask)
        self.assertEqual(neue_umask & 0o077, 0o077, "umask muss 077 sein")
        self.assertEqual(len(oeffnungen), 2)
        for pfad, flags, mode in oeffnungen:
            self.assertEqual(os.path.dirname(pfad), self.daten)
            self.assertEqual(mode, 0o600)
            for f in (os.O_WRONLY, os.O_CREAT, os.O_EXCL, os.O_NOFOLLOW):
                self.assertTrue(flags & f)
        self.assertEqual([(os.path.basename(z), m, r) for _, z, m, r in ersetzungen],
                         [("einstellungen.json", 0o600, True), ("modellweg.schluessel", 0o600, True)])
        self.assertEqual(sorted(os.listdir(self.daten)), ["einstellungen.json", "modellweg.schluessel"])

    def test_schluesseldatei_endet_nie_als_link_oder_fremder_modus(self):
        os.symlink("/etc/hosts", os.path.join(self.daten, ".modellweg.schluessel.tmp-1"))  # fremde Datei stoert nicht
        code, aus = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())
        self.assertEqual(code, 0)
        st = os.lstat(self.schl)
        self.assertTrue(stat.S_ISREG(st.st_mode))
        self.assertEqual(stat.S_IMODE(st.st_mode), 0o600)


class TestProbelaufAufruf(Basis):
    def einrichten(self):
        code, _ = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())
        self.assertEqual(code, 0)

    def test_exitcodes_werden_weitergegeben(self):
        self.einrichten()
        for soll in (0, 1, 2, 3):
            env = self.umgebung(ATTRAPPE_EXIT=str(soll))
            code, aus = self.lauf_pty(["--ergebnis", self.erg], [], env=env)
            self.assertEqual(code, soll)
            p = self.lies_protokoll()
            self.assertEqual(p["argv"], ["probelauf", "--ergebnis", self.erg, "--konfig", self.konfig])
            self.assertEqual(p["isoliert"], 1)
            self.assertEqual(p["kein_bytecode"], 1)
        code, _ = self.lauf_pty(["--ergebnis", self.erg], [], env=self.umgebung(ATTRAPPE_EXIT="7"))
        self.assertEqual(code, 3, "unbekannter Code wird 3")

    def test_nur_cloud_wird_weitergegeben_und_exit_4_bleibt(self):
        self.einrichten()
        code, _ = self.lauf_pty(["--ergebnis", self.erg, "--nur-cloud"], [], env=self.umgebung(ATTRAPPE_EXIT="4"))
        self.assertEqual(code, 4, "Teilumfang: ein eigener fester Code, nie 0")
        self.assertEqual(self.lies_protokoll()["argv"], ["probelauf", "--ergebnis", self.erg, "--konfig", self.konfig, "--nur-cloud"])
        code, _ = self.lauf_pty(["--ergebnis", self.erg], [], env=self.umgebung(ATTRAPPE_EXIT="0"))
        self.assertEqual(self.lies_protokoll()["argv"], ["probelauf", "--ergebnis", self.erg, "--konfig", self.konfig],
                         "ohne den Schalter geht er nie mit")

    def test_einrichtung_vor_dem_aufruf_und_leckpruefung(self):
        env = self.umgebung(ATTRAPPE_EXIT="1")
        code, stdout, stderr = self.lauf_rohr(["--ergebnis", self.erg], eingabe=TEST_ADRESSE + "\n" + TEST_SCHLUESSEL + "\n",
                                              env=env)
        self.assertEqual(code, 1)
        for strom in (stdout, stderr):
            self.assertEqual(strom.count(TEST_SCHLUESSEL.encode()), 0)
            self.assertEqual(strom.count(TEST_ADRESSE.encode()), 0)
        self.assertIn(b"attrappe laeuft", stdout)
        self.assertEqual(lies(self.schl, "r"), TEST_SCHLUESSEL + "\n")

    def test_leckpruefung_ergebnisdatei_und_alle_dateien(self):
        """Eine Attrappe, die die Ergebnisdatei schreibt, sieht Schluessel und Adresse nie; ausser den zwei Dateien
        steht beides in keiner Datei unter dem Wegwerf-Ordner."""
        with open(self.attrappe, "w") as f:
            f.write("import sys\nopen(sys.argv[sys.argv.index('--ergebnis') + 1], 'w').write('{\"ok\": true}')\n"
                    "print('lauf')\n")
        code, aus = self.lauf_pty(["--ergebnis", self.erg], self.einrichtung_eingaben())
        self.assertEqual(code, 0)
        self.assertEqual(aus.count(TEST_SCHLUESSEL.encode()) + aus.count(TEST_ADRESSE.encode()), 0)
        treffer = []
        for wurzel, _, dateien in os.walk(self.wurzel):
            for n in dateien:
                pfad = os.path.join(wurzel, n)
                daten = lies(pfad)
                if TEST_SCHLUESSEL.encode() in daten or TEST_ADRESSE.encode() in daten:
                    treffer.append(pfad)
        self.assertEqual(sorted(treffer), sorted([self.einst, self.schl]))
        self.assertEqual(lies(self.erg).count(TEST_SCHLUESSEL.encode()), 0)


SIGNAL_ATTRAPPE = """\
import os, signal, sys, time
def ende(n, _f):
    time.sleep(1.0)  # Aufraeumen dauert laenger als die 0,25 s, die subprocess.call nach Strg-C wartet
    open(os.environ["ATTRAPPE_PROTOKOLL"] + ".aufgeraeumt", "w").close()
    sys.exit(128 + n)
for s in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
    signal.signal(s, ende)
open(os.environ["ATTRAPPE_PROTOKOLL"] + ".bereit", "w").close()
time.sleep(30)
sys.exit(0)
"""


class Signale(Basis):
    """Befund B1 und B8: Der Befehl laesst den Laeufer fertig aufraeumen und gibt 128 + Signal durch
    (Projektentscheid, 23.11.2026). Signale gehen nur an eigene Kindprozesse in einer eigenen Sitzung."""

    def starte(self):
        with open(self.attrappe, "w") as f:
            f.write(SIGNAL_ATTRAPPE)
        fd = os.open(self.schl, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)  # eingerichtet: keine Abfrage
        os.write(fd, (TEST_SCHLUESSEL + "\n").encode())
        os.close(fd)
        p = subprocess.Popen([sys.executable, "-B", BEFEHL, "--ergebnis", self.erg], env=self.umgebung(),
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             start_new_session=True)
        self.addCleanup(self.aufraeumen, p)
        ende = time.monotonic() + 10
        while not os.path.exists(self.protokoll + ".bereit"):
            self.assertLess(time.monotonic(), ende, "Attrappe nicht bereit")
            self.assertIsNone(p.poll(), "der Befehl endete vor dem Laeufer")
            time.sleep(0.05)
        return p

    @staticmethod
    def aufraeumen(p):
        try:
            os.killpg(p.pid, signal.SIGKILL)  # die eigene Gruppe, auch ein verwaister Laeufer
        except (ProcessLookupError, PermissionError):
            pass
        p.wait()

    def pruefe(self, p, soll):
        p.wait(timeout=10)
        self.assertTrue(os.path.exists(self.protokoll + ".aufgeraeumt"), "der Wrapper hat den Laeufer im Aufraeumen getoetet")
        self.assertEqual(p.returncode, soll, "128 + Signal, nicht 3: nach einem Signal gibt es keine Ergebnisdatei")

    def test_strg_c_laesst_den_laeufer_fertig_aufraeumen(self):
        p = self.starte()
        os.killpg(p.pid, signal.SIGINT)  # Strg-C trifft die ganze Vordergrundgruppe
        self.pruefe(p, 130)

    def test_sigterm_an_den_befehl_geht_an_den_laeufer(self):
        p = self.starte()
        os.kill(p.pid, signal.SIGTERM)  # nur der Befehl, etwa `kill <pid>`
        self.pruefe(p, 143)

    def test_sighup_an_den_befehl_geht_an_den_laeufer(self):
        p = self.starte()
        os.kill(p.pid, signal.SIGHUP)
        self.pruefe(p, 129)

    def test_sighup_an_die_gruppe_raeumt_einmal_auf(self):
        p = self.starte()
        os.killpg(p.pid, signal.SIGHUP)  # ssh-Abbruch: Gruppe und Weiterreichen, der Laeufer bekommt es zweimal
        self.pruefe(p, 129)

    def test_abbildung_der_exitcodes(self):
        mod = lade_modul()
        faelle = {0: 0, 1: 1, 2: 2, 3: 3, 4: 4, 5: 3, 7: 3, 129: 129, 130: 130, 143: 143, 137: 3,
                  -signal.SIGINT: 130, -signal.SIGTERM: 143, -signal.SIGHUP: 129, -signal.SIGKILL: 3}
        for code, soll in faelle.items():
            with self.subTest(code=code):
                self.assertEqual(mod.exitcode_des_laeufers(code), soll)


if __name__ == "__main__":
    unittest.main()
