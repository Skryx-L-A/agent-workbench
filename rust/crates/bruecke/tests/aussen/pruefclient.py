"""Prüfclient of the `bruecke` contract (section 6 of the contract text). Standard library only.

  BR_BIN=<bridge program> python3 -I -B pruefclient.py <fall.json>

Reads one case. Starts the counterpart (the reference of the service `dienst` with `lauf`, a fixed child and the test mode), writes the Startdatei of
the bridge, starts BR_BIN, speaks HTTP/1.1 to 127.0.0.1 over raw sockets (also malformed ones) and reads the event streams. Writes ONE JSON object on
stdout (exit 0): {"start": {...}, "ergebnisse": [...], "ende": {...}}. Exit 2: the case is invalid. Exit 3: the environment is wrong.

Environment: BR_BIN (required), BR_DIENST (the service program, default referenz/dienst_ref.py), BR_REF_DIR (folder with ra_ref.py, default referenz/),
BR_EINST (the settings file the service reads, default einstellungen/vorfuehrung.json), BR_TMP (base folder for the throw-away folders, default /tmp).
"""
from __future__ import annotations

import base64
import json
import os
import re
import selectors
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import time

HIER = os.path.dirname(os.path.abspath(__file__))
MUT = os.environ.pop("REF_GEGENPROBE", "")
REFDIR = os.environ.get("BR_REF_DIR") or os.path.join(HIER, "referenz")
sys.path.insert(0, REFDIR)
import ra_ref  # noqa: E402

FRIST = 20.0
ENV = {"TERM": "xterm-256color", "LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin"}



# Nachtrag vom 21.11.2026: Ein wasm-Modul, das SVG zeichnet, traegt den Namensraum von SVG als Zeichenkette. Ein Namensraum von XML
# ist ein Name, kein Abruf; er zaehlt nicht als Adresse. Jedes andere "://" ist eine.
NAMENSRAEUME = (b"http://www.w3.org/2000/svg", b"http://www.w3.org/1999/xlink", b"http://www.w3.org/1998/Math/MathML",
                b"http://www.w3.org/1999/xhtml", b"http://www.w3.org/XML/1998/namespace", b"http://www.w3.org/2000/xmlns/")


def ohne_adresse(koerper):
    """True, wenn jedes "://" im Koerper zu einem Namensraum von XML gehoert (NAMENSRAEUME)."""
    i = koerper.find(b"://")
    while i >= 0:
        if not any(koerper[i - 4:i - 4 + len(n)] == n for n in NAMENSRAEUME):
            return False
        i = koerper.find(b"://", i + 3)
    return True

class Ungueltig(Exception):
    pass


class Umgebung(Exception):
    pass


# --- Kernabbild (Schritt 2a): a program that shields its memory from core images (`PR_SET_DUMPABLE 0`) is invisible to `lsof` of the same user
# (measured on Linux: 0 lines, `/proc/<pid>/fd` is "Permission denied"). So the program runs in its own user namespace without capabilities
# (`unshare -Ur`, then `setpriv` drops them): the observer outside, same user, owns that namespace and sees it (measured: 1 line). The
# program starts as the same process (no fork: `unshare` and `setpriv` exec), so `proc.pid` stays its pid. Other systems have no
# `PR_SET_DUMPABLE`: no prefix. `BR_NS=aus` (a counter-check of the tests, never a setting of the product) starts it without the
# namespace: the program is then blind for `lsof` and the positive control below must end the run.
NS_PRAEFIX = ["unshare", "-Ur", "--", "setpriv", "--bounding-set=-all", "--inh-caps=-all", "--"]
_NS_GEPRUEFT = []


def beobacht_praefix():
    """The command prefix under which the program under test starts ([] where nothing is needed)."""
    if sys.platform != "linux" or os.environ.get("BR_NS") == "aus":
        return []
    if not _NS_GEPRUEFT:
        try:
            r = subprocess.run(NS_PRAEFIX + ["true"], capture_output=True, timeout=20)
        except (OSError, subprocess.SubprocessError) as e:
            raise Umgebung("Nutzer-Namensraum nicht benutzbar (unshare oder setpriv fehlt): %s" % e)
        if r.returncode != 0:
            raise Umgebung("Nutzer-Namensraum nicht erlaubt (unshare -Ur): %s" % r.stderr.decode("utf-8", "replace").strip()[:200])
        _NS_GEPRUEFT.append(True)
    return NS_PRAEFIX


def lsof_zeilen(pid, *args):
    """The lines of `lsof -nP -a -p <pid> <args>` without the head line."""
    try:
        r = subprocess.run(["lsof", "-nP", "-a", "-p", str(pid), *args], capture_output=True, timeout=20)
    except FileNotFoundError:
        raise Umgebung("lsof fehlt")
    return [z for z in r.stdout.decode("utf-8", "replace").split("\n")[1:] if z.strip()]


def rahmen(art, nutz):
    return struct.pack(">IB", 1 + len(nutz), art) + nutz


def jb(o):
    return json.dumps(o, ensure_ascii=False, sort_keys=True).encode("utf-8")


class Strom:
    def __init__(self, sock):
        self.sock = sock
        self.buf = bytearray()
        self.ver = ra_ref.Verbindung(False)
        self.getrennt = False
        self.status = None
        self.kennung = None
        self.kopf_fertig = False
        self.roh = bytearray()
        self.nachrichten, self.ablehnungen, self.fehler, self.abbild_fehler = [], [], [], []
        self.liste = None
        self.rahmen_ok = True
        self.deflate = None

    def abbilder(self):
        return {str(nr): {"groesse": [ab.spalten, ab.zeilen], "cursor": list(ab.cursor),
                          "zeilen": [ra_ref.zeile_schreiben(z) for z in ab.lines]} for nr, ab in sorted(self.ver.empfang.items())}

    def text(self, nr=None):
        for n, ab in sorted(self.ver.empfang.items()):
            if nr is None or n == nr:
                return ["".join(l[3] for l in z) for z in ab.lines]
        return []


class Lauf:
    def __init__(self, fall):
        self.fall = fall
        self.base = tempfile.mkdtemp(prefix="br-", dir=os.environ.get("BR_TMP") or "/tmp")
        self.ds = os.path.join(self.base, "ds")
        self.tk = os.path.join(self.base, "tk")
        self.proc = None
        self.dproc = None
        self.port = 0
        self.token = ""
        self.strome = {}
        self.sel = selectors.DefaultSelector()
        self.gehalten = {}
        self.monitore = 0
        self.laufkind = None
        self.belegt = None
        self.dienst_aktiv = fall.get("dienst", {}) is not None

    # ---------- placeholders
    def laufkind_datei(self):
        if self.laufkind is None:
            self.laufkind = os.path.join(self.base, "laufkind")
            dienst = os.path.abspath(os.environ.get("BR_DIENST") or os.path.join(REFDIR, "dienst_ref.py"))
            with open(self.laufkind, "w") as h:
                h.write("#!/usr/bin/python3 -I\nimport os, sys\nos.execv(%r, [%r, \"--kind\", \"lauf\"] + sys.argv)\n" % (dienst, dienst))
            os.chmod(self.laufkind, 0o755)
        return self.laufkind

    def ersetze(self, x):
        if isinstance(x, str):
            if x == "@dienst_socket":
                return os.path.join(self.ds, "dienst.sock")
            if x == "@token_datei":
                return os.path.join(self.tk, "token")
            if x == "@port_datei":
                return os.path.join(self.tk, "port")
            if x == "@port_belegt":
                if self.belegt is None:
                    self.belegt = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                    self.belegt.bind(("127.0.0.1", 0))
                    self.belegt.listen(1)
                return self.belegt.getsockname()[1]
            return x
        if isinstance(x, list):
            return [self.ersetze(v) for v in x]
        if isinstance(x, dict):
            return {k: self.ersetze(v) for k, v in x.items()}
        return x

    # ---------- the service (counterpart)
    def dienst_starten(self):
        d = self.fall.get("dienst") or {}
        os.mkdir(self.ds, 0o700)
        plan = d.get("plan", {"*": {"modus": "marke", "text": "X", "sek": 30}})
        einst = os.environ.get("BR_EINST") or os.path.join(HIER, "einstellungen", "vorfuehrung.json")
        pm = d.get("pruefmodus", True)
        st = {"socket": {"ordner": self.ds}, "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "browser"},
              "lauf": {"programm": self.laufkind_datei(), "env": dict(ENV, KIND_PLAN=json.dumps(plan, sort_keys=True)), "groesse": d.get("groesse", [100, 12]),
                       "einstellungen": einst},
              "deflate": bool(d.get("deflate", False)), "pruefmodus": pm, "anfang": {"personen": {"anna": {"rolle": "nutzer", "art": "mensch"}}}}
        f = os.path.join(self.base, "dienst.json")
        with open(f, "w") as h:
            json.dump(st, h)
        dienst = os.environ.get("BR_DIENST") or os.path.join(REFDIR, "dienst_ref.py")
        self.dlog = open(os.path.join(self.base, "dienst.err"), "wb")
        self.dproc = subprocess.Popen([dienst, f], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=self.dlog, cwd=self.base)
        t0 = time.monotonic()
        while time.monotonic() - t0 < 10:
            try:
                if stat.S_ISSOCK(os.stat(os.path.join(self.ds, "dienst.sock")).st_mode):
                    return
            except OSError:
                pass
            if self.dproc.poll() is not None:
                raise Umgebung("der Dienst startet nicht")
            time.sleep(0.02)
        raise Umgebung("der Dienst startet nicht (Zeit)")

    def monitor(self, warte_betrachter=None):
        """A fresh connection to the service asks for its state (test mode). The name of the connection counts the connections of the service."""
        t0 = time.monotonic()
        while True:
            sk = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sk.settimeout(10)
            sk.connect(os.path.join(self.ds, "dienst.sock"))
            self.monitore += 1
            sk.sendall(rahmen(0x12, b""))
            buf = b""
            antwort = None
            while antwort is None:
                x = sk.recv(65536)
                if not x:
                    break
                buf += x
                while len(buf) >= 4 and len(buf) >= 4 + struct.unpack(">I", buf[:4])[0]:
                    n = struct.unpack(">I", buf[:4])[0]
                    art, nl = buf[4], buf[5:4 + n]
                    buf = buf[4 + n:]
                    if art == 0x12:
                        antwort = json.loads(nl.decode("utf-8"))
            sk.close()
            if antwort is None:
                raise Umgebung("der Dienst antwortet nicht auf Zustand")
            namen = sorted(antwort["kern"]["betrachter"], key=lambda n: int(n[1:]))
            mich = namen[-1]
            erg = {"verbindungen": int(mich[1:]) - self.monitore, "betrachter": len(namen) - 1, "sitzungen": [x["id"] for x in antwort["kern"]["sitzungen"]]}
            if warte_betrachter is None or erg["betrachter"] == warte_betrachter or time.monotonic() - t0 > 10:
                return erg
            time.sleep(0.05)

    # ---------- the bridge
    def ordner_token(self):
        sp = self.fall.get("ordner", {"art": "verzeichnis", "modus": "0700"})
        art = sp.get("art", "verzeichnis")
        if art == "verzeichnis":
            os.mkdir(self.tk)
            v = sp.get("token_vorher")
            if v == "datei":
                open(os.path.join(self.tk, "token"), "w").close()
            elif v == "link":
                os.symlink("/nonexistent-br", os.path.join(self.tk, "token"))
            if sp.get("port_vorher") == "datei":
                open(os.path.join(self.tk, "port"), "w").close()
            os.chmod(self.tk, int(sp.get("modus", "0700"), 8))
        elif art == "link":
            os.mkdir(os.path.join(self.base, "ziel"), 0o700)
            os.symlink(os.path.join(self.base, "ziel"), self.tk)
        elif art == "datei":
            open(self.tk, "w").close()
        elif art != "fehlt":
            raise Ungueltig("ordner.art")

    def bruecke_starten(self):
        self.ordner_token()
        start = self.ersetze(self.fall["start"]) if "start" in self.fall else None
        f = os.path.join(self.base, "start.json")
        with open(f, "wb") as h:
            h.write(self.fall["start_roh"].encode("utf-8") if "start_roh" in self.fall else json.dumps(start, ensure_ascii=False).encode("utf-8"))
        env = dict(os.environ)
        if MUT:
            env["REF_GEGENPROBE"] = MUT
        self.so = open(os.path.join(self.base, "stdout"), "wb")
        self.se = open(os.path.join(self.base, "stderr"), "wb")
        self.proc = subprocess.Popen([*beobacht_praefix(), os.environ["BR_BIN"], f], stdin=subprocess.DEVNULL, stdout=self.so, stderr=self.se, env=env, cwd=self.base)
        t0 = time.monotonic()
        while time.monotonic() - t0 < 10:
            rc = self.proc.poll()
            ausgabe = self.lies_datei("stdout")
            if rc is not None or b"\n" in ausgabe:
                break
            time.sleep(0.02)
        else:
            return {"ok": False, "fehler": "zeit"}
        rc = self.proc.poll()
        out, err = self.lies_datei("stdout"), self.lies_datei("stderr")
        if rc is not None:
            zeilen = [z for z in err.decode("utf-8", "replace").split("\n") if z.strip()]
            return {"ok": False, "exitcode": rc, "stderr_zeilen": len(zeilen), "stderr_beginn": bool(zeilen) and zeilen[0].startswith("wb-bruecke: "),
                    "stdout_leer": not out.strip(), "token_da": os.path.lexists(os.path.join(self.tk, "token"))}
        zeilen = out.decode("utf-8", "replace").split("\n")
        mo = re.fullmatch(r"PORT ([0-9]+)", zeilen[0])
        if not mo:
            return {"ok": False, "fehler": "port_zeile", "stdout_zeilen": len([z for z in zeilen if z])}
        self.port = int(mo.group(1))
        tdat = os.path.join(self.tk, "token")
        try:
            roh = open(tdat, "rb").read()
            modus = "%04o" % (os.stat(tdat).st_mode & 0o7777)
        except OSError:
            roh, modus = b"", None
        self.token = roh.decode("ascii", "replace").strip()
        pdat = None
        if start and start.get("port_datei"):
            try:
                pr = open(start["port_datei"], "rb").read()
                pdat = {"modus": "%04o" % (os.stat(start["port_datei"]).st_mode & 0o7777), "inhalt_stimmt": pr == ("%d\n" % self.port).encode()}
            except OSError:
                pdat = {"modus": None, "inhalt_stimmt": False}
        erg = {"ok": True, "port_zeile": True, "stdout_zeilen": len([z for z in zeilen if z]), "token_datei": {
                    "modus": modus, "form": bool(re.fullmatch(r"[0-9a-f]{64}\n", roh.decode("ascii", "replace"))),
                    "verschieden": len(set(self.token)) >= 8}, "ordner_modus": "%04o" % (os.stat(self.tk).st_mode & 0o7777), "hoerer": self.hoerer(),
                "stderr_leer": not self.lies_datei("stderr").strip()}
        if pdat is not None:
            erg["port_datei"] = pdat
        return erg

    def lies_datei(self, name):
        try:
            with open(os.path.join(self.base, name), "rb") as h:
                return h.read()
        except OSError:
            return b""

    def hoerer(self):
        # positive control: a bridge always listens on its port; if lsof shows no listening TCP socket at all, it does not see the program
        # (a program that is not observable shows nothing), and an empty answer would not be a statement about the listener
        try:
            r = subprocess.run(["lsof", "-nP", "-a", "-p", str(self.proc.pid), "-iTCP", "-sTCP:LISTEN", "-Fn"], capture_output=True, timeout=20)
        except FileNotFoundError:
            raise Umgebung("lsof fehlt")
        adr = set()
        for z in r.stdout.decode("utf-8", "replace").split("\n"):
            if z.startswith("n"):
                a, _, p = z[1:].rpartition(":")
                adr.add(a if p == str(self.port) else a + "!port")
        if not adr:
            raise Umgebung("Positivkontrolle: lsof sieht keinen hoerenden TCP-Socket der Bruecke (Port %s); die Bruecke waere unsichtbar" % self.port)
        return sorted(adr)

    # ---------- HTTP
    def roh_anfrage(self, a):
        if "roh" in a:
            t = a["roh"]
            return self.platzhalter(t).encode("latin-1")
        methode, ziel = a.get("methode", "GET"), a.get("ziel", "/")
        zeilen = ["%s %s %s" % (methode, ziel, a.get("version", "HTTP/1.1"))]
        host = a.get("host", "gut")
        if host == "gut":
            zeilen.append("Host: 127.0.0.1:%d" % self.port)
        elif host is not None:
            zeilen.append("Host: " + self.platzhalter(host))
        o = a.get("origin")
        if o == "gut":
            zeilen.append("Origin: http://127.0.0.1:%d" % self.port)
        elif o == "gut_localhost":
            zeilen.append("Origin: http://localhost:%d" % self.port)
        elif o is not None:
            zeilen.append("Origin: " + self.platzhalter(o))
        t = a.get("token")
        if t == "gut":
            zeilen.append("Authorization: Bearer " + self.token)
        elif t is not None:
            zeilen.append("Authorization: " + self.platzhalter(t))
        koerper = a.get("koerper", "")
        if isinstance(koerper, (dict, list)):
            koerper = json.dumps(koerper, ensure_ascii=False)
        if "koerper_laenge" in a:
            koerper = koerper + " " * (a["koerper_laenge"] - len(koerper.encode("utf-8")))
        kb = koerper.encode("utf-8")
        if methode == "POST" or "inhalt_typ" in a:
            it = a.get("inhalt_typ", "application/json")
            if it is not None:
                zeilen.append("Content-Type: " + it)
        if a.get("verbindung"):
            zeilen.append("X-Verbindung: " + self.strome[a["verbindung"]].kennung)
        elif a.get("kennung"):
            zeilen.append("X-Verbindung: " + a["kennung"])
        cl = a.get("content_laenge", len(kb) if (methode == "POST" or kb) else None)
        if cl is not None:
            zeilen.append("Content-Length: %s" % cl)
        for n, w in a.get("koepfe", []):
            zeilen.append("%s: %s" % (n, self.platzhalter(w)))
        return ("\r\n".join(zeilen) + "\r\n\r\n").encode("latin-1") + kb

    def platzhalter(self, t):
        t = t.replace("@token_falsch", self.token[:-1] + ("0" if self.token[-1:] != "0" else "1"))
        t = t.replace("@token_anders_erst", ("0" if self.token[:1] != "0" else "1") + self.token[1:])
        t = t.replace("@port", str(self.port)).replace("@token", self.token)
        for n, s in self.strome.items():
            t = t.replace("@verbindung:" + n, s.kennung or "")
        return t

    def antwort_lesen(self, sock, frist):
        buf = b""
        t_ende = time.monotonic() + frist
        sock.settimeout(0.2)
        while True:
            try:
                d = sock.recv(65536)
            except socket.timeout:
                if time.monotonic() > t_ende:
                    break
                continue
            except OSError:
                break
            if not d:
                break
            buf += d
        return buf

    def auswerten(self, buf):
        if not buf:
            return {"status": None, "zu": True}
        kopf, _, koerper = buf.partition(b"\r\n\r\n")
        zeilen = kopf.decode("latin-1").split("\r\n")
        mo = re.match(r"HTTP/1\.1 ([0-9]{3}) ", zeilen[0])
        if not mo:
            return {"status": "?", "roh": buf[:60].decode("latin-1")}
        h = {}
        for z in zeilen[1:]:
            n, _, w = z.partition(":")
            h[n.strip().lower()] = w.strip()
        st = int(mo.group(1))
        erg = {"status": st, "cors": any(k.startswith("access-control-") for k in h), "cookie": "set-cookie" in h, "close": h.get("connection") == "close",
               "nosniff": h.get("x-content-type-options") == "nosniff", "no_store": h.get("cache-control") == "no-store",
               "laenge_stimmt": ("content-length" not in h) or int(h["content-length"]) == len(koerper)}
        if "content-type" in h:
            erg["typ"] = h["content-type"]
        if "content-security-policy" in h:
            erg["csp"] = self.csp_pruefen(h["content-security-policy"])
        if st >= 400 or st == 204:
            erg["koerper_laenge"] = len(koerper)
        elif h.get("content-type", "").startswith("application/json"):
            try:
                erg["json"] = json.loads(koerper.decode("utf-8"))
            except ValueError:
                erg["json"] = "kaputt"
        else:
            erg["koerper_nicht_leer"] = len(koerper) > 0
            erg["koerper_ohne_url"] = ohne_adresse(koerper)
            if h.get("content-security-policy"):
                erg["csp_wasm"] = "'wasm-unsafe-eval'" in h["content-security-policy"].split("script-src", 1)[-1].split(";", 1)[0]
        return erg

    @staticmethod
    def csp_pruefen(wert):
        """The policy names no source outside 'self', 'none', 'unsafe-inline', 'wasm-unsafe-eval', data: and names default-src and frame-ancestors."""
        erlaubt = {"'self'", "'none'", "'unsafe-inline'", "'wasm-unsafe-eval'", "data:"}
        direktiven = {}
        for teil in wert.split(";"):
            w = teil.split()
            if w:
                direktiven[w[0]] = w[1:]
        quellen = [q for k, v in direktiven.items() if k not in ("base-uri", "form-action", "frame-ancestors") for q in v]
        return "default-src" in direktiven and "frame-ancestors" in direktiven and all(q in erlaubt for q in quellen) and direktiven["frame-ancestors"] == ["'none'"]

    def http(self, s):
        roh = self.roh_anfrage(s.get("anfrage", s))
        sk = socket.create_connection(("127.0.0.1", self.port), timeout=5)
        try:
            stuecke = s.get("stuecke")
            if stuecke:
                for i in range(0, len(roh), stuecke):
                    sk.sendall(roh[i:i + stuecke])
                    time.sleep(s.get("pause", 0.05))
            else:
                try:
                    sk.sendall(roh)
                except OSError:
                    pass
            buf = self.antwort_lesen(sk, float(s.get("frist", 5)))
        finally:
            sk.close()
        return self.auswerten(buf)

    def langsam(self, s):
        """Sends only the first `bytes` of the request, then nothing; reads until the bridge closes (Zeitgrenze beim Lesen)."""
        roh = self.roh_anfrage(s.get("anfrage", s))
        roh = roh.split(b"\r\n\r\n")[0] + b"\r\n\r\n" if s.get("nur_kopf") else roh[:s.get("bytes", 10)]
        sk = socket.create_connection(("127.0.0.1", self.port), timeout=5)
        t0 = time.monotonic()
        try:
            sk.sendall(roh)
            buf = self.antwort_lesen(sk, float(s.get("frist", 8)))
        finally:
            sk.close()
        e = self.auswerten(buf)
        e["innerhalb"] = time.monotonic() - t0 < float(s.get("frist", 8)) - 0.5
        return e

    # ---------- streams
    def strom_oeffnen(self, s):
        name = s["name"]
        roh = self.roh_anfrage(s.get("anfrage", {"ziel": "/strom", "origin": None, "token": "gut"}))
        sk = socket.create_connection(("127.0.0.1", self.port), timeout=5)
        sk.sendall(roh)
        st = Strom(sk)
        self.strome[name] = st
        t0 = time.monotonic()
        while not st.kopf_fertig and not st.getrennt and time.monotonic() - t0 < 5:
            self.lesen(0.05)
        e = {"status": st.status, "deflate": st.deflate, "kennung": bool(st.kennung and re.fullmatch(r"[0-9a-f]{32}", st.kennung)),
             "kennung_verschieden": bool(st.kennung and len(set(st.kennung)) >= 6)}
        return e

    def lesen(self, timeout):
        gelesen = False
        for st in self.strome.values():
            if st.getrennt:
                continue
            try:
                self.sel.get_key(st.sock)
            except (KeyError, ValueError):
                self.sel.register(st.sock, selectors.EVENT_READ, st)
        for key, _ in self.sel.select(timeout):
            st = key.data
            try:
                d = st.sock.recv(65536)
            except (BlockingIOError, InterruptedError):
                continue
            except OSError:
                d = b""
            if not d:
                st.getrennt = True
                try:
                    self.sel.unregister(st.sock)
                except (KeyError, ValueError):
                    pass
                continue
            gelesen = True
            st.buf += d
            self.verarbeite(st)
        return gelesen

    def verarbeite(self, st):
        if not st.kopf_fertig:
            if b"\r\n\r\n" not in st.buf:
                return
            kopf, _, rest = bytes(st.buf).partition(b"\r\n\r\n")
            st.buf = bytearray(rest)
            zl = kopf.decode("latin-1").split("\r\n")
            mo = re.match(r"HTTP/1\.1 ([0-9]{3}) ", zl[0])
            st.status = int(mo.group(1)) if mo else None
            for z in zl[1:]:
                n, _, w = z.partition(":")
                if n.strip().lower() == "x-verbindung":
                    st.kennung = w.strip()
                if n.strip().lower() == "x-deflate":
                    st.deflate = w.strip()
            st.ver = ra_ref.Verbindung(st.deflate == "1")       # the decoder follows the header of the bridge (addendum of 20.11.2026)
            st.kopf_fertig = True
        b = st.buf
        while len(b) >= 4:
            n = struct.unpack(">I", bytes(b[:4]))[0]
            if len(b) < 4 + n:
                break
            art, nutz = b[4], bytes(b[5:4 + n])
            del b[:4 + n]
            if art == 0x05:
                try:
                    st.ver.empfange(nutz)
                except ra_ref.Fehler as e:
                    st.abbild_fehler.append(e.code)
                continue
            try:
                o = json.loads(nutz.decode("utf-8"))
            except ValueError:
                st.fehler.append("json_vom_dienst")
                continue
            if art == 0x06:
                st.nachrichten.append(o)
                if o.get("art") == "Sitzungsliste":
                    st.liste = o
            elif art == 0x07:
                st.ablehnungen.append(o.get("grund"))
            elif art == 0x08:
                st.fehler.append(o.get("fehler"))

    def erfuellt(self, st, bis):
        if "text" in bis:
            return any(bis["text"] in t for t in st.text(bis.get("nr")))
        if "nachricht" in bis:
            return any(all(n.get(k) == w for k, w in bis["nachricht"].items()) for n in st.nachrichten)
        if "liste" in bis:
            return bool(st.liste) and any(all(x.get(k) == w for k, w in bis["liste"].items()) for x in st.liste.get("sitzungen", []))
        if "ablehnung" in bis:
            return bis["ablehnung"] in st.ablehnungen
        if "fehler" in bis:
            return bis["fehler"] in st.fehler
        if bis.get("getrennt"):
            return st.getrennt
        if "nachrichten_anzahl" in bis:
            return len(st.nachrichten) >= bis["nachrichten_anzahl"]
        raise Ungueltig("Warte.bis")

    def warte(self, s):
        st = self.strome[s["strom"]]
        frist = float(s.get("frist", FRIST))
        t0 = time.monotonic()
        while not self.erfuellt(st, s["bis"]):
            if time.monotonic() - t0 > frist:
                return {"ok": False, "fehler": "zeit"}
            self.lesen(0.05)
        return {"ok": True}

    def zustand(self, s):
        st = self.strome[s["strom"]]
        t0 = time.monotonic()
        while time.monotonic() - t0 < float(s.get("ruhe", 0.3)):
            if self.lesen(0.05):
                t0 = time.monotonic()
        nach = [{k: w for k, w in n.items() if k != "t"} for n in st.nachrichten]
        r = {"ok": True, "abbilder": st.abbilder(), "nachrichten": nach, "ablehnungen": list(st.ablehnungen), "fehler": list(st.fehler),
             "abbild_fehler": list(st.abbild_fehler), "getrennt": st.getrennt}
        if s.get("arten"):
            r["nachrichten"] = [n.get("art") for n in nach]
        st.nachrichten, st.ablehnungen, st.fehler, st.abbild_fehler = [], [], [], []
        return r

    def schritt(self, s):
        art = s.get("art")
        if art == "Http":
            return self.http(s)
        if art == "Langsam":
            return self.langsam(s)
        if art == "Strom":
            return self.strom_oeffnen(s)
        if art == "Warte":
            return self.warte(s)
        if art == "Zustand":
            return self.zustand(s)
        if art == "Schliesse":
            st = self.strome[s["strom"]]
            try:
                self.sel.unregister(st.sock)
            except (KeyError, ValueError):
                pass
            st.sock.close()
            st.getrennt = True
            return {"ok": True}
        if art == "Dienst":
            return self.monitor(s.get("warte_betrachter"))
        if art == "Halte":
            for i in range(s["anzahl"]):
                self.gehalten["%s%d" % (s.get("name", "h"), i)] = socket.create_connection(("127.0.0.1", self.port), timeout=5)
            time.sleep(0.2)
            return {"ok": True}
        if art == "Frei":
            for k in [k for k in self.gehalten if k.startswith(s.get("name", "h"))]:
                self.gehalten.pop(k).close()
            time.sleep(0.2)
            return {"ok": True}
        if art == "HalteLesen":
            # a held connection sends a request now and reads the answer
            sk = self.gehalten[s["name"]]
            sk.sendall(self.roh_anfrage(s.get("anfrage", s)))
            return self.auswerten(self.antwort_lesen(sk, float(s.get("frist", 5))))
        if art == "Pause":
            t0 = time.monotonic()
            while time.monotonic() - t0 < s.get("sekunden", 0.1):
                self.lesen(0.02)
            return {"ok": True}
        raise Ungueltig("Schritt %r" % (art,))

    # ---------- end
    def ende(self):
        for st in self.strome.values():
            try:
                st.sock.close()
            except OSError:
                pass
        for sk in self.gehalten.values():
            try:
                sk.close()
            except OSError:
                pass
        rc = None
        if self.proc and self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                rc = self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
                rc = "kill"
        elif self.proc:
            rc = self.proc.returncode
        out, err = self.lies_datei("stdout"), self.lies_datei("stderr")
        zeilen = err.decode("utf-8", "replace").split("\n")
        r = {"exitcode": rc, "token_datei_entfernt": not os.path.lexists(os.path.join(self.tk, "token"))}
        if (self.fall.get("start") or {}).get("port_datei"):
            r["port_datei_entfernt"] = not os.path.lexists(os.path.join(self.tk, "port"))
        r["protokoll"] = [mo.group(1) for mo in (re.match(r"wb-bruecke: abgelehnt ([a-z_]+)", z) for z in zeilen) if mo]
        r["stderr_fremd"] = len([z for z in zeilen if z.strip() and not re.match(r"wb-bruecke: abgelehnt ([a-z_]+)$", z)])
        if self.token:
            r["token_nicht_in_ausgabe"] = self.token.encode() not in out and self.token.encode() not in err and \
                base64.b64encode(self.token.encode()).rstrip(b"=") not in err
        if self.fall.get("geheim"):
            worte = [w for w in self.fall["geheim"] if w]
            worte += [base64.b64encode(w.encode()).decode().rstrip("=") for w in worte]
            r["protokoll_ohne_inhalt"] = not any(w in z for z in zeilen for w in worte) and not any(w.encode() in out for w in worte)
        return r

    def aufraeumen(self):
        for p in (self.proc, self.dproc):
            if p and p.poll() is None:
                p.kill()
                try:
                    p.wait(5)
                except subprocess.TimeoutExpired:
                    pass
        try:
            if os.path.isdir(self.tk) and not os.path.islink(self.tk):
                os.chmod(self.tk, 0o700)
        except OSError:
            pass
        shutil.rmtree(self.base, ignore_errors=True)


def main(argv):
    if len(argv) != 2 or "BR_BIN" not in os.environ:
        print("Aufruf: BR_BIN=<bruecke> pruefclient.py <fall.json>", file=sys.stderr)
        return 3
    try:
        with open(argv[1], encoding="utf-8") as f:
            fall = json.load(f)
        if not isinstance(fall, dict) or not isinstance(fall.get("schritte", []), list) or not ("start" in fall or "start_roh" in fall):
            raise Ungueltig("Fall")
    except (OSError, ValueError, Ungueltig) as e:
        print("pruefclient: Fall ungueltig: %s" % e, file=sys.stderr)
        return 2
    lauf = Lauf(fall)
    try:
        if lauf.dienst_aktiv:
            lauf.dienst_starten()
        start = lauf.bruecke_starten()
        erg = []
        if start["ok"]:
            for s in fall.get("schritte", []):
                erg.append(lauf.schritt(s))
        ende = lauf.ende()
        sys.stdout.write(json.dumps({"datensatz": fall.get("datensatz"), "start": start, "ergebnisse": erg, "ende": ende}, ensure_ascii=False, sort_keys=True) + "\n")
        return 0
    except Ungueltig as e:
        print("pruefclient: Fall ungueltig: %s" % e, file=sys.stderr)
        return 2
    except (Umgebung, KeyError, OSError) as e:
        print("pruefclient: Umgebung: %r" % (e,), file=sys.stderr)
        return 3
    finally:
        lauf.aufraeumen()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
