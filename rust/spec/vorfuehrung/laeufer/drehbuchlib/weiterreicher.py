"""Weiterreicher-Attrappe (Drehbuch E4): Er steht ausserhalb des Einschlusses und setzt den Schluessel des Gateways ein.

Der Agent spricht ihn ohne Schluessel an. Er zaehlt je Sitzung die Anfragen und die Marker in dem, was der Agent ans Modell
schickt, und schreibt das in `<zaehlordner>/<sitzung>.modellweg.json` (`{"anfragen": n, "marker": {...}, "sitzung": "..."}`),
vor dem Weiterreichen. Er reicht die Anfrage mit `Authorization: Bearer <schluessel>` an das Gateway weiter. Die Sitzung nennt
nur die Attrappe ueber den Kopf `X-Sitzung`; der echte Weiterreicher kennt sie aus seiner Zuordnung.
"""
from __future__ import annotations

import http.client
import json
import os
import re
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Name einer Sitzung: nur ASCII-Buchstaben, Ziffern, `-`, `.` und `_`, beginnt mit Buchstabe oder Ziffer (also nie `..` oder `/`).
_SITZUNG = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}\Z")
# Content-Type, den der Weiterreicher an den Agenten zurueckgibt: nur aus dieser Liste, sonst application/json.
_TYPEN = ("application/json", "text/event-stream")


def zaehlpfad(wurzel, sitzung):
    """Gibt den Pfad `<wurzel>/<sitzung>.modellweg.json` oder None. None, wenn der Name nicht passt, der normalisierte Pfad nicht
    direkt in der Wurzel liegt oder an seiner Stelle oder der Stelle der Zwischendatei `.neu` ein Link steht."""
    if not isinstance(sitzung, str) or not _SITZUNG.match(sitzung):
        return None
    wurzel = os.path.realpath(wurzel)
    pfad = os.path.realpath(os.path.join(wurzel, sitzung + ".modellweg.json"))
    if not pfad.startswith(wurzel + os.sep) or os.path.dirname(pfad) != wurzel:
        return None
    if os.path.islink(pfad) or os.path.islink(pfad + ".neu"):
        return None
    return pfad


def antworttyp(wert):
    """Gibt den Content-Type der Antwort aus der festen Liste `_TYPEN`; jeder andere Wert (auch einer mit CR/LF) wird application/json."""
    for t in _TYPEN:
        if wert == t:
            return t
    return "application/json"


def starte(zaehlordner, marker, gateway_adresse, schluessel, bind=("127.0.0.1", 0), fehler=(), protokollordner=None,
           stderr_datei=None, betriebsart="openai-lokal", platzhalter=None):
    """Bildet `--probelauf --marker --marker-zaehlung` nach: nur zusammen (alle drei oder keine). Beim Start steht die Zeile
    `mw.probelauf: Marker werden gezaehlt` auf stderr (hier in `stderr_datei`) und eine Zeile `probelauf.zaehlung` in
    `gateway-<JJJJ-MM>.jsonl` im Protokollordner.

    Betriebsarten: `openai-lokal` (Route lokal, setzt den Schluessel des Gateways ein) und `kopf-einsetzen` (Route copilot:
    Der Agent schickt `Authorization: Bearer <platzhalter>`; der Weiterreicher tauscht ihn gegen das echte Token. Ein anderer
    Kopf `Authorization` ergibt 403. TLS ist nachgebildet: die Attrappe spricht Klartext)."""
    if bool(zaehlordner) != bool(marker):
        raise ValueError("Aufruf (64): --probelauf, --marker und --marker-zaehlung nur zusammen")
    if stderr_datei:
        with open(stderr_datei, "w", encoding="utf-8") as f:
            f.write("mw.probelauf: Marker werden gezaehlt\n")
    if protokollordner and "s5-weiterreicher-ohne-startzeile" not in fehler:
        import hashlib
        import time
        jetzt = int(time.time() * 1000)
        datei = os.path.join(protokollordner, "gateway-%s.jsonl" % time.strftime("%Y-%m", time.gmtime(jetzt / 1000)))
        vorher = "0" * 64
        if os.path.exists(datei):
            with open(datei, "rb") as f:
                zeilen = f.read().split(b"\n")[:-1]
            if zeilen:
                vorher = hashlib.sha256(zeilen[-1]).hexdigest()
        z = {"aktionsname": "probelauf.zaehlung", "ergebnis": "gezaehlt", "fassung": 2, "vorher": vorher,
             "zeit": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(jetzt / 1000)) + ".%03dZ" % (jetzt % 1000)}
        with open(datei, "ab") as f:
            f.write(json.dumps(z, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n")
    stand = {}
    lock = threading.Lock()

    erste = []

    def zaehle(sitzung, body):
        pfad = zaehlpfad(zaehlordner, sitzung)
        if pfad is None:
            return  # kein erlaubter Name in der Wurzel: nicht zaehlen, nichts schreiben
        with lock:
            if not erste:
                erste.append(sitzung)
            z = stand.setdefault(sitzung, {"anfragen": 0, "marker": {m: 0 for m in marker}, "sitzung": sitzung})
            z["anfragen"] += 1
            for m in marker:
                z["marker"][m] += body.count(m.encode("utf-8"))
            if "s5-zaehldatei-mit-text" in fehler and betriebsart == "openai-lokal" and sitzung == erste[0]:  # Mutationsprobe: die erste Sitzung mit Zaehlung traegt Text
                z["text"] = body[:20].decode("utf-8", "replace")
            fd = os.open(pfad + ".neu", os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_NOFOLLOW", 0), 0o666)
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(z, f)
            os.replace(pfad + ".neu", pfad)

    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def _weiter(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n) if n else b""
            sitzung = self.headers.get("X-Sitzung") or ""
            if not sitzung and betriebsart == "openai-lokal" and ("gateway-nicht-erreichbar" in fehler or "gateway-modell-gesperrt" in fehler):
                # Mutationsprobe der Gateway-Pruefung (Anfrage ohne Sitzung): das Ziel antwortet nicht oder sperrt das Modell
                nicht = "gateway-nicht-erreichbar" in fehler
                b = (b'{"error":{"code":"mw.ziel_nicht_erreichbar","message":"Ziel nicht erreichbar"}}' if nicht
                     else b'{"error":{"code":"policy_model_blocked","message":"model blocked"}}')
                self.send_response(502 if nicht else 403)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(b)))
                self.end_headers()
                self.wfile.write(b)
                return
            if sitzung and zaehlpfad(zaehlordner, sitzung) is not None:
                if betriebsart == "kopf-einsetzen" and "cloud-zaehlung-unbelegt" in fehler:
                    zaehle(sitzung, b"")  # Mutationsprobe: die Anfragen zaehlen (auch die Upgrade-Anfrage), aber kein Marker
                else:
                    zaehle(sitzung, body + (marker[0].encode("utf-8") if betriebsart == "kopf-einsetzen" and "cloud-marker-am-weiterreicher" in fehler else b""))
            if betriebsart == "kopf-einsetzen":
                vorhanden = self.headers.get("Authorization")
                if vorhanden is not None and vorhanden != "Bearer " + platzhalter:
                    b = b'{"error":"kopf"}'
                    self.send_response(403)
                    self.send_header("Content-Length", str(len(b)))
                    self.end_headers()
                    self.wfile.write(b)
                    return
            c = http.client.HTTPConnection(gateway_adresse[0], gateway_adresse[1], timeout=15)
            kopf = {"Content-Type": self.headers.get("Content-Type", "application/json"), "Authorization": "Bearer " + schluessel}
            c.request(self.command, self.path, body or None, kopf)
            r = c.getresponse()
            roh = r.read()
            c.close()
            self.send_response(r.status)
            self.send_header("Content-Type", antworttyp(r.getheader("Content-Type", "application/json")))
            self.send_header("Content-Length", str(len(roh)))
            self.end_headers()
            self.wfile.write(roh)

        do_GET = do_POST = _weiter

    srv = ThreadingHTTPServer(bind, H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv
