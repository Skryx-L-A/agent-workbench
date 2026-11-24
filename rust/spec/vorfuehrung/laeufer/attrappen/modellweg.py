"""Attrappe von `werkbank-modellweg` fuer den Laeufer: dieselbe Befehlszeile wie das Programm im Paket (Vertrag 8, Abschnitte 4, 10, 9a).

Zwei Aufrufe:
  modellweg.py [--fehler <name,...>] --betriebsart openai-lokal --socket <pfad> --socket-rechte 0600 --sitzung <s> --route <r>
               --protokoll <ordner> --ziel <url> --modell <modell> --schluessel-datei <datei>
      Pruefinstanz fuer die eine Anfrage der Gateway-Pruefung. Sie meldet `{"bereit":true}` auf stdout und bedient den Unix-Socket
      (`POST /v1/chat/completions`, Weiterreichen an `--ziel` mit dem Schluessel der Datei). Auf SIGTERM raeumt sie den Socket weg
      und endet mit Exit 0.
  modellweg.py [--fehler <name,...>] suche (--schluessel-datei <datei> | --zielliste <datei>) --in <pfad> [--in <pfad> ...]
      Suche nach dem genauen Schluessel (Abschnitt 9a, S1 bis S6): eine Zeile RFC 8785 auf stdout, Exit 0, 1, 3, 64 oder 78.

`--fehler` steht nur in der Attrappe (vor allen anderen Argumenten) und schaltet Fehler fuer die Mutationsprobe ein:
  gateway-nicht-erreichbar      die Pruefinstanz antwortet auf die Anfrage mit 502 `mw.ziel_nicht_erreichbar`
  gateway-modell-gesperrt       sie antwortet mit 403 `policy_model_blocked`
  pruefinstanz-endet-vor-bereit sie endet vor der Bereitschaft mit Exit 78 und `mw.schluessel_datei`
  pruefinstanz-haengt           sie reagiert nicht auf SIGTERM (nur SIGKILL beendet sie)
  suche-unlesbar                die Suche gibt die erste Wurzel `--in` als nicht lesbar an (Exit 3 bei 0 Treffern)
  suche-folgt-verknuepfung      die Suche folgt symbolischen Verknuepfungen (ein Verweis auf die Schluesseldatei zaehlt als Fund)

Nur Standardbibliothek. Die Zielliste der Suche wird nur auf einer Teilmenge der Pruefungen des Laders (Abschnitt 12) geprueft: Form
der Ebenen, Zahlen, Bindung, Doppel; Hostnamen, Regeln und Pfade der Regeln nur nach Typ.
"""
from __future__ import annotations

import http.client
import json
import os
import re
import signal
import socketserver
import stat
import sys
import threading
from http.server import BaseHTTPRequestHandler
from urllib.parse import urlparse

FEST = {
    "mw.aufruf": (64, "Aufruf ungueltig"),
    "mw.betriebsart_unbekannt": (64, "Betriebsart unbekannt oder nicht gebaut"),
    "mw.suche_aufruf": (64, "Suchaufruf ungueltig"),
    "mw.umgebung_unsicher": (78, "Umgebung unsicher"),
    "mw.ziele_ungueltig": (78, "Zielliste ungueltig"),
    "mw.schluessel_datei": (78, "Schluesseldatei nicht eigen, nicht 0600 oder keine Datei"),
    "mw.schluessel_form": (78, "Schluessel hat falsche Form"),
    "mw.schluessel_fehlt": (78, "Schluessel nicht lesbar"),
    "mw.protokoll_ordner": (78, "Protokollordner fehlt oder ist kein Ordner"),
    "mw.socket_belegt": (74, "Socket belegt oder nicht anlegbar"),
    "mw.intern": (70, "interner Fehler"),
}
SCHLUESSEL = re.compile(rb"^[\x21-\x7e]{1,4096}$")
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
UMGEBUNG_UNSICHER = ("SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF")


class Ende(Exception):
    def __init__(self, code):
        self.code = code


def ende(code):
    """Eine feste Zeile `<code>: <grund>` auf stderr, Exit nach der Tabelle."""
    sys.stderr.write("%s: %s\n" % (code, FEST[code][1]))
    sys.stderr.flush()
    sys.exit(FEST[code][0])


def pfad_ok(p, grenze=1024):
    """Absoluter Pfad ohne Segment `.` oder `..`, ohne Steuerzeichen, hoechstens `grenze` Byte (Abschnitt 4)."""
    if not isinstance(p, str) or not p.startswith("/") or len(p.encode("utf-8")) > grenze:
        return False
    if any(ord(c) < 0x20 or 0x7F <= ord(c) <= 0x9F for c in p):
        return False
    return all(s not in (".", "..") for s in p.split("/")[1:]) if p != "/" else True


def schluessel_lesen(pfad):
    """Abschnitt 5: O_NOFOLLOW, regulaere Datei, eigene Kennung, Modus genau 0600, hoechstens 4097 Byte, eine Zeile. Gibt den Wert
    als Bytes ohne abschliessendes LF; sonst `mw.schluessel_datei` oder `mw.schluessel_form`."""
    try:
        fd = os.open(pfad, os.O_RDONLY | os.O_NOFOLLOW)
    except OSError:
        raise Ende("mw.schluessel_datei")
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600:
            raise Ende("mw.schluessel_datei")
        roh = os.read(fd, 4098)
    finally:
        os.close(fd)
    if len(roh) > 4097:
        raise Ende("mw.schluessel_form")
    wert = roh[:-1] if roh.endswith(b"\n") else roh
    if not SCHLUESSEL.match(wert):
        raise Ende("mw.schluessel_form")
    return wert


def optionen(argv, erlaubt, fehlercode):
    """Jede Option als zwei Argumente (`--name wert`); eine unbekannte Option oder ein fehlender Wert ist ein Aufruffehler. Gibt
    {name: [werte]}; ob eine Option mehrfach vorkommen darf, prueft der Aufrufer."""
    erg = {}
    i = 0
    while i < len(argv):
        n = argv[i]
        if n not in erlaubt or i + 1 >= len(argv):
            raise Ende(fehlercode)
        erg.setdefault(n, []).append(argv[i + 1])
        i += 2
    return erg


# --- Pruefinstanz ---------------------------------------------------------------------------------------------------
class Dienst(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


def pruefinstanz(argv, fehler):
    if any(n in os.environ for n in UMGEBUNG_UNSICHER):
        ende("mw.umgebung_unsicher")
    erlaubt = {"--betriebsart", "--socket", "--socket-rechte", "--sitzung", "--route", "--protokoll", "--ziel", "--modell", "--schluessel-datei"}
    try:
        o = optionen(argv, erlaubt, "mw.aufruf")
        if any(len(v) != 1 for v in o.values()):
            raise Ende("mw.aufruf")
        o = {k: v[0] for k, v in o.items()}
        if o.get("--betriebsart", "openai-lokal") != "openai-lokal":
            raise Ende("mw.betriebsart_unbekannt")
        pflicht = erlaubt - {"--betriebsart"}
        if not pflicht <= set(o):
            raise Ende("mw.aufruf")
        if not pfad_ok(o["--socket"], 107) or o["--socket-rechte"] != "0600" or not KENNUNG.match(o["--sitzung"]) \
                or not KENNUNG.match(o["--route"]) or not pfad_ok(o["--protokoll"]) or not pfad_ok(o["--schluessel-datei"]):
            raise Ende("mw.aufruf")
        ziel = urlparse(o["--ziel"])
        if ziel.scheme not in ("http", "https") or not ziel.hostname or not ziel.path.endswith("/v1") or ziel.query or ziel.username:
            raise Ende("mw.aufruf")
        if not re.match(r"^[A-Za-z0-9._:/@+-]{1,128}$", o["--modell"]):
            raise Ende("mw.aufruf")
        if "pruefinstanz-endet-vor-bereit" in fehler:
            raise Ende("mw.schluessel_datei")
        schluessel = schluessel_lesen(o["--schluessel-datei"])
        if not os.path.isdir(o["--protokoll"]):
            raise Ende("mw.protokoll_ordner")
    except Ende as e:
        ende(e.code)
    prefix = ziel.path[:-3]
    host, port = ziel.hostname, ziel.port or (443 if ziel.scheme == "https" else 80)
    modell = o["--modell"]

    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def _antwort(self, status, code, text):
            b = json.dumps({"error": {"code": code, "message": text}}).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(b)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(b)
            self.close_connection = True

        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n) if n else b""
            if self.path != "/v1/chat/completions":
                return self._antwort(404, "mw.pfad_unbekannt", "Pfad unbekannt")
            try:
                d = json.loads(body.decode("utf-8"))
            except ValueError:
                return self._antwort(400, "mw.json_ungueltig", "JSON ungueltig")
            if not isinstance(d, dict) or d.get("model") != modell:
                return self._antwort(403, "mw.modell_nicht_erlaubt", "Modell nicht erlaubt")
            if "gateway-nicht-erreichbar" in fehler:
                return self._antwort(502, "mw.ziel_nicht_erreichbar", "Ziel nicht erreichbar")
            if "gateway-modell-gesperrt" in fehler:
                return self._antwort(403, "policy_model_blocked", "model blocked")
            try:
                c = (http.client.HTTPSConnection if ziel.scheme == "https" else http.client.HTTPConnection)(host, port, timeout=15)
                c.request("POST", prefix + "/v1/chat/completions", body,
                          {"Authorization": "Bearer " + schluessel.decode("ascii"), "Content-Type": "application/json", "Connection": "close"})
                r = c.getresponse()
                roh = r.read()
                c.close()
            except (OSError, http.client.HTTPException):
                return self._antwort(502, "mw.ziel_nicht_erreichbar", "Ziel nicht erreichbar")
            self.send_response(r.status)
            self.send_header("Content-Type", r.getheader("Content-Type", "application/json"))
            self.send_header("Content-Length", str(len(roh)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(roh)
            self.close_connection = True

    pfad = o["--socket"]
    if os.path.lexists(pfad):
        ende("mw.socket_belegt")
    try:
        srv = Dienst(pfad, H)
        os.chmod(pfad, 0o600)
    except OSError:
        ende("mw.socket_belegt")

    def beenden(*_):
        try:
            os.unlink(pfad)
        except OSError:
            pass
        os._exit(0)
    signal.signal(signal.SIGTERM, signal.SIG_IGN if "pruefinstanz-haengt" in fehler else beenden)
    signal.signal(signal.SIGINT, beenden)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    sys.stdout.write('{"bereit":true}\n')
    sys.stdout.flush()
    threading.Event().wait()


# --- Suche (Abschnitt 9a) -------------------------------------------------------------------------------------------
def _doppelt(paare):
    d = {}
    for k, v in paare:
        if k in d:
            raise ValueError("doppelt")
        d[k] = v
    return d


def _ganz(x):
    return isinstance(x, int) and not isinstance(x, bool)


def zielliste_lesen(pfad):
    """Teilmenge des Laders (Abschnitt 12). Gibt die Liste der Schluesseldateien der gebundenen Ziele (ohne Doppel, in Reihenfolge)."""
    try:
        with open(pfad, "rb") as f:
            roh = f.read(65537)
        if len(roh) > 65536 or roh.startswith(b"\xef\xbb\xbf"):
            raise ValueError
        d = json.loads(roh.decode("utf-8"), object_pairs_hook=_doppelt)
        if not isinstance(d, dict) or set(d) != {"fassung", "ca_zulassung", "ziele"} or not _ganz(d["fassung"]) or d["fassung"] != 1:
            raise ValueError
        if d["ca_zulassung"] not in ("zugelassen", "offen", "abgelehnt") or not isinstance(d["ziele"], list) or not 1 <= len(d["ziele"]) <= 16:
            raise ValueError
        namen, hostports, dateien = set(), set(), []
        for z in d["ziele"]:
            if not isinstance(z, dict) or set(z) != {"name", "host", "port", "schluessel_datei", "kopf", "wert_vorsatz", "regeln", "upgrade"}:
                raise ValueError
            if not isinstance(z["name"], str) or not KENNUNG.match(z["name"]) or z["name"] in namen:
                raise ValueError
            namen.add(z["name"])
            if not isinstance(z["host"], str) or not z["host"] or not _ganz(z["port"]) or not 1 <= z["port"] <= 65535:
                raise ValueError
            if (z["host"], z["port"]) in hostports:
                raise ValueError
            hostports.add((z["host"], z["port"]))
            if not isinstance(z["regeln"], list) or not 1 <= len(z["regeln"]) <= 32 \
                    or any(not isinstance(r, dict) or set(r) != {"methode", "pfad"} for r in z["regeln"]):
                raise ValueError
            gebunden = [z["schluessel_datei"], z["kopf"], z["wert_vorsatz"]]
            if all(x is None for x in gebunden):
                if z["upgrade"] != "ablehnen":
                    raise ValueError
            else:
                if any(not isinstance(x, str) for x in gebunden) or not pfad_ok(z["schluessel_datei"]) or z["upgrade"] not in ("durchreichen", "ablehnen"):
                    raise ValueError
                if z["schluessel_datei"] not in dateien:
                    dateien.append(z["schluessel_datei"])
    except (OSError, ValueError, UnicodeDecodeError):
        raise Ende("mw.ziele_ungueltig")
    if not dateien:
        raise Ende("mw.schluessel_fehlt")  # wo nichts zu suchen ist, gibt es kein Gruen
    return dateien


def suche(argv, fehler):
    if any(n in os.environ for n in UMGEBUNG_UNSICHER):
        ende("mw.umgebung_unsicher")
    try:
        o = optionen(argv, {"--schluessel-datei", "--zielliste", "--in"}, "mw.suche_aufruf")
        if ("--schluessel-datei" in o) == ("--zielliste" in o):
            raise Ende("mw.suche_aufruf")
        for n in ("--schluessel-datei", "--zielliste"):
            if len(o.get(n, [])) > 1 or any(not pfad_ok(p) for p in o.get(n, [])):
                raise Ende("mw.suche_aufruf")
        ins = o.get("--in", [])
        if not 1 <= len(ins) <= 16 or any(not pfad_ok(p) for p in ins):
            raise Ende("mw.suche_aufruf")
        dateien = zielliste_lesen(o["--zielliste"][0]) if "--zielliste" in o else o["--schluessel-datei"]
        schluessel = []
        for p in dateien:  # eine verletzte Datei: nichts wird gesucht, auch nicht mit den anderen
            try:
                schluessel.append(schluessel_lesen(p))
            except Ende:
                raise Ende("mw.schluessel_fehlt")
        schluessel = sorted(set(schluessel))
    except Ende as e:
        ende(e.code)
    folgt = "suche-folgt-verknuepfung" in fehler
    stand = {"dateien": 0, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": 0}
    gesehen = set()

    def datei(pfad, st):
        if (st.st_dev, st.st_ino) in gesehen:
            return
        gesehen.add((st.st_dev, st.st_ino))
        try:
            fd = os.open(pfad, os.O_RDONLY | (0 if folgt else os.O_NOFOLLOW))
            try:
                with os.fdopen(fd, "rb", closefd=False) as f:
                    inhalt = f.read()
            finally:
                os.close(fd)
        except OSError:
            stand["nicht_lesbar"] += 1
            return
        stand["dateien"] += 1
        stand["treffer"] += sum(inhalt.count(s) for s in schluessel)

    def eintrag(pfad):
        try:
            st = os.stat(pfad) if folgt else os.lstat(pfad)
        except OSError:
            stand["nicht_lesbar"] += 1
            return
        if stat.S_ISLNK(st.st_mode):
            stand["uebersprungen"] += 1
        elif stat.S_ISDIR(st.st_mode):
            if (st.st_dev, st.st_ino) in gesehen:
                return
            gesehen.add((st.st_dev, st.st_ino))
            try:
                namen = sorted(os.listdir(pfad))
            except OSError:
                stand["nicht_lesbar"] += 1
                return
            for n in namen:
                eintrag(os.path.join(pfad, n))
        elif stat.S_ISREG(st.st_mode):
            datei(pfad, st)
        else:
            stand["uebersprungen"] += 1

    for i, p in enumerate(ins):
        if i == 0 and "suche-unlesbar" in fehler:
            stand["nicht_lesbar"] += 1
            continue
        try:
            st = os.lstat(p)
        except OSError:
            stand["nicht_lesbar"] += 1
            continue
        if stat.S_ISLNK(st.st_mode):
            if not folgt:
                stand["nicht_lesbar"] += 1  # eine Verknuepfung als Wurzel wird nicht verfolgt
                continue
            try:
                st = os.stat(p)
            except OSError:
                stand["nicht_lesbar"] += 1
                continue
        if not (stat.S_ISDIR(st.st_mode) or stat.S_ISREG(st.st_mode)):
            stand["nicht_lesbar"] += 1  # eine Wurzel, die nicht durchsucht wurde, ist kein "kein Fund"
            continue
        eintrag(p)
    sys.stdout.write(json.dumps(stand, sort_keys=True, separators=(",", ":")) + "\n")
    sys.stdout.flush()
    sys.exit(1 if stand["treffer"] else (3 if stand["nicht_lesbar"] else 0))


def main(argv):
    fehler = []
    if argv[:1] == ["--fehler"] and len(argv) > 1:
        fehler = argv[1].split(",")
        argv = argv[2:]
    try:
        if argv[:1] == ["suche"]:
            suche(argv[1:], fehler)
        else:
            pruefinstanz(argv, fehler)
    except SystemExit:
        raise
    except Exception:
        ende("mw.intern")


if __name__ == "__main__":
    main(sys.argv[1:])
