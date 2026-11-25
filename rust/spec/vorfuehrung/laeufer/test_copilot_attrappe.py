"""Test der Attrappe der Copilot CLI (`attrappen/copilot_attrappe.py`): Sie startet gegen einen kleinen lokalen Proxy und ein Pruefziel mit
Test-CA, beides im Test erzeugt (Zertifikate mit dem Programm `openssl`). Pruefziel und Proxy hoeren nur auf 127.0.0.1.

Nur Python-Standardbibliothek. Aufruf: python3 -I -B test_copilot_attrappe.py
Die Attrappe laeuft an einem Pseudoterminal wie die echte CLI: Vertrauensfrage, Enter, Eingabefeld, Eingabe.
"""
import json
import os
import pty
import re
import select
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
ATTRAPPE = os.path.join(HIER, "attrappen", "copilot_attrappe.py")
PAKET_LEGEN = os.path.join(HIER, "attrappen", "paket_legen.py")
# Die vier Pfade des entpackten Pakets unter $HOME/.cache/copilot/pkg/linux-x64/0.0.88 (Form `paketform-copilot-1.0.88`, Punkt 5)
CACHE_REL = os.path.join(".cache", "copilot", "pkg", "linux-x64", "0.0.88")
VIER = ["index.js", "app.js", os.path.join("prebuilds", "linux-x64", "runtime.node"), ".extraction-complete"]
ENTPACKFEHLER = "Failed to extract bundled package\n"
PLATZHALTER = "gho_" + "0" * 36
HOST = "copilot-attrappe.test"
# argv nach dem Fall `aufruf-copilot-auto` (Vertrag 1) mit den drei erlaubten Werten aus `werkzeuge_ohne_rueckfrage`
ARGV = ["--model", "auto", "--auto-tier", "efficiency", "--disable-builtin-mcps", "--additional-mcp-config", "@<heim>/.copilot/wb-mcp.json",
        "--allow-tool", "shell", "--allow-tool", "write", "--allow-tool", "such-mcp"]


def openssl(*args, cwd):
    subprocess.run(["openssl"] + list(args), cwd=cwd, check=True, capture_output=True)


def zertifikate(ordner):
    """Test-CA und ein Zertifikat fuer `copilot-attrappe.test` (SAN)."""
    with open(os.path.join(ordner, "san.cnf"), "w") as f:
        f.write("[req]\ndistinguished_name=dn\n[dn]\n[ext]\nsubjectAltName=DNS:%s\nbasicConstraints=CA:FALSE\n" % HOST)
    openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "2", "-subj", "/CN=Test-CA", cwd=ordner)
    openssl("req", "-newkey", "rsa:2048", "-nodes", "-keyout", "ziel.key", "-out", "ziel.csr", "-subj", "/CN=" + HOST, cwd=ordner)
    openssl("x509", "-req", "-in", "ziel.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial", "-out", "ziel.pem", "-days", "2",
            "-extfile", "san.cnf", "-extensions", "ext", cwd=ordner)


class Pruefziel(threading.Thread):
    """TLS-Server mit der Test-CA. Speichert nur: ob der Kopf `Authorization` der Platzhalter ist (ja/nein) und wie oft ein Marker im Koerper steht."""

    def __init__(self, ordner, marker, offen=False):
        """`offen`: das Ziel antwortet ohne `Connection: close` und laesst die Verbindung nach der Antwort offen (keep-alive). Die Attrappe
        muss dann die Antwort als HTTP bis zum Kopf lesen, nicht bis zum Ende der Verbindung (Befund eines Testlaufs)."""
        super().__init__(daemon=True)
        self.offen = offen
        self.halten = []  # offene Verbindungen des keep-alive-Ziels; bleiben bis zum Ende des Tests offen
        self.ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.ctx.load_cert_chain(os.path.join(ordner, "ziel.pem"), os.path.join(ordner, "ziel.key"))
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(5)
        self.port = self.sock.getsockname()[1]
        self.marker = marker
        self.anfragen = []
        self.lock = threading.Lock()
        self.start()

    def run(self):
        while True:
            try:
                c, _ = self.sock.accept()
            except OSError:
                return
            try:
                s = self.ctx.wrap_socket(c, server_side=True)
                roh = b""
                while b"\r\n\r\n" not in roh:
                    roh += s.recv(4096)
                kopf, _, rest = roh.partition(b"\r\n\r\n")
                n = int(re.search(rb"(?i)content-length: (\d+)", kopf).group(1))
                while len(rest) < n:
                    rest += s.recv(4096)
                auth = re.search(rb"(?im)^authorization: (.*)\r?$", kopf)
                zeile = kopf.split(b"\r\n", 1)[0].decode("ascii", "replace")  # `POST <pfad> HTTP/1.1`
                try:
                    modell = json.loads(rest.decode("utf-8", "replace")).get("model")
                except (ValueError, AttributeError):
                    modell = None
                with self.lock:
                    self.anfragen.append({"pfad": zeile.split(" ")[1] if zeile.count(" ") >= 2 else None, "modell": modell,
                                          "platzhalter": bool(auth and auth.group(1).strip() == ("Bearer " + PLATZHALTER).encode()),
                                          "marker": rest.count(self.marker.encode())})
                if self.offen:
                    s.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                    self.halten.append(s)
                else:
                    s.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                    s.close()
            except (OSError, ssl.SSLError, AttributeError):
                c.close()

    def ende(self):
        self.sock.close()


class Proxy(threading.Thread):
    """Kleiner CONNECT-Proxy: nimmt jedes Ziel an und verbindet es mit dem Pruefziel. Merkt sich die Ziele."""

    def __init__(self, ziel_port):
        super().__init__(daemon=True)
        self.ziel_port = ziel_port
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(5)
        self.port = self.sock.getsockname()[1]
        self.ziele = []
        self.start()

    def _pumpe(self, a, b):
        try:
            while True:
                d = a.recv(4096)
                if not d:
                    break
                b.sendall(d)
        except OSError:
            pass
        finally:
            for x in (a, b):
                try:
                    x.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

    def run(self):
        while True:
            try:
                c, _ = self.sock.accept()
            except OSError:
                return
            try:
                kopf = b""
                while b"\r\n\r\n" not in kopf:
                    kopf += c.recv(4096)
                self.ziele.append(kopf.split(b" ")[1].decode())
                z = socket.create_connection(("127.0.0.1", self.ziel_port))
                c.sendall(b"HTTP/1.1 200 Connection established\r\n\r\n")
                threading.Thread(target=self._pumpe, args=(c, z), daemon=True).start()
                threading.Thread(target=self._pumpe, args=(z, c), daemon=True).start()
            except OSError:
                c.close()

    def ende(self):
        self.sock.close()


def fuelle_cache(heim):
    """Ein vollstaendig entpackter Cache-Ordner, wie ihn das Produkt vor dem Start vorbereitet."""
    for d in VIER:
        pfad = os.path.join(heim, CACHE_REL, d)
        os.makedirs(os.path.dirname(pfad), exist_ok=True)
        with open(pfad, "w") as f:
            f.write("x")


def baum(wurzel):
    """{relativer Pfad: (Art, Modus, Groesse, mtime_ns)} aller Eintraege unter `wurzel`, ohne Verweise zu verfolgen."""
    d = {}
    for ordner, namen, dateien in os.walk(wurzel):
        for n in namen + dateien:
            pfad = os.path.join(ordner, n)
            st = os.lstat(pfad)
            art = "l" if os.path.islink(pfad) else ("d" if os.path.isdir(pfad) else "f")
            d[os.path.relpath(pfad, wurzel)] = (art, st.st_mode & 0o777, st.st_size, st.st_mtime_ns)
    return d


class Terminal:
    """Die Attrappe an einem Pseudoterminal; `warte` liest, bis ein Muster im neuen Text steht."""

    def __init__(self, env, argv=None):
        self.master, slave = pty.openpty()
        self.p = subprocess.Popen([sys.executable, "-I", "-B", ATTRAPPE] + (ARGV if argv is None else argv), stdin=slave, stdout=slave, stderr=subprocess.DEVNULL, env=env,
                                  close_fds=True)
        os.close(slave)
        self.text = ""

    def warte(self, muster, frist_s=20.0, ab=0):
        ende = time.monotonic() + frist_s
        while time.monotonic() < ende:
            m = re.search(muster, self.text[ab:])
            if m:
                return m
            r, _, _ = select.select([self.master], [], [], 0.1)
            if r:
                try:
                    d = os.read(self.master, 4096)
                except OSError:
                    break
                if not d:
                    break
                self.text += d.decode("utf-8", "replace")
        raise AssertionError("Muster %r nicht gefunden; Ende: %r" % (muster, self.text[-300:]))

    def sende(self, text):
        """Schreibt in Stuecken und liest dazwischen das Echo, sonst fuellt sich der Puffer des Pseudoterminals (wie der Pty-Leser der Bruecke)."""
        roh = text.encode("utf-8")
        for i in range(0, len(roh), 128):
            os.write(self.master, roh[i:i + 128])
            while select.select([self.master], [], [], 0)[0]:
                d = os.read(self.master, 4096)
                if not d:
                    break
                self.text += d.decode("utf-8", "replace")

    def schliesse(self):
        try:
            os.close(self.master)
        except OSError:
            pass
        try:
            self.p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait(timeout=5)


class CopilotAttrappe(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ordner = tempfile.mkdtemp(prefix="copilot-att-")
        zertifikate(cls.ordner)
        cls.ziel = Pruefziel(cls.ordner, "MP-2C5", offen=True)  # keep-alive: die Attrappe liest die Antwort als HTTP, nicht bis zum Verbindungsende
        cls.proxy = Proxy(cls.ziel.port)

    @classmethod
    def tearDownClass(cls):
        cls.ziel.ende()
        cls.proxy.ende()
        shutil.rmtree(cls.ordner, ignore_errors=True)

    def setUp(self):
        self.heim = tempfile.mkdtemp(prefix="copilot-heim-", dir=self.ordner)
        self.proto = os.path.join(self.heim, "protokoll.json")
        self.env = {"TERM": "xterm-256color", "PATH": "/usr/bin:/bin", "HOME": self.heim, "COPILOT_GITHUB_TOKEN": PLATZHALTER,
                    "HTTPS_PROXY": "http://127.0.0.1:%d" % self.proxy.port, "COPILOT_AUTO_UPDATE": "false", "COPILOT_MASCOT_BLINK": "0",
                    "COPILOT_ATTRAPPE_PROTOKOLL": self.proto, "COPILOT_ATTRAPPE_CA": os.path.join(self.ordner, "ca.pem")}
        fuelle_cache(self.heim)
        self.t = None
        with self.ziel.lock:
            self.vorher = len(self.ziel.anfragen)

    def tearDown(self):
        if self.t:
            self.t.schliesse()

    def neue_anfragen(self):
        with self.ziel.lock:
            return self.ziel.anfragen[self.vorher:]

    def starte(self, env=None, argv=None):
        self.t = Terminal(env or self.env, argv)
        return self.t

    def eine_eingabe(self, argv):
        """Startet mit `argv`, bestaetigt die Vertrauensfrage, sendet eine Eingabe und gibt die eine neue Anfrage am Ziel."""
        t = self.starte(argv=argv)
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        t.sende("#T1 MP-2C5 ZAEHLEN\r")
        t.warte(r"FERTIG T1", ab=ab)
        a = self.neue_anfragen()
        self.assertEqual(len(a), 1, "genau eine Anfrage je Eingabe")
        return a[0]

    def test_modell_gpt_6_luna_geht_an_chat_completions_mit_modell_im_koerper(self):
        a = self.eine_eingabe(["--model", "gpt-6-luna"] + ARGV[2:])
        self.assertEqual(a["pfad"], "/chat/completions")
        self.assertEqual(a["modell"], "gpt-6-luna", "der Koerper traegt `model`")
        self.assertEqual(a["marker"], 1, "der Text der Eingabe steht in `messages`")
        self.assertTrue(a["platzhalter"])

    def test_modell_gpt_6_luna_in_der_form_mit_gleichheitszeichen(self):
        a = self.eine_eingabe(["--model=gpt-6-luna"] + ARGV[2:])
        self.assertEqual(a["pfad"], "/chat/completions")
        self.assertEqual(a["modell"], "gpt-6-luna")

    def test_anderes_modell_bleibt_bei_v1_messages_ohne_modell(self):
        for modell in ("auto", "gpt-6-luna-x", "gpt-5"):
            with self.subTest(modell=modell):
                a = self.eine_eingabe(["--model", modell] + ARGV[2:])
                self.assertEqual(a["pfad"], "/v1/messages")
                self.assertIsNone(a["modell"], "der alte Koerper hat kein `model`")
            self.t.schliesse()
            self.t = None
            with self.ziel.lock:
                self.vorher = len(self.ziel.anfragen)

    def test_vertrauensfrage_enter_eingabefeld_und_eine_anfrage_je_eingabe(self):
        t = self.starte()
        t.warte(r"Confirm folder trust")
        self.assertNotIn("/ commands", t.text, "das Eingabefeld erscheint erst nach Enter")
        t.sende("\r")
        t.warte(r"/ commands")
        self.assertIn("❯", t.text)
        self.assertIn("─" * 10, t.text)
        ab = len(t.text)
        t.sende("#T1 MP-2C5 ZAEHLEN\r")
        t.warte(r"FERTIG T1", ab=ab)
        a = self.neue_anfragen()
        self.assertEqual(len(a), 1, "genau eine Anfrage je Eingabe")
        self.assertEqual(a[0]["pfad"], "/v1/messages", "Modell `auto`: der alte Pfad")
        self.assertTrue(a[0]["platzhalter"], "der Kopf Authorization traegt den Platzhalter")
        self.assertEqual(a[0]["marker"], 1, "der Text der Eingabe steht im Koerper: MP-2C5 zaehlbar")
        self.assertEqual(self.proxy.ziele[-1], HOST + ":443", "ueber HTTPS_PROXY an das Ziel der Route")
        self.assertIn("HTTP 200", t.text)
        with open(self.proto, encoding="utf-8") as f:
            protokoll = json.load(f)
        self.assertEqual(protokoll["anfragen"][-1]["status"], 200, "das Protokoll traegt den Status der Antwort, nicht 0 (Punkt 3b)")

    def test_version_antwortet_sofort_ohne_stdin(self):
        """`copilot --version` gibt eine feste Zeile aus und endet mit 0, ohne stdin zu lesen (Punkt 3a). stdin ist ein offenes Rohr,
        das nie geschlossen wird: liest die Attrappe es, laeuft die Frist ab."""
        lesen, schreiben = os.pipe()
        try:
            t0 = time.monotonic()
            p = subprocess.run([sys.executable, "-I", "-B", ATTRAPPE, "--version"], stdin=lesen, capture_output=True, timeout=5,
                               env={"PATH": "/usr/bin:/bin", "HOME": self.heim})
        except subprocess.TimeoutExpired:
            self.fail("copilot --version liest stdin und wartet")
        finally:
            os.close(schreiben)
            os.close(lesen)
        self.assertEqual(p.returncode, 0)
        self.assertEqual(p.stdout.decode("utf-8"), "GitHub Copilot CLI 0.0.88 (Attrappe)\n")
        self.assertLess(time.monotonic() - t0, 5)

    def test_zweite_eingabe_fuehrt_das_buendel_aus(self):
        t = self.starte()
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        skript = 'z=$(echo 7); echo "a_b=$z RC=$?"; z=$(echo 0 | grep -c 1); echo "c=$z RC=$?"'
        t.sende("#T2 MP-2C5 BUENDEL %s\r" % skript)
        t.warte(r"FERTIG T2", ab=ab)
        self.assertRegex(t.text[ab:], r"(?m)^a_b=7 RC=0\s*$")
        self.assertRegex(t.text[ab:], r"(?m)^c=0 RC=1\s*$")
        self.assertEqual(len(self.neue_anfragen()), 1)

    def test_shell_eingabe_ohne_anfrage_an_das_ziel(self):
        """Zug 2 des knappen Laufs: `!<buendel>; echo "FERTIG T$((n))"` laeuft in der Shell, ohne Anfrage an das Ziel (kein Modell)."""
        t = self.starte()
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        skript = 'z=$(echo 7); echo "a_b=$z RC=$?"; z=$(echo 0 | grep -c 1); echo "c=$z RC=$?"'
        t.sende('!%s; echo "FERTIG T$((2))"\r' % skript)
        t.warte(r"FERTIG T2(?!\d)", ab=ab)
        self.assertRegex(t.text[ab:], r"(?m)^a_b=7 RC=0\s*$")
        self.assertRegex(t.text[ab:], r"(?m)^c=0 RC=1\s*$")
        self.assertEqual(len(self.neue_anfragen()), 0, "die Shell-Eingabe geht nicht an das Ziel")

    def test_eingabe_des_probelaufs_mit_skript_zwischen_gravis(self):
        t = self.starte()
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        t.sende("Kennung MP-2C5, Zug T3. Fuehre in der Shell genau einmal dieses Skript aus: `z=$(echo 5); echo \"x=$z RC=$?\"` Gib aus.\r")
        t.warte(r"FERTIG T3", ab=ab)
        self.assertRegex(t.text[ab:], r"(?m)^x=5 RC=0\s*$")
        t2 = self.neue_anfragen()
        self.assertEqual((len(t2), t2[0]["marker"]), (1, 1))

    def test_langes_buendel_ueber_ein_terminal(self):
        t = self.starte()
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        langer = "; ".join('z=$(echo %d); echo "n%d=$z RC=$?"' % (i, i) for i in range(60))
        self.assertGreater(len(langer), 2000)
        t.sende("#T4 MP-2C5 BUENDEL %s\r" % langer)
        t.warte(r"FERTIG T4", ab=ab)
        self.assertRegex(t.text[ab:], r"(?m)^n59=59 RC=0\s*$")

    def test_protokoll_hat_argv_und_namen_nie_werte(self):
        t = self.starte()
        t.warte(r"Confirm folder trust")
        with open(self.proto, encoding="utf-8") as f:
            roh = f.read()
        d = json.loads(roh)
        self.assertEqual(d["argv"], ["copilot"] + ARGV)
        vom_interpreter = {"LC_CTYPE", "__CF_USER_TEXT_ENCODING"}  # setzt der Interpreter unter macOS selbst
        self.assertEqual([n for n in d["env_namen"] if n not in vom_interpreter], sorted(self.env))
        self.assertEqual(d["token_format"], "platzhalter")
        for wert in (PLATZHALTER, self.heim, "127.0.0.1"):
            self.assertNotIn(wert, roh, "kein Wert der Umgebung in der Datei")
        self.assertEqual(oct(os.stat(self.proto).st_mode & 0o777), "0o600")

    def test_token_format(self):
        sys.path.insert(0, os.path.join(HIER, "attrappen"))
        import copilot_attrappe as c
        self.assertEqual([c.token_format(x) for x in (PLATZHALTER, "gho_" + "a" * 36, "ghp_" + "a" * 36, "gho_kurz", "", None)],
                         ["platzhalter", "gho", "falsch", "falsch", "fehlt", "fehlt"])

    def test_ohne_token_zeigt_sie_please_use_login(self):
        env = dict(self.env)
        del env["COPILOT_GITHUB_TOKEN"]
        t = self.starte(env)
        t.warte(r"Please use /login")
        self.assertNotIn("Confirm folder trust", t.text)
        self.assertEqual(json.load(open(self.proto))["token_format"], "fehlt")

    def test_ziel_nicht_erreichbar_ergibt_status_null_und_die_attrappe_laeuft_weiter(self):
        env = dict(self.env, HTTPS_PROXY="http://127.0.0.1:1")
        t = self.starte(env)
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        t.sende("#T1 MP-2C5 ZAEHLEN\r")
        t.warte(r"FERTIG T1", ab=ab)
        self.assertIn("HTTP 0", t.text)
        self.assertEqual(len(self.neue_anfragen()), 0)

    def test_falsche_ca_wird_nicht_angenommen(self):
        zweite = tempfile.mkdtemp(prefix="copilot-ca2-", dir=self.ordner)
        zertifikate(zweite)
        env = dict(self.env, COPILOT_ATTRAPPE_CA=os.path.join(zweite, "ca.pem"))
        t = self.starte(env)
        t.warte(r"Confirm folder trust")
        t.sende("\r")
        t.warte(r"/ commands")
        ab = len(t.text)
        t.sende("#T1 MP-2C5 ZAEHLEN\r")
        t.warte(r"FERTIG T1", ab=ab)
        self.assertIn("HTTP 0", t.text, "eine fremde CA nimmt die Attrappe nicht an")
        self.assertEqual(len(self.neue_anfragen()), 0)


class ZielZaehler(unittest.TestCase):
    """Das Pruefziel des Laeufers (`drehbuchlib/modell.py`, Zaehler `copilot.ziel_zaehler`) zaehlt den Chat-Pfad der Attrappe mit."""

    def post(self, port, pfad, koerper, schluessel):
        import http.client
        c = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
        try:
            c.request("POST", pfad, json.dumps(koerper).encode("utf-8"),
                      {"Content-Type": "application/json", "Authorization": "Bearer " + schluessel})
            r = c.getresponse()
            r.read()
            return r.status
        finally:
            c.close()

    def test_zaehler_am_ziel_zaehlt_den_chat_pfad(self):
        sys.path.insert(0, HIER)
        try:
            from drehbuchlib import modell
        finally:
            sys.path.pop(0)
        srv = modell.starte(modell.Mitschnitt(), "such-mcp-brain_suche", schluessel="S")
        try:
            kopf = {"model": "gpt-6-luna", "messages": [{"role": "user", "content": "x"}]}
            self.assertEqual(self.post(srv.server_address[1], "/chat/completions", kopf, "S"), 200)
            self.assertEqual(srv.autorisiert, 1, "der Chat-Pfad steigert den Zaehler")
            self.assertEqual(self.post(srv.server_address[1], "/v1/messages", {"messages": kopf["messages"]}, "S"), 200)
            self.assertEqual(srv.autorisiert, 2, "der alte Pfad auch")
            self.assertEqual(self.post(srv.server_address[1], "/chat/completions", kopf, "falsch"), 401)
            self.assertEqual(srv.autorisiert, 2, "ein falscher Schluessel zaehlt nicht")
        finally:
            srv.shutdown()
            srv.server_close()


class Paketform(unittest.TestCase):
    """Die Form des Pakets der echten CLI (`paketform-copilot-1.0.88`): Vorentpacken, Entpackfehler, Werkzeug `paket_legen.py`."""

    def setUp(self):
        self.heim = tempfile.mkdtemp(prefix="copilot-paket-")
        self.addCleanup(shutil.rmtree, self.heim, True)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": self.heim}
        self.prefix = os.path.join(self.heim, ".npm-global")

    def lauf(self, programm, *args, env=None):
        return subprocess.run(programm + list(args), stdin=subprocess.DEVNULL, capture_output=True, timeout=20, env=env or self.env)

    def attrappe(self, *args, env=None):
        return self.lauf([sys.executable, "-I", "-B", ATTRAPPE], *args, env=env)

    def dateien_unter(self, wurzel):
        return sorted(os.path.relpath(os.path.join(o, n), wurzel) for o, _, ns in os.walk(wurzel) for n in ns)

    def test_version_legt_genau_die_vier_pfade_an_und_sonst_nichts(self):
        p = self.attrappe("--version")
        self.assertEqual((p.returncode, p.stdout.decode(), p.stderr), (0, "GitHub Copilot CLI 0.0.88 (Attrappe)\n", b""))
        self.assertEqual(self.dateien_unter(self.heim), sorted(os.path.join(CACHE_REL, d) for d in VIER), "genau die vier Pfade, nichts sonst im Heim")
        zeiten = {d: os.stat(os.path.join(self.heim, CACHE_REL, d)).st_mtime_ns for d in VIER}
        self.assertEqual(zeiten[".extraction-complete"], max(zeiten.values()), ".extraction-complete zuletzt")

    def test_version_mit_vollstaendigem_ordner_schreibt_nichts(self):
        fuelle_cache(self.heim)
        vorher = baum(self.heim)
        p = self.attrappe("--version")
        self.assertEqual((p.returncode, p.stdout.decode()), (0, "GitHub Copilot CLI 0.0.88 (Attrappe)\n"))
        self.assertEqual(baum(self.heim), vorher, "Inhalt, Modi und Zeiten unveraendert")

    def test_version_legt_nur_die_fehlenden_an(self):
        fuelle_cache(self.heim)
        os.remove(os.path.join(self.heim, CACHE_REL, ".extraction-complete"))
        os.remove(os.path.join(self.heim, CACHE_REL, "app.js"))
        self.assertEqual(self.attrappe("--version").returncode, 0)
        self.assertEqual(self.dateien_unter(self.heim), sorted(os.path.join(CACHE_REL, d) for d in VIER))

    def test_start_ohne_den_ganzen_ordner_ist_ein_entpackfehler(self):
        p = self.attrappe("--model", "auto")
        self.assertEqual((p.returncode, p.stdout, p.stderr.decode()), (1, b"", ENTPACKFEHLER))
        self.assertEqual(os.listdir(self.heim), [], "keine Sitzung, kein Protokoll, nichts geschrieben")

    def test_start_mit_je_einem_fehlenden_pfad_ist_ein_entpackfehler(self):
        for fehlt in VIER:
            with self.subTest(fehlt=fehlt):
                shutil.rmtree(os.path.join(self.heim, ".cache"), ignore_errors=True)
                fuelle_cache(self.heim)
                os.remove(os.path.join(self.heim, CACHE_REL, fehlt))
                vorher = baum(self.heim)
                proto = os.path.join(self.heim, "protokoll.json")
                p = self.attrappe("--model", "auto", env=dict(self.env, COPILOT_GITHUB_TOKEN="gho_" + "0" * 36, COPILOT_ATTRAPPE_PROTOKOLL=proto))
                self.assertEqual((p.returncode, p.stdout, p.stderr.decode()), (1, b"", ENTPACKFEHLER))
                self.assertEqual(baum(self.heim), vorher, "nichts geschrieben, auch kein Protokoll")

    def test_start_mit_vollstaendigem_ordner_ist_der_alte_start(self):
        fuelle_cache(self.heim)
        p = self.attrappe("--model", "auto")  # ohne Token: die Anmeldeaufforderung und Exit 1 am Ende der Eingabe, wie vor der Paketform
        self.assertIn("Please use /login", p.stdout.decode())
        self.assertEqual(p.returncode, 1)
        self.assertEqual(p.stderr, b"")

    def test_fassung_ist_ueberall_0_0_88(self):
        sys.path.insert(0, os.path.join(HIER, "attrappen"))
        try:
            import copilot_attrappe as c
            import paket_legen as w
        finally:
            sys.path.pop(0)
        self.assertEqual((c.PAKET_FASSUNG, w.FASSUNG), ("0.0.88", "0.0.88"))
        self.assertEqual(os.path.relpath(c.cache_ordner({"HOME": "H"}), "H"), CACHE_REL)

    def lege(self, prefix=None):
        p = subprocess.run([sys.executable, "-I", "-B", PAKET_LEGEN, prefix or self.prefix], capture_output=True, timeout=20)
        return p

    def test_werkzeug_legt_die_form_an_ohne_verweise(self):
        p = self.lege()
        self.assertEqual((p.returncode, p.stderr), (0, b""))
        haupt = os.path.join(self.prefix, "lib", "node_modules", "@github", "copilot")
        plattform = os.path.join(haupt, "node_modules", "@github", "copilot-linux-x64")
        self.assertEqual(self.dateien_unter(self.prefix), sorted(os.path.relpath(os.path.join(o, n), self.prefix) for o, n in
                         ((haupt, "package.json"), (haupt, "npm-loader.js"), (plattform, "package.json"), (plattform, "copilot"))))
        b = baum(self.prefix)
        self.assertEqual([k for k, v in b.items() if v[0] == "l"], [], "keine Verweise")
        self.assertEqual({k: v[1] for k, v in b.items() if v[0] == "d"}, {k: 0o755 for k, v in b.items() if v[0] == "d"}, "Ordner 0755")
        modi = {os.path.basename(k): v[1] for k, v in b.items() if v[0] == "f" and not k.endswith("package.json")}
        self.assertEqual(modi, {"npm-loader.js": 0o755, "copilot": 0o755})
        self.assertEqual({v[1] for k, v in b.items() if k.endswith("package.json")}, {0o644})
        with open(os.path.join(haupt, "package.json"), encoding="utf-8") as f:
            hp = json.load(f)
        self.assertEqual((hp["name"], hp["version"], hp["bin"], hp["type"]), ("@github/copilot", "0.0.88", {"copilot": "npm-loader.js"}, "module"))
        self.assertEqual(hp["optionalDependencies"]["@github/copilot-linux-x64"], "0.0.88")
        with open(os.path.join(plattform, "package.json"), encoding="utf-8") as f:
            pp = json.load(f)
        self.assertEqual(pp, {"name": "@github/copilot-linux-x64", "version": "0.0.88", "type": "module", "os": ["linux"], "cpu": ["x64"],
                              "libc": ["glibc"], "bin": {"copilot-linux-x64": "copilot"}, "exports": {".": "./copilot"}})
        with open(os.path.join(haupt, "npm-loader.js"), encoding="utf-8") as f:
            self.assertEqual(f.readline(), "#!/usr/bin/env python3\n")
        with open(os.path.join(plattform, "copilot"), "rb") as a, open(ATTRAPPE, "rb") as b2:
            self.assertEqual(a.read(), b2.read(), "das Programm ist die Attrappe, eine gewoehnliche Datei")

    def test_aus_der_form_startet_das_produkt_wie_es_die_cli_startet(self):
        """`<prefix>/bin/copilot` (Verweis, hier vom Test angelegt) per exec: Vorentpacken mit `--version`, danach der Start."""
        self.assertEqual(self.lege().returncode, 0)
        os.makedirs(os.path.join(self.prefix, "bin"))
        os.symlink(os.path.join("..", "lib", "node_modules", "@github", "copilot", "npm-loader.js"), os.path.join(self.prefix, "bin", "copilot"))
        cli = os.path.join(self.prefix, "bin", "copilot")
        p = self.lauf([cli], "--version")
        self.assertEqual((p.returncode, p.stdout.decode(), p.stderr), (0, "GitHub Copilot CLI 0.0.88 (Attrappe)\n", b""))
        self.assertEqual(self.dateien_unter(os.path.join(self.heim, ".cache")), sorted(os.path.join("copilot", "pkg", "linux-x64", "0.0.88", d) for d in VIER))
        p = self.lauf([cli], "--model", "auto")
        self.assertIn("Please use /login", p.stdout.decode())
        shutil.rmtree(os.path.join(self.heim, ".cache"))
        p = self.lauf([cli], "--model", "auto")
        self.assertEqual((p.returncode, p.stdout, p.stderr.decode()), (1, b"", ENTPACKFEHLER), "ohne Vorentpacken kein Start")

    def test_werkzeug_schreibt_nicht_durch_einen_verweis(self):
        fremd = tempfile.mkdtemp(prefix="copilot-fremd-", dir=self.heim)
        os.makedirs(self.prefix)
        os.symlink(fremd, os.path.join(self.prefix, "lib"))
        p = self.lege()
        self.assertEqual(p.returncode, 2)
        self.assertEqual(os.listdir(fremd), [], "hinter dem Verweis wurde nichts angelegt")


if __name__ == "__main__":
    unittest.main()
