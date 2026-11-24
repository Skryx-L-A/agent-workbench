"""Test des Weiterreichers (`drehbuchlib/weiterreicher.py`): Die Zaehldatei entsteht nur unter dem Zaehlordner, und der
Content-Type der Antwort kommt nur aus einer festen Liste.

Angriffe, die abgewiesen werden: `X-Sitzung` mit `../`, mit absolutem Pfad, ein Link an der Stelle der Zaehldatei oder ihrer
Zwischendatei `.neu`, und ein Content-Type des Gateways mit CR/LF (gefaltete Kopfzeile). Nur Python-Standardbibliothek, alles auf
127.0.0.1. Aufruf: python3 -I -B test_weiterreicher.py
"""
import http.client
import json
import os
import shutil
import socket
import sys
import tempfile
import threading
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)

from drehbuchlib import weiterreicher  # noqa: E402

MARKER = ["MP-2C5"]


class Gateway:
    """Kleines Gateway: antwortet auf jede Anfrage mit den Kopfzeilen `kopf` (rohe Bytes) und dem Koerper `{}`."""

    def __init__(self, kopf):
        self.kopf = kopf
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(8)
        self.adresse = self.sock.getsockname()
        self.faden = threading.Thread(target=self._lauf, daemon=True)
        self.faden.start()

    def _lauf(self):
        while True:
            try:
                c, _ = self.sock.accept()
            except OSError:
                return
            with c:
                daten = b""
                while b"\r\n\r\n" not in daten:
                    stueck = c.recv(4096)
                    if not stueck:
                        break
                    daten += stueck
                kopf, _, rest = daten.partition(b"\r\n\r\n")
                laenge = 0
                for z in kopf.split(b"\r\n"):
                    if z.lower().startswith(b"content-length:"):
                        laenge = int(z.split(b":", 1)[1])
                while len(rest) < laenge:
                    stueck = c.recv(4096)
                    if not stueck:
                        break
                    rest += stueck
                c.sendall(b"HTTP/1.1 200 OK\r\n" + self.kopf + b"Content-Length: 2\r\nConnection: close\r\n\r\n{}")

    def schliessen(self):
        self.sock.close()


class Weiterreicher(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="wr-")
        self.wurzel = os.path.join(self.tmp, "zaehl")
        os.mkdir(self.wurzel)
        self.draussen = os.path.join(self.tmp, "draussen")
        os.mkdir(self.draussen)
        self.gw = None
        self.srv = None

    def tearDown(self):
        if self.srv:
            self.srv.shutdown()
            self.srv.server_close()
        if self.gw:
            self.gw.schliessen()
        shutil.rmtree(self.tmp)

    def starten(self, kopf=b"Content-Type: application/json\r\n"):
        self.gw = Gateway(kopf)
        self.srv = weiterreicher.starte(self.wurzel, MARKER, self.gw.adresse, "gw-test")

    def anfrage(self, sitzung):
        c = http.client.HTTPConnection("127.0.0.1", self.srv.server_address[1], timeout=10)
        c.request("POST", "/v1/chat/completions", b'{"x":"MP-2C5"}', {"X-Sitzung": sitzung, "Content-Type": "application/json"})
        r = c.getresponse()
        roh = r.read()
        kopf = r.getheaders()
        c.close()
        return r.status, kopf, roh

    def alle_dateien(self):
        aus = []
        for wurzel, _, namen in os.walk(self.tmp):
            aus += [os.path.relpath(os.path.join(wurzel, n), self.tmp) for n in namen]
        return sorted(aus)

    def test_gueltige_sitzung_zaehlt(self):
        self.starten()
        status, _, _ = self.anfrage("s-1")
        self.assertEqual(status, 200)
        with open(os.path.join(self.wurzel, "s-1.modellweg.json"), encoding="utf-8") as f:
            z = json.load(f)
        self.assertEqual(z, {"anfragen": 1, "marker": {"MP-2C5": 1}, "sitzung": "s-1"})
        self.assertEqual(self.alle_dateien(), [os.path.join("zaehl", "s-1.modellweg.json")])

    def test_punktpunkt_abgewiesen(self):
        self.starten()
        for s in ("../draussen/boese", "..", "../boese", "a/../../boese", ".versteckt", "s-1/x"):
            status, _, _ = self.anfrage(s)
            self.assertEqual(status, 200, s)  # weitergereicht, aber nicht gezaehlt
        self.assertEqual(self.alle_dateien(), [])

    def test_absoluter_pfad_abgewiesen(self):
        self.starten()
        status, _, _ = self.anfrage(os.path.join(self.draussen, "boese"))
        self.assertEqual(status, 200)
        self.assertEqual(self.alle_dateien(), [])
        self.assertIsNone(weiterreicher.zaehlpfad(self.wurzel, os.path.join(self.draussen, "boese")))

    def test_link_an_der_zaehldatei_abgewiesen(self):
        ziel = os.path.join(self.draussen, "ziel")
        with open(ziel, "w") as f:
            f.write("unberuehrt")
        os.symlink(ziel, os.path.join(self.wurzel, "s-1.modellweg.json"))
        os.symlink(ziel, os.path.join(self.wurzel, "s-2.modellweg.json.neu"))
        self.starten()
        for s in ("s-1", "s-2"):
            status, _, _ = self.anfrage(s)
            self.assertEqual(status, 200, s)
        with open(ziel) as f:
            self.assertEqual(f.read(), "unberuehrt")
        self.assertTrue(os.path.islink(os.path.join(self.wurzel, "s-1.modellweg.json")))
        self.assertFalse(os.path.exists(os.path.join(self.wurzel, "s-2.modellweg.json")))

    def test_zaehlpfad_bleibt_in_der_wurzel(self):
        p = weiterreicher.zaehlpfad(self.wurzel, "s-7")
        self.assertEqual(p, os.path.join(os.path.realpath(self.wurzel), "s-7.modellweg.json"))
        for s in ("", "/", "..", "../x", "x/y", "x\\y", "s-1\r\nX: y", "s\x00", "ä", None, 5, "a" * 200):
            self.assertIsNone(weiterreicher.zaehlpfad(self.wurzel, s), repr(s))

    def test_content_type_mit_crlf_abgewiesen(self):
        # Gefaltete Kopfzeile: http.client liefert den Wert mit CR/LF; der Weiterreicher darf ihn nicht weitergeben
        self.starten(b"Content-Type: text/html\r\n X-Boese: 1\r\n")
        status, kopf, roh = self.anfrage("s-1")
        self.assertEqual(status, 200)
        self.assertEqual(roh, b"{}")
        namen = [k.lower() for k, _ in kopf]
        self.assertNotIn("x-boese", namen)
        self.assertEqual(dict((k.lower(), v) for k, v in kopf)["content-type"], "application/json")

    def test_content_type_aus_der_liste(self):
        self.starten(b"Content-Type: text/event-stream\r\n")
        _, kopf, _ = self.anfrage("s-1")
        self.assertEqual(dict((k.lower(), v) for k, v in kopf)["content-type"], "text/event-stream")
        self.assertEqual(weiterreicher.antworttyp("text/html"), "application/json")
        self.assertEqual(weiterreicher.antworttyp("application/json\r\nSet-Cookie: a=b"), "application/json")


if __name__ == "__main__":
    unittest.main()
