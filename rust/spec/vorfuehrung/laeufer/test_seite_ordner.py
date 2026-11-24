"""Seitenordner der Attrappe (21.11.): `seite_ordner` in konfig.attrappe.json.

Ist er gesetzt, liefert die Attrappe genau drei Dateien an genau drei Pfade (`/`, `/ansicht.js`, `/ansicht_bg.wasm`) mit den Typen und der
CSP der echten Bruecke (rust/crates/bruecke/src/seite.rs und verbindung.rs). Jeder andere Pfad ergibt 404, ein Verweis oder eine fehlende
Datei beendet den Start mit festem Text, und ohne `seite_ordner` bleibt es bei seite.html. Das Testpaket ist klein und ohne dioxus.

Nur Standardbibliothek. Aufruf: python3 -I -B test_seite_ordner.py   (startet die Attrappe der Bruecke)
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)
sys.path.insert(0, os.path.join(HIER, "attrappen"))

import seitenordner  # noqa: E402

KENNUNGEN = ["sitzung-start", "harness-wahl", "route-wahl", "modell-wahl", "kunde-wahl", "arbeitsordner-eingabe", "sitzung-liste", "sitzung-{id}",
             "terminal-raster", "terminal-zeile-{n}", "eingabe", "eingabe-senden", "fehler", "fehler-code", "fehler-text"]
INDEX = ("<!doctype html><html><body>" + "".join('<div data-testid="%s"></div>' % k for k in KENNUNGEN)
         + '<script type="module" src="ansicht.js"></script></body></html>').encode()
JS = b"export default async function init() { return fetch('ansicht_bg.wasm'); }\n"
WASM = b"\x00asm\x01\x00\x00\x00"  # kleinstes gueltiges Modul: Kopf ohne Abschnitte


def paket(ordner, weglassen=()):
    os.makedirs(ordner, exist_ok=True)
    for name, inhalt in (("index.html", INDEX), ("ansicht.js", JS), ("ansicht_bg.wasm", WASM)):
        if name not in weglassen:
            with open(os.path.join(ordner, name), "wb") as f:
                f.write(inhalt)


class Attrappe:
    def __init__(self, wurzel, seite_ordner=None):
        self.token = "t" * 32
        konf = {"token": self.token, "harness": {}, "einstellungen": os.path.join(wurzel, "e.json")}
        if seite_ordner is not None:
            konf["seite_ordner"] = seite_ordner
        self.kpfad, self.port = os.path.join(wurzel, "k.json"), os.path.join(wurzel, "bruecke.port")
        with open(self.kpfad, "w") as f:
            json.dump(konf, f)
        self.p = subprocess.Popen([sys.executable, "-I", "-B", os.path.join(HIER, "attrappen", "bruecke_attrappe.py"), "--konfig", self.kpfad,
                                   "--portdatei", self.port], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        ende = time.monotonic() + 5
        while time.monotonic() < ende and not os.path.exists(self.port) and self.p.poll() is None:
            time.sleep(0.02)
        self.nr = int(open(self.port).read()) if os.path.exists(self.port) and open(self.port).read().strip() else None

    def roh(self, pfad, token=True):
        """Roher GET (kein Normalisieren des Pfads); gibt (Status, Koepfe in Kleinschrift, Koerper)."""
        s = socket.create_connection(("127.0.0.1", self.nr), timeout=5)
        kopf = "GET %s HTTP/1.1\r\nHost: 127.0.0.1:%d\r\nConnection: close\r\n" % (pfad, self.nr)
        if token:
            kopf += "Authorization: Bearer %s\r\n" % self.token
        s.sendall((kopf + "\r\n").encode())
        d = b""
        while True:
            c = s.recv(65536)
            if not c:
                break
            d += c
        s.close()
        k, _, b = d.partition(b"\r\n\r\n")
        zeilen = k.decode("latin-1").split("\r\n")
        return int(zeilen[0].split()[1]), {z.split(":", 1)[0].lower(): z.split(":", 1)[1].strip() for z in zeilen[1:]}, b

    def ende(self):
        if self.p.poll() is None:
            self.p.terminate()
            try:
                self.p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.p.kill()
                self.p.wait(timeout=5)
        if self.p.stderr:
            self.p.stderr.close()


class Basis(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory(prefix="so-", dir=os.environ.get("TMPDIR") or None)
        self.addCleanup(self.t.cleanup)
        self.w = os.path.realpath(self.t.name)
        self.o = os.path.join(self.w, "pakett")

    def start(self, ordner):
        a = Attrappe(self.w, ordner)
        self.addCleanup(a.ende)
        return a


class TestAuslieferung(Basis):
    def test_drei_pfade_mit_typ_und_csp(self):
        paket(self.o)
        a = self.start(self.o)
        self.assertIsNotNone(a.nr, a.p.stderr.read() if a.p.poll() is not None else "")
        for pfad, typ, inhalt in (("/", "text/html; charset=utf-8", INDEX), ("/ansicht.js", "text/javascript; charset=utf-8", JS),
                                  ("/ansicht_bg.wasm", "application/wasm", WASM)):
            for token in (False, True):  # die Seite selbst braucht kein Token (verbindung.rs: pruefung.rs Zeile 38)
                st, k, b = a.roh(pfad, token)
                self.assertEqual((st, k["content-type"], b), (200, typ, inhalt), pfad)
                self.assertEqual(k["content-security-policy"], seitenordner.CSP)
                self.assertEqual((k["cache-control"], k["x-content-type-options"], k["content-length"]), ("no-store", "nosniff", str(len(inhalt))))

    def test_csp_ist_die_der_bruecke_mit_wasm(self):
        """Gleich seite.rs, nur script-src mit 'wasm-unsafe-eval'."""
        text = open(os.path.join(HIER, "..", "..", "..", "crates", "bruecke", "src", "seite.rs"), encoding="utf-8").read()
        zeilen = text.split('pub(crate) const CSP: &str = "')[1].split('";')[0].replace('\\\n', "")
        self.assertEqual(zeilen.replace("script-src 'self';", "script-src 'self' 'wasm-unsafe-eval';"), seitenordner.CSP)

    def test_alles_andere_ist_404(self):
        paket(self.o)
        with open(os.path.join(self.o, "geheim.txt"), "w") as f:
            f.write("x")
        a = self.start(self.o)
        for pfad in ("/index.html", "/seite.html", "/geheim.txt", "/x", "/ansicht.js?x=1", "/ansicht.js/", "/ansicht_bg.wasm.map", "//", "/./ansicht.js",
                     "/../etc/passwd", "/..%2f..%2fetc/passwd", "/%2e%2e/%2e%2e/etc/passwd", "/ansicht.js%00", "/%61nsicht.js"):
            st, k, b = a.roh(pfad)
            self.assertEqual((st, b), (404, b""), pfad)
            self.assertNotIn("content-security-policy", k, pfad)

    def test_verweis_beendet_den_start_ohne_rueckfall(self):
        paket(self.o)
        ziel = os.path.join(self.w, "aussen.js")
        with open(ziel, "wb") as f:
            f.write(b"aussen")
        os.remove(os.path.join(self.o, "ansicht.js"))
        os.symlink(ziel, os.path.join(self.o, "ansicht.js"))
        a = self.start(self.o)
        self.assertEqual(a.p.wait(timeout=5), 2)
        self.assertFalse(os.path.exists(a.port))
        self.assertIn("Seitenordner: Abbruch: Seitenordner unvollstaendig: ansicht.js fehlt oder ist keine regulaere Datei", a.p.stderr.read().decode())

    def test_verweisordner_als_ordner_ist_erlaubt_aber_dateien_nicht(self):
        """Der Ordner selbst darf ein Verweis sein (der Lauf loest ihn auf), die drei Dateien nie."""
        paket(self.o)
        os.symlink(self.o, os.path.join(self.w, "ln"))
        a = self.start(os.path.join(self.w, "ln"))
        self.assertEqual(a.roh("/ansicht_bg.wasm")[2], WASM)

    def test_fehlende_datei_beendet_den_start(self):
        for fehlt in ("index.html", "ansicht.js", "ansicht_bg.wasm"):
            with self.subTest(fehlt=fehlt):
                o = os.path.join(self.w, "o-" + fehlt)
                paket(o, (fehlt,))
                a = Attrappe(os.path.join(self.w), o)
                try:
                    self.assertEqual(a.p.wait(timeout=5), 2)
                    self.assertEqual(a.p.stderr.read().decode().strip(),
                                     "Seitenordner: Abbruch: Seitenordner unvollstaendig: %s fehlt oder ist keine regulaere Datei" % fehlt)
                finally:
                    a.ende()
                    if os.path.exists(a.port):
                        os.remove(a.port)

    def test_ordner_fehlt(self):
        a = self.start(os.path.join(self.w, "gibtsnicht"))
        self.assertEqual(a.p.wait(timeout=5), 2)
        self.assertIn("Seitenordner: Abbruch: Seitenordner fehlt: kein Ordner", a.p.stderr.read().decode())

    def test_unterordner_zaehlt_nicht_als_datei(self):
        paket(self.o, ("ansicht.js",))
        os.mkdir(os.path.join(self.o, "ansicht.js"))
        a = self.start(self.o)
        self.assertEqual(a.p.wait(timeout=5), 2)

    def test_groessengrenze(self):
        alt, g = seitenordner.GRENZE, len(INDEX) + 100
        seitenordner.GRENZE = g
        try:
            for name in ("index.html", "ansicht.js", "ansicht_bg.wasm"):
                o = os.path.join(self.w, "g-" + name)
                paket(o)
                with open(os.path.join(o, name), "wb") as f:
                    f.write(b"x" * (g + 1))
                with self.assertRaises(seitenordner.SeitenFehler) as e:
                    seitenordner.lade(o)
                self.assertEqual(str(e.exception), "Seitenordner: %s ist groesser als %d Bytes" % (name, g))
                with open(os.path.join(o, name), "wb") as f:
                    f.write(b"x" * g)
                self.assertEqual(len(seitenordner.lade(o)), 3)  # genau an der Grenze geht
        finally:
            seitenordner.GRENZE = alt

    def test_ohne_seite_ordner_wie_heute(self):
        for konf in (None, ""):
            a = self.start(konf)
            eigene = open(os.path.join(HIER, "attrappen", "seite.html"), "rb").read()
            for pfad in ("/", "/index.html"):
                st, k, b = a.roh(pfad, False)
                self.assertEqual((st, k["content-type"], b), (200, "text/html; charset=utf-8", eigene))
                self.assertNotIn("content-security-policy", k)  # unveraendert gegenueber vorher
            self.assertEqual(a.roh("/ansicht.js", False)[0], 401)
            a.ende()
            if os.path.exists(a.port):
                os.remove(a.port)

    def test_diagnose_nennt_nur_namen_und_pruefsummen(self):
        paket(self.o)
        a = self.start(self.o)
        st, _, b = a.roh("/diagnose")
        s = json.loads(b)["seite"]
        self.assertEqual(s["name"], "pakett")
        self.assertEqual(s["sha256"], {"index.html": hashlib.sha256(INDEX).hexdigest(), "ansicht.js": hashlib.sha256(JS).hexdigest(),
                                       "ansicht_bg.wasm": hashlib.sha256(WASM).hexdigest()})
        self.assertNotIn(INDEX, b)


class TestLaeufer(Basis):
    def lauf(self, *args):
        arbeit = os.path.join(self.w, "a")
        return subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "attrappe", "--nur", "S1", "--arbeit", arbeit,
                               "--json", os.path.join(self.w, "b.json"), *args], capture_output=True, text=True, timeout=120, cwd=self.w)

    def test_zeile_und_bericht(self):
        paket(self.o)
        r = self.lauf("--seite-ordner", self.o)
        h = lambda b: hashlib.sha256(b).hexdigest()
        erwartet = "Seite: pakett index.html sha256 %s, ansicht.js sha256 %s, ansicht_bg.wasm sha256 %s" % (h(INDEX), h(JS), h(WASM))
        self.assertIn(erwartet, r.stdout.splitlines(), r.stdout + r.stderr)
        d = json.load(open(os.path.join(self.w, "b.json")))
        self.assertEqual(d["seite"]["name"], "pakett")
        self.assertEqual(d["seite"]["sha256"]["ansicht_bg.wasm"], h(WASM))
        self.assertNotIn("sitzung-start", json.dumps(d["seite"]))

    def test_ohne_ordner_seite_html(self):
        r = self.lauf()
        d = json.load(open(os.path.join(self.w, "b.json")))
        eigene = hashlib.sha256(open(os.path.join(HIER, "attrappen", "seite.html"), "rb").read()).hexdigest()
        self.assertIn("Seite: seite.html index.html sha256 %s, ansicht.js sha256 -, ansicht_bg.wasm sha256 -" % eigene, r.stdout.splitlines())
        self.assertEqual(d["seite"]["name"], "seite.html")

    def test_fehlende_datei_bricht_den_lauf_ab(self):
        paket(self.o, ("ansicht_bg.wasm",))
        r = self.lauf("--seite-ordner", self.o)
        self.assertEqual(r.returncode, 3)
        self.assertIn("Seitenordner: Abbruch: Seitenordner unvollstaendig: ansicht_bg.wasm fehlt oder ist keine regulaere Datei", r.stderr)
        self.assertNotIn("Seite: seite.html", r.stdout)

    def test_echte_bruecke_nimmt_keinen_seitenordner(self):
        paket(self.o)
        r = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "echt", "--seite-ordner", self.o],
                           capture_output=True, text=True, timeout=30, cwd=self.w)
        self.assertEqual(r.returncode, 2)
        self.assertIn("seite_ordner gibt es nur gegen die Attrappe", r.stderr)


if __name__ == "__main__":
    unittest.main()
