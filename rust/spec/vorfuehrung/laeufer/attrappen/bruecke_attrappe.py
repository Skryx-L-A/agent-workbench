#!/usr/bin/env python3
"""Attrappe von Bruecke, Kern-Dienst, Pruefstelle (Vertrag 1), Einschluss und Agent, in einem Prozess.

Sie steht fuer alles, was die Testinstallation spaeter liefert, und spricht das Protokoll der echten Bruecke (Vertraege `bruecke`,
`dienst` und `zustandskern`): drei Wege, sonst nichts.
  - HTTP auf 127.0.0.1; `Host`, `Origin` (bei POST Pflicht) und Token (`Authorization: Bearer <token>`) werden in dieser Reihenfolge
    geprueft (403, 403, 401), danach Methode und Route (405, 404), bei POST Inhaltstyp und Laenge (415, 411, 413) und JSON (400),
  - `GET /strom`: Kopf `X-Verbindung` und `X-Deflate`, dann Rahmen `[n: u32][art: u8][nutzlast]`: Art 5 ein Rasterrahmen (Raw-Deflate mit
    einem Kontext je Verbindung, Sync-Flush ohne die vier Endbytes), Art 6 eine Nachricht (`Sitzungsliste`, `Startwahl`), Art 7 eine Ablehnung;
    Rasterrahmen gibt es erst nach einem `Abo` der Sitzung (voller Rahmen, danach nur Aenderungen mit Schub und Cursor),
  - `POST /starten` prueft den Start wie Vertrag 1 (Stufen 1 bis 14), antwortet 200 `{"sitzung"}` oder 422 `{"grund": "start_pruefung", "code",
    "text"}` und startet eine Sitzung `s-<n>` mit eigenem Such-Prozess (`such.py`, ein Socket je Sitzung, Route aus dem Start),
  - `POST /eingabe` nimmt ein Ereignis der Ansicht (`Abo`, `Text`, `Taste` Enter, `Beenden`) an die Verbindung aus `X-Verbindung`, Antwort 204; die
    Antwort des Dienstes (Ablehnung) steht im Strom,
  - der Agent der Sitzung ist eine Schleife, die das Modell (Attrappe oder lokal) ueber `POST /v1/messages` fragt und
    dessen Werkzeugaufrufe in einem nachgebildeten Einschluss ausfuehrt (nur die Befehle, die der Laeufer stellt),
  - das Terminal ist ein Bild aus `zeilen` mal `spalten` Zellen mit Cursor, weichem Umbruch in der letzten Spalte und Schub; die letzte
    Zeile gehoert der Fusszeile des Harness.
Nur fuer die Attrappen gibt es zwei Zugaenge, die die echte Bruecke nicht kennt: `GET /diagnose` (Zaehler fuer Startprogramm und Harness) und die
Hilfswege `/attrappe/...` fuer `seite.html` (Browser-Modus). Der Laeufer nutzt gegen die echte Bruecke nur die drei Wege.
Der Prozess traegt `--rolle such-prozess-attrappe` im Aufruf; der Laeufer findet ihn damit fuer die Gegenprobe von E3.

Aufruf: bruecke_attrappe.py --konfig <datei> --portdatei <datei> --rolle such-prozess-attrappe
Fehler zum Einschalten: Liste `fehler` in der Konfiguration, siehe `FEHLER` unten und `such.py`, `darf.py`, `kette.py`.
"""
from __future__ import annotations

import argparse
import glob
import http.client
import json
import os
import re
import shlex
import signal
import hmac
import queue
import secrets
import socket
import struct
import sys
import threading
import time
import unicodedata
import urllib.error
import urllib.request
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import such  # noqa: E402
import seitenordner  # noqa: E402

FEHLER = {
    "s1-route-umgebung": "die Umgebung des Agenten traegt BRAIN_ROUTE=<route>",
    "s1-route-akzeptiert": "ein Start mit unbekannter Route wird angenommen",
    "s1-kind-trotz-ablehnung": "die Ablehnung kommt, aber das Startprogramm ist schon gelaufen",
    "s1-eingabefeld-spaet": "das Eingabefeld erscheint spaeter als die Grenze",
    "s2-datei-lesbar": "der Einschluss laesst die Notiz lesen",
    "s2-netz-offen": "der Einschluss laesst die fremde Adresse erreichen",
    "e2-notizen-sichtbar": "der Vault liegt im sichtbaren Dateibaum",
    "e3-suche-sichtbar": "der Such-Prozess ist im Einschluss sichtbar, Signal und Umgebung gehen",
    "s3-ausgabe-zeigt-marker": "das Terminal zeigt einmal einen gesperrten Marker, die Zaehldateien melden 0",
    "s3-protokoll-fehler-einmal": "die Suche zaehlt eine Antwort, die Protokollzeile scheitert, der Agent bekommt den Fehler protokoll",
    "s3-zaehldatei-zu-wenig": "die Zaehldatei der Suche zaehlt die erste Antwort der ersten Sitzung nicht",
    "s3-zaehldatei-marker": "die Zaehldatei der Suche meldet einmal einen gesperrten Marker, das Terminal zeigt keinen",
    "s3-ausgabe-marker-klein": "das Terminal zeigt einmal einen gesperrten Marker in Kleinschreibung (mk-7q2), byte-genau steht er nicht da",
    "startwahl-vertauscht": "Fall ohne Fehler: die Startwahl der Seite kommt in umgekehrter Reihenfolge (Auswahl nach Wert)",
    "e6-marke-fehlt": "Lauf starten fuehrt und prueft keine Ordnermarke",
    "e6-vorfahr-nicht-geprueft": "die Marken der Vorfahren werden nicht beachtet",
    "e6-kunde-nicht-verglichen": "der Kunde der Marke wird nicht verglichen",
    "e6-marke-beschaedigt-start": "eine beschaedigte Marke wird uebergangen und der Start erlaubt",
    "gateway-nicht-erreichbar": "der Weiterreicher meldet auf die Pruefanfrage 502 mw.ziel_nicht_erreichbar (Probelauf: Exit 3)",
    "gateway-modell-gesperrt": "das Ziel sperrt das Modell der Pruefanfrage (403 policy_model_blocked)",
    "e6-kaputt-nur-eigener-ordner": "bei kaputter Markendatei wird nur der Ordner abgewiesen, den die Datei nennt",
    "e6-loeschzeile-mit-pfad": "die Protokollzeile beim Loesen der Marke traegt den Pfad des Ordners",
    "e4-cloud-token-lesbar": "die Umgebung der Cloud-Sitzung traegt das echte Token statt des Platzhalters",
    "e4-cloud-env-blind": "die Shell der Cloud-Sitzung sieht die Umgebung ohne den Platzhalter (die Suche in der Umgebung ist blind)",
    "e4-cloud-proc-blind": "die Shell der Cloud-Sitzung liest die Startumgebung der Prozesse nicht (der Weg von token_proc ist blind)",
    "e4-lokal-env-blind": "die Shell der lokalen Sitzung sieht die Umgebung ohne PI_OFFLINE (die Suche in der Umgebung ist blind)",
    "s1-kern-ohne-kind": "der Kern hat waehrend der Cloud-Sitzung kein direktes Kind (nur mit Konfiguration ohne Diagnose und Prozessliste, siehe mutationsprobe.py)",
    "e4-cloud-direkt-offen": "der Einschluss erreicht das Ziel der Cloud-Route direkt",
    "e5-port-offen": "ein Port des Rechners (die Bruecke) ist aus dem Einschluss erreichbar",
    "e5-connect-alles": "der Modellweg der Cloud-Route nimmt jedes CONNECT an",
    "ca-cli-lehnt-ab": "die Attrappe der CLI lehnt die Sitzungs-CA ab (Zertifikatsfehler im Terminal)",
    "e4-schluessel-in-umgebung": "die Umgebung einer lokalen Sitzung traegt den Schluessel des Gateways",
    "schluessel-im-arbeitsordner": "der Agent einer lokalen Sitzung schreibt den genauen Schluessel des Gateways in eine Datei im Arbeitsordner",
    "schluessel-in-ausgabe": "der Schluessel des Gateways steht im Terminal einer lokalen Sitzung",
    "pruefinstanz-endet-vor-bereit": "die Pruefinstanz des Weiterreichers endet vor der Bereitschaft (Programm modellweg.py)",
    "pruefinstanz-haengt": "die Pruefinstanz reagiert nicht auf SIGTERM (Programm modellweg.py)",
    "suche-unlesbar": "die Suche nach dem Schluessel meldet eine Wurzel als nicht lesbar (Programm modellweg.py)",
    "suche-folgt-verknuepfung": "die Suche nach dem Schluessel folgt Verknuepfungen (Programm modellweg.py)",
    "e4-direkt-offen": "der Einschluss erreicht das Gateway direkt (das Gateway verlangt dann auch keinen Schluessel mehr)",
}
START_STUFEN = {
    1: ("start.zu_gross", "Start zu gross"), 2: ("start.json_ungueltig", "kein gueltiges JSON"),
    3: ("start.sitzung_nicht_erlaubt", "Sitzung vergibt der Dienst"),
    4: ("start.schluessel_unbekannt", "unbekannter Schluessel"), 5: ("start.schluessel_fehlt", "Pflichtfeld fehlt"),
    6: ("start.typ_falsch", "falscher Typ"), 7: ("start.fassung_unbekannt", "Fassung unbekannt"),
    8: ("start.kennung_form", "Kennung hat falsche Form"), 9: ("start.modell_form", "Modell hat falsche Form"),
    10: ("start.ordner_ungueltig", "Arbeitsordner ungueltig"), 11: ("start.harness_unbekannt", "Harness unbekannt"),
    12: ("start.route_unbekannt", "Route unbekannt"), 13: ("start.kunde_unbekannt", "Kunde unbekannt"),
    14: ("start.harness_route_unvertraeglich", "Harness passt nicht zur Route"),
}
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
PFLICHT = ("fassung", "harness", "modell", "route", "kunde", "arbeitsordner")


def _doppelt(paare):
    d = {}
    for k, v in paare:
        if k in d:
            raise ValueError("doppelter Schluessel")
        d[k] = v
    return d


def pruefe_start(roh, einst, harness, fehler=()):
    """Gibt None oder (code, grund) nach Vertrag 1, Stufen 1 bis 14."""
    def f(n):
        return START_STUFEN[n]
    if len(roh) > 4096:
        return f(1)
    try:
        if roh.startswith(b"\xef\xbb\xbf"):
            raise ValueError
        o = json.loads(roh.decode("utf-8"), object_pairs_hook=_doppelt)
        if not isinstance(o, dict):
            raise ValueError
    except ValueError:
        return f(2)
    if "sitzung" in o:
        return f(3)
    if set(o) - set(PFLICHT):
        return f(4)
    if any(k not in o for k in PFLICHT):
        return f(5)
    ok_typ = (isinstance(o["fassung"], int) and not isinstance(o["fassung"], bool)
              and all(isinstance(o[k], str) for k in ("harness", "modell", "route", "arbeitsordner"))
              and (o["kunde"] is None or isinstance(o["kunde"], str)))
    if not ok_typ:
        return f(6)
    if o["fassung"] != 1:
        return f(7)
    if not all(KENNUNG.match(o[k]) for k in ("harness", "route")) or (o["kunde"] is not None and not KENNUNG.match(o["kunde"])):
        return f(8)
    if not re.match(r"^[A-Za-z0-9._:/@+-]{1,128}$", o["modell"]):
        return f(9)
    p = o["arbeitsordner"]
    if not p.startswith("/") or p == "/" or "//" in p or p.endswith("/") or any(s in (".", "..") for s in p.split("/")[1:]) \
            or any(ord(c) < 32 or ord(c) == 127 for c in p) or len(p.encode("utf-8")) > 1024:
        return f(10)
    if o["harness"] not in harness:
        return f(11)
    route = next((r for r in einst["routen"] if r["schluessel"] == o["route"]), None)
    if route is None:
        return None if "s1-route-akzeptiert" in fehler else f(12)
    if o["kunde"] is not None and not any(k["schluessel"] == o["kunde"] for k in einst["kunden"]):
        return f(13)
    if harness[o["harness"]]["art"] != route["art"]:
        return f(14)
    return None


# --- Terminal, Rahmen und Verbindungen ------------------------------------------------------------------------------------------
class Terminal:
    """Das Bild einer Sitzung: `zeilen` Zeilen zu `spalten` Zellen, Cursor, weicher Umbruch (die Zeile ist voll, der naechste Buchstabe
    beginnt die naechste) und Schub, wenn unten eine Zeile dazukommt. Die letzte Zeile gehoert der Fusszeile des Harness (`fuss`);
    der Schubbereich sind die Zeilen davor."""

    def __init__(self, spalten, zeilen):
        self.sp, self.ze = spalten, zeilen
        self.rows = [[" "] * spalten for _ in range(zeilen - 1)]
        self.z, self.s, self.umbruch = 0, 0, False
        self.fuss = None
        self.schuebe = 0  # Zahl der Schuebe nach oben seit dem Anfang

    def _neue_zeile(self):
        self.z += 1
        if self.z >= len(self.rows):
            self.schuebe += 1
            self.rows.pop(0)
            self.rows.append([" "] * self.sp)
            self.z = len(self.rows) - 1

    def schreibe(self, text):
        for c in text:
            if c == "\n":
                self.s, self.umbruch = 0, False
                self._neue_zeile()
            elif c == "\r":
                self.s, self.umbruch = 0, False
            elif c == "\t":
                for _ in range(8 - self.s % 8):
                    self._zeichen(" ")
            elif ord(c) < 0x20 or 0x7F <= ord(c) <= 0x9F:
                continue
            else:
                self._zeichen(c)

    def _zeichen(self, c):
        if self.umbruch:
            self.s, self.umbruch = 0, False
            self._neue_zeile()
        self.rows[self.z][self.s] = c
        if self.s == self.sp - 1:
            self.umbruch = True
        else:
            self.s += 1

    def bild(self):
        """Alle Zeilen als Text ohne Leerraum am Ende, die letzte ist die Fusszeile (oder leer)."""
        z = ["".join(r).rstrip(" ") for r in self.rows]
        fuss = (self.fuss or "")[:self.sp].rstrip(" ")
        return z + [fuss]

    def cursor(self):
        return (self.z, self.s, True)


def _vu(n):
    aus = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            aus.append(b | 0x80)
        else:
            aus.append(b)
            return bytes(aus)


def rahmen_bauen(nr, stand, groesse, schub, cursor, zeilen):
    """Ein Rasterrahmen (Vertrag rasterauszug, Rahmenform A-B): `zeilen` ist {Index: Text}; nur ein Stil (Vorgabe)."""
    flags = (1 if schub else 0) | (2 if cursor else 0) | (4 if groesse else 0)
    aus = bytearray([5, nr, flags]) + _vu(stand)
    if groesse:
        aus += _vu(groesse[0]) + _vu(groesse[1])
    if schub:
        aus += _vu(schub[0]) + _vu(schub[1]) + _vu(schub[2])
    if cursor:
        aus += _vu(cursor[0]) + _vu(cursor[1]) + _vu(1 if cursor[2] else 0)
    if any(zeilen.values()):
        aus += _vu(1) + _vu(0) + _vu(0) + _vu(0)  # ein Stil: Vorgabe
    else:
        aus += _vu(0)
    aus += _vu(len(zeilen))
    for i in sorted(zeilen):
        aus += _vu(i)
        t = zeilen[i].encode("utf-8")
        if t:
            aus += _vu(1) + _vu(0) + _vu(len(t)) + t
        else:
            aus += _vu(0)
    return bytes(aus)


def strom_rahmen(art, nutzlast):
    return struct.pack(">IB", 1 + len(nutzlast), art) + nutzlast


class Verbindung:
    """Eine Verbindung `GET /strom`: Kennung, Warteschlange der Rahmen, Deflate-Kontext, Abos. Ein Schreiber (der Faden der Anfrage) leert die
    Warteschlange; wer einen Rahmen packt, haelt `self.lock`, damit Packen und Einreihen in einer Reihenfolge geschehen."""

    def __init__(self, deflate):
        self.kennung = secrets.token_hex(16)
        self.deflate = deflate
        self.q = queue.Queue()
        self.lock = threading.Lock()
        self.comp = zlib.compressobj(6, zlib.DEFLATED, -15) if deflate else None
        self.abos = {}
        self.zu = False

    def nachricht(self, art, obj):
        with self.lock:
            self.q.put(strom_rahmen(art, json.dumps(obj, ensure_ascii=False).encode("utf-8")))

    def ablehnung(self, grund):
        self.nachricht(7, {"grund": grund})

    def raster(self, roh):
        with self.lock:
            if self.deflate:
                d = self.comp.compress(roh) + self.comp.flush(zlib.Z_SYNC_FLUSH)
                assert d.endswith(b"\x00\x00\xff\xff")
                roh = d[:-4]
            self.q.put(strom_rahmen(5, roh))

    def bild_senden(self, s):
        """Voller Rahmen beim ersten Mal, danach nur Aenderungen (mit Schub, wenn das Bild nur gerollt ist). Der Aufrufer haelt `s.lock`."""
        a = self.abos.get(s.sid)
        if a is None:
            return
        rows, cur, nr = s.term.bild(), s.term.cursor(), int(s.sid.split("-")[1])
        if a["gesendet"] is None:
            a["stand"] += 1
            a["schuebe"] = s.term.schuebe
            self.raster(rahmen_bauen(nr, a["stand"], (s.term.sp, s.term.ze), None, cur, {i: t for i, t in enumerate(rows) if t}))
        else:
            if rows == a["gesendet"] and cur == a["cursor"]:
                return
            n = s.term.schuebe - a["schuebe"]  # die Schuebe seit dem letzten Rahmen sind bekannt, nichts wird geraten
            a["schuebe"] = s.term.schuebe
            schub = (0, s.term.ze - 1, n) if 0 < n < s.term.ze - 1 else None
            alt = list(a["gesendet"])
            if schub:
                o, u, n = schub
                alt[o:u] = alt[o + n:u] + [""] * n
            geaendert = {i: rows[i] for i in range(len(rows)) if rows[i] != alt[i]}
            a["stand"] += 1
            self.raster(rahmen_bauen(nr, a["stand"], None, schub, cur if cur != a["cursor"] else None, geaendert))
        a["gesendet"], a["cursor"] = rows, cur


class Sitzung:
    def __init__(self, sid, start, such_prozess, env, bereit_ab, groesse=(120, 40), bei_aenderung=None):
        self.sid, self.start, self.such, self.env, self.bereit_ab = sid, start, such_prozess, env, bereit_ab
        self.term = Terminal(*groesse)
        self.lock = threading.RLock()
        self.beendet = False
        self.code = 0
        self.eingabe = ""
        self.gezeigt = 0  # so viele Zeichen von `eingabe` stehen schon im Bild (Echo beim Ereignis `Text`)
        self.feld = None  # Zeile (ab Anfang, mit Schub gezaehlt) des Promptzeichens, solange darin ungesendeter Text stehen kann
        self.bei_aenderung = bei_aenderung or (lambda s: None)

    def schreibe(self, text):
        with self.lock:
            self.term.schreibe(text)
            self.bei_aenderung(self)

    def feld_setzen(self):
        """Merkt die Zeile des Cursors als Eingabefeld (dort steht das Promptzeichen)."""
        with self.lock:
            self.feld = self.term.schuebe + self.term.z

    def abschicken(self, rest):
        """Enter: Das Promptzeichen der Zeile des Felds wird `›` (die abgeschickte Zeile steht im Verlauf), dann `rest` und ein Zeilenende.
        Nachgebildet: Wie die echte CLI 1.0.88 die abgeschickte Zeile zeigt, ist nicht gemessen; gemessen ist nur, dass ihr Feld danach leer
        ist (22.11.)."""
        with self.lock:
            if self.feld is not None:
                i = self.feld - self.term.schuebe
                if 0 <= i < len(self.term.rows) and self.term.rows[i][0] == "\u276f":
                    self.term.rows[i][0] = "\u203a"
                self.feld = None
            self.schreibe(rest + "\n")

    def fuss_setzen(self, text):
        with self.lock:
            self.term.fuss = text
            self.bei_aenderung(self)

    def verlauf(self):
        """Alles, was bisher im Bild stand, nur fuer die Hilfswege der Attrappen-Seite: das aktuelle Bild."""
        with self.lock:
            return self.term.bild()


class Kern:
    def __init__(self, k):
        self.k = k
        self.fehler = set(k.get("fehler", []))
        self.lock = threading.Lock()
        self.sitzungen = {}
        self.einmalig = set()
        self.zaehler = {"kinder_gestartet": 0, "harness_gestartet": 0}
        self.n = 0
        self.verbindungen = []
        self.verb_lock = threading.Lock()
        g = k.get("groesse") or [120, 40]
        self.groesse = (int(g[0]), int(g[1]))
        self.deflate = bool(k.get("deflate", True))
        self.seite = None  # {Pfad: (Bytes, Typ)} aus `seite_ordner`; None = seite.html wie bisher
        self.seite_beleg = seitenordner.beleg_html(os.path.join(os.path.dirname(os.path.abspath(__file__)), "seite.html"))
        if k.get("seite_ordner"):
            self.seite = seitenordner.lade(k["seite_ordner"])  # SeitenFehler beendet den Start (main)
            self.seite_beleg = {"name": os.path.basename(os.path.realpath(k["seite_ordner"])),
                                "sha256": seitenordner.pruefsummen(self.seite)}

    def einstellungen(self):
        with open(self.k["einstellungen"], encoding="utf-8") as f:
            return json.load(f)

    def einmal(self, name):
        with self.lock:
            if name in self.einmalig:
                return False
            self.einmalig.add(name)
            return True

    def zuletzt_gestartete_route(self, eigene):
        with self.lock:
            lebende = [s for s in self.sitzungen.values() if not s.beendet and s.such]
        return lebende[-1].start["route"] if lebende else eigene

    # --- Start --------------------------------------------------------------------------------------
    def starten(self, roh):
        einst = self.einstellungen()
        fehl = pruefe_start(roh, einst, self.k["harness"], self.fehler)
        if fehl:
            if "s1-kind-trotz-ablehnung" in self.fehler:
                with self.lock:
                    self.zaehler["kinder_gestartet"] += 1
            return 422, {"grund": "start_pruefung", "code": fehl[0], "text": fehl[1]}
        start = json.loads(roh.decode("utf-8"))
        if not os.path.isdir(start["arbeitsordner"]):
            return self._abgewiesen(start, "lauf.ordner_fehlt", "Arbeitsordner fehlt oder ist kein Ordner")
        abw = self.ordner_pruefen(start, einst) if any(r["schluessel"] == start["route"] for r in einst["routen"]) else None
        if abw:
            return self._abgewiesen(start, abw[0], abw[1])
        if any(r["schluessel"] == start["route"] for r in einst["routen"]):
            self.ordner_markieren(start, einst)
        with self.lock:
            self.n += 1
            sid = "s-%d" % self.n
            self.zaehler["kinder_gestartet"] += 1
        vorlage = self.k.get("marker_start_datei")
        if vorlage:  # wie der Kern: start.json der Sitzung mit `marker` (Liste und Zaehlordner)
            pfad_s = vorlage.replace("{sitzung}", sid)
            os.makedirs(os.path.dirname(pfad_s), mode=0o700, exist_ok=True)
            liste = list(self.k["marker"].values())
            if "startdatei-falsche-markerliste" in self.fehler:
                liste = liste[:-1]
            with open(pfad_s, "w", encoding="utf-8") as f:
                json.dump({"marker": {"liste": liste, "ordner": self.k["zaehlordner"]}}, f)
        env = {"HOME": "/nonexistent", "PATH": "/usr/bin:/bin", "TERM": "xterm-256color", "LANG": "C.UTF-8",
               "MODELL_URL": self.k["modell_url"]}
        if "s1-route-umgebung" in self.fehler:
            env["BRAIN_ROUTE"] = start["route"]
        if start["harness"] == "pi" and "e4-lokal-env-blind" not in self.fehler:
            env["PI_OFFLINE"] = "1"  # Vertrag 6: das Programm setzt PI_OFFLINE=1 und PI_TELEMETRY=0 nur fuer pi
            env["PI_TELEMETRY"] = "0"
        if self.k["harness"][start["harness"]]["art"] == "cloud":
            env["COPILOT_GITHUB_TOKEN"] = self.k["cloud_token"] if "e4-cloud-token-lesbar" in self.fehler else self.k["cloud_platzhalter"]
            env["HTTPS_PROXY"] = "http://127.0.0.1:%d" % self.k["gastpunkt"]["cloud"]
        if "e4-schluessel-in-umgebung" in self.fehler and self.k["harness"][start["harness"]]["art"] == "lokal":
            env["GATEWAY_KEY"] = self.k.get("gateway_schluessel", "")
        lokal = self.k["harness"][start["harness"]]["art"] == "lokal"
        if "schluessel-im-arbeitsordner" in self.fehler and lokal:  # der Agent legt den Schluessel in eine Datei seines Ordners
            with open(os.path.join(start["arbeitsordner"], "notiz-gateway.txt"), "w", encoding="utf-8") as f:
                f.write(self.k.get("gateway_schluessel", ""))
        sp = None
        if any(r["schluessel"] == start["route"] for r in einst["routen"]):
            ordner = os.path.join(self.k["socket_wurzel"], sid)
            os.makedirs(ordner, mode=0o700)
            sp = such.SuchProzess(self.k["vault"], os.path.join(ordner, "such.sock"), start["route"], sid,
                                  self.k["einstellungen"], self.k["protokoll"],
                                  self.k["zaehlordner"], self.k["marker"], self.fehler, self, env)
            sp.starten()
        dauer = float(self.k.get("start_dauer_s", 0.2))
        if "s1-eingabefeld-spaet" in self.fehler:
            dauer += float(self.k.get("fehler_verzoegerung_s", 2.5))
        s = Sitzung(sid, start, sp, env, time.time() + dauer, self.groesse, self.bild_verteilen)
        ende = self.k["harness"][start["harness"]].get("sim_ende")  # Konfiguration (kein Fehlername): der Harness endet vor dem ersten Bild
        if ende:
            s.schreibe(ende["text"])
            s.beendet, s.code = True, int(ende["code"])
            if sp:  # der Harness ist tot: die Suche der Sitzung endet mit ihm
                sp.beenden()
                try:
                    os.rmdir(os.path.dirname(sp.socket_pfad))
                except OSError:
                    pass
            with self.lock:
                self.sitzungen[sid] = s
            self.liste_senden()
            return 200, {"sitzung": sid}
        s.schreibe("Sim-Harness %s, Route %s\n" % (start["harness"], start["route"]))
        if "schluessel-in-ausgabe" in self.fehler and lokal:
            s.schreibe("Gateway-Schluessel: %s\n" % self.k.get("gateway_schluessel", ""))
        if "ca-cli-lehnt-ab" in self.fehler and self.k["harness"][start["harness"]]["art"] == "cloud":
            s.schreibe("Error: unable to verify the first certificate (SELF_SIGNED_CERT_IN_CHAIN)\n")
        with self.lock:
            self.sitzungen[sid] = s
        threading.Timer(dauer, self._harness_bereit, args=(s,)).start()
        self.liste_senden()
        return 200, {"sitzung": sid}


    # --- Ordnermarke (A156, Vertrag 1 "Ordnermarke"): Alle Marken stehen in EINER Datei `ordnermarken.json` im Zustandsordner,
    # `{"fassung": 1, "marken": [{"ordner", "kunde", "klasse_hoch", "kennung"}]}` (`kennung`: addendum of 20.11.2026,
    # 32 Zeichen hex, entsteht beim Setzen). Geschrieben wird atomar (neue Datei, fsync, rename) unter flock auf
    # `ordnermarken.lock`. Fehlt die Datei, gibt es keine Marke; jede andere Abweichung ist `lauf.ordner_marke_kaputt` (fail
    # closed: kein Start, in keinem Ordner).
    def _markendatei(self):
        return os.path.join(self.k["zustandsordner"], "ordnermarken.json")

    def _marken_lesen(self, klassen):
        """Gibt (Liste der Marken, None) oder (None, Rohbytes) bei Beschaedigung. Fehlt die Datei: ([], None)."""
        try:
            with open(self._markendatei(), "rb") as f:
                roh = f.read()
        except FileNotFoundError:
            return [], None
        except OSError:
            return None, b""
        try:
            if len(roh) > 1048576 or roh.startswith(b"\xef\xbb\xbf"):
                raise ValueError
            d = json.loads(roh.decode("utf-8"), object_pairs_hook=_doppelt)
            if not (isinstance(d, dict) and set(d) == {"fassung", "marken"} and d["fassung"] == 1
                    and not isinstance(d["fassung"], bool) and isinstance(d["marken"], list) and len(d["marken"]) <= 4096):
                raise ValueError
            gesehen = set()
            for m in d["marken"]:
                if not (isinstance(m, dict) and set(m) == {"ordner", "kunde", "klasse_hoch", "kennung"}):
                    raise ValueError
                if not (isinstance(m["ordner"], str) and m["ordner"].startswith("/") and m["ordner"] not in gesehen
                        and (m["kunde"] is None or (isinstance(m["kunde"], str) and KENNUNG.match(m["kunde"])))
                        and m["klasse_hoch"] in klassen and isinstance(m["kennung"], str)
                        and re.match(r"^[0-9a-f]{32}$", m["kennung"])):
                    raise ValueError
                gesehen.add(m["ordner"])
            return d["marken"], None
        except (ValueError, UnicodeDecodeError):
            return None, roh

    def _marken_schreiben(self, marken):
        """Atomar: neue Datei, fsync, rename; der Aufrufer haelt die Sperre."""
        ziel = self._markendatei()
        roh = json.dumps({"fassung": 1, "marken": marken}, sort_keys=True, separators=(",", ":")).encode("utf-8")
        fd = os.open(ziel + ".neu", os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(roh)
            f.flush()
            os.fsync(f.fileno())
        os.replace(ziel + ".neu", ziel)

    def _marken_sperre(self):
        import fcntl
        os.makedirs(self.k["zustandsordner"], mode=0o700, exist_ok=True)
        f = open(os.path.join(self.k["zustandsordner"], "ordnermarken.lock"), "a")
        fcntl.flock(f.fileno(), fcntl.LOCK_EX)
        return f

    def ordner_pruefen(self, start, einst):
        """Gibt None (Start erlaubt) oder (Code, Grund). Reihenfolge nach Vertrag 1: kaputt, anderer Kunde, hoeher markiert; die
        strengste Marke aus dem Ordner, allen Vorfahren und allen Nachfahren mit Marke gilt (aufgeloester Pfad, ganzteilig)."""
        if "e6-marke-fehlt" in self.fehler:
            return None
        pfad = os.path.realpath(start["arbeitsordner"])
        klassen = einst["klassen"]
        route = next(r for r in einst["routen"] if r["schluessel"] == start["route"])
        mein = klassen.index(route["klasse"])
        sperre = self._marken_sperre()
        try:
            marken, roh = self._marken_lesen(klassen)
        finally:
            sperre.close()
        if marken is None:
            if "e6-marke-beschaedigt-start" in self.fehler:
                return None  # Mutationsprobe: eine beschaedigte Marke wird uebergangen (fail open)
            if "e6-kaputt-nur-eigener-ordner" in self.fehler and pfad.encode("utf-8") not in (roh or b""):
                return None  # Mutationsprobe: nur der Ordner, den die kaputte Datei nennt, wird abgewiesen
            return ("lauf.ordner_marke_kaputt", "Ordnermarke unlesbar oder beschaedigt")
        familie = []
        for d in marken:
            mp = d["ordner"]
            vorfahr = pfad.startswith(mp + os.sep)
            nachfahr = mp.startswith(pfad + os.sep)
            if mp == pfad or nachfahr or (vorfahr and "e6-vorfahr-nicht-geprueft" not in self.fehler):
                familie.append(d)
        for d in familie:
            if d["kunde"] != start["kunde"] and "e6-kunde-nicht-verglichen" not in self.fehler:
                return ("lauf.ordner_anderer_kunde", "Ordner gehoert einem anderen Kunden")
        for d in familie:
            if mein < klassen.index(d["klasse_hoch"]):
                return ("lauf.ordner_hoeher_markiert", "Ordner hoeher markiert als die Route")
        return None

    def ordner_markieren(self, start, einst):
        if "e6-marke-fehlt" in self.fehler:
            return
        import secrets
        pfad = os.path.realpath(start["arbeitsordner"])
        klassen = einst["klassen"]
        route = next(r for r in einst["routen"] if r["schluessel"] == start["route"])
        sperre = self._marken_sperre()
        try:
            marken, _ = self._marken_lesen(klassen)
            if marken is None:
                return  # beschaedigte Datei (nur im Fehlerfall bis hierher): nie ueberschreiben
            alt = next((m for m in marken if m["ordner"] == pfad), None)
            klasse = route["klasse"]
            if alt and klassen.index(alt["klasse_hoch"]) > klassen.index(klasse):
                klasse = alt["klasse_hoch"]  # die Marke steigt nur
            neu = {"ordner": pfad, "kunde": start["kunde"], "klasse_hoch": klasse,
                   "kennung": alt["kennung"] if alt else secrets.token_hex(16)}
            self._marken_schreiben([m for m in marken if m["ordner"] != pfad] + [neu])
        finally:
            sperre.close()

    def _abgewiesen(self, start, code, grund):
        """Lauf starten weist ab, vor jedem Kind: Die Sitzung entsteht und endet mit dem Code im Terminal."""
        with self.lock:
            self.n += 1
            sid = "s-%d" % self.n
            self.zaehler["kinder_gestartet"] += 1  # das Startprogramm lief, ein Harness nicht
        s = Sitzung(sid, start, None, {}, time.time(), self.groesse, self.bild_verteilen)
        s.beendet, s.code = True, 65
        s.schreibe("%s: %s\n" % (code, grund))
        with self.lock:
            self.sitzungen[sid] = s
        self.liste_senden()
        return 200, {"sitzung": sid}

    def _harness_bereit(self, s):
        with self.lock:
            self.zaehler["harness_gestartet"] += 1
        s.schreibe("\n❯ ")
        s.feld_setzen()
        s.fuss_setzen(self.k["harness"][s.start["harness"]]["sim_zeile"])

    def beenden(self, sid):
        s = self.sitzungen.get(sid)
        if not s:
            return 404, {"error": "unbekannt"}
        s.beendet, s.code = True, 129  # SIGHUP, wie der Dienst (DI12 Punkt 6)
        if s.such:
            s.such.beenden()
            try:
                os.rmdir(os.path.dirname(s.such.socket_pfad))
            except OSError:
                pass
        self.liste_senden()
        return 200, {"ok": True}

    def beenden_alle(self):
        for sid in list(self.sitzungen):
            if not self.sitzungen[sid].beendet:
                self.beenden(sid)

    # --- Strom: Verbindungen, Liste, Bilder --------------------------------------------------------------------
    def liste_nachricht(self):
        with self.lock:
            sitzungen = list(self.sitzungen.values())
        liste = []
        for s in sitzungen:
            e = {"id": s.sid, "harness": s.start["harness"], "modell": s.start["modell"], "route": s.start["route"],
                 "kunde": s.start["kunde"] or "", "zustand": "beendet" if s.beendet else "laeuft"}
            if s.beendet:
                e["code"] = s.code
            liste.append(e)
        return {"art": "Sitzungsliste", "sitzungen": liste}

    def startwahl_nachricht(self):
        einst = self.einstellungen()
        return {"art": "Startwahl", "fassung": 1,
                "harness": [{"name": n, "art": h["art"], "routenart": h["art"]} for n, h in sorted(self.k["harness"].items())],
                "routen": [{"schluessel": r["schluessel"], "art": r["art"], "klasse": r["klasse"], "modell": r.get("modell")} for r in einst["routen"]],
                "kunden": [x["schluessel"] for x in einst["kunden"]] + [None]}

    def liste_senden(self):
        m = self.liste_nachricht()
        with self.verb_lock:
            vs = list(self.verbindungen)
        for v in vs:
            v.nachricht(6, m)

    def verbunden(self):
        v = Verbindung(self.deflate)
        with self.verb_lock:
            self.verbindungen.append(v)
        v.nachricht(6, self.liste_nachricht())
        try:
            v.nachricht(6, self.startwahl_nachricht())
        except (OSError, ValueError, KeyError):
            pass
        return v

    def getrennt(self, v):
        v.zu = True
        with self.verb_lock:
            if v in self.verbindungen:
                self.verbindungen.remove(v)

    def bild_verteilen(self, s):
        """Wird mit `s.lock` aufgerufen: jede Verbindung mit Abo auf die Sitzung bekommt den neuen Stand."""
        with self.verb_lock:
            vs = list(self.verbindungen)
        for v in vs:
            v.bild_senden(s)

    def verbindung(self, kennung):
        with self.verb_lock:
            return next((v for v in self.verbindungen if v.kennung == kennung), None)

    def ereignis(self, v, e):
        """Ein Ereignis der Ansicht (Vertrag dienst DI3 und DI12). Die Antwort des Dienstes (Ablehnung) geht nur an die Verbindung."""
        art = e.get("art")
        sid = e.get("sitzung")
        if art not in ("Abo", "Groesse", "Text", "Taste", "Beenden"):  # `Groesse`: Vertrag dienst DI3, erlaubte Arten der Ansicht
            v.ablehnung("art_nicht_erlaubt")
            return
        s = self.sitzungen.get(sid) if isinstance(sid, str) else None
        if s is None:
            v.ablehnung("sitzung_unbekannt")
        elif art == "Abo":
            sp, ze = e.get("spalten"), e.get("zeilen")
            if e.get("an", True) is False:
                v.abos.pop(sid, None)
            elif not (isinstance(sp, int) and isinstance(ze, int) and 2 <= sp <= 1000 and 1 <= ze <= 1000):
                v.ablehnung("groesse_ungueltig")
            elif sid not in v.abos:
                v.abos[sid] = {"gesendet": None, "cursor": None, "stand": 0, "schuebe": 0}
                with s.lock:
                    v.bild_senden(s)
        elif art == "Groesse":
            # Vertrag dienst DI3: `spalten` unter 2 lehnt mit `groesse_ungueltig` ab; `sitzung` und `zeilen` wie beim `Abo` (hergeleitet aus der
            # Messung der Ansicht 2a, nicht aus dem Vertragstext). Ohne Abo gibt es keine Groesse. Das Bild der Attrappe aendert sich nicht.
            sp, ze = e.get("spalten"), e.get("zeilen")
            if not (isinstance(sp, int) and isinstance(ze, int) and 2 <= sp <= 1000 and 1 <= ze <= 1000):
                v.ablehnung("groesse_ungueltig")
            elif sid not in v.abos:
                v.ablehnung("kein_abo")
        elif art == "Beenden":
            if s.beendet:
                v.ablehnung("sitzung_beendet")
            else:
                self.beenden(sid)
        elif sid not in v.abos:
            v.ablehnung("kein_abo")
        elif s.beendet:
            v.ablehnung("sitzung_beendet")
        elif art == "Text":
            t = e.get("text")
            if not isinstance(t, str) or any(ord(c) < 0x20 or 0x7F <= ord(c) <= 0x9F for c in t):
                v.ablehnung("steuerzeichen_im_text")
            else:
                with s.lock:
                    s.eingabe += t
                    if time.time() >= s.bereit_ab:  # wie die echte CLI: der Text steht vor dem Enter im Bild (Enter mit Abstand)
                        s.schreibe(s.eingabe[s.gezeigt:])
                        s.gezeigt = len(s.eingabe)
        elif art == "Taste":
            if e.get("taste") != "Enter" or e.get("umschalter") not in ([], None):
                v.ablehnung("taste_unbekannt")
            else:
                with s.lock:
                    text, rest, s.eingabe, s.gezeigt = s.eingabe, s.eingabe[s.gezeigt:], "", 0
                self.eingabe(s, text, v, rest)

    def eingabe(self, s, text, v=None, rest=None):
        """Enter mit dem getippten `text`; `rest` ist der Teil, der noch nicht im Bild steht (Vorgabe: alles)."""
        if time.time() < s.bereit_ab:
            if v:
                v.ablehnung("harness_nicht_bereit")  # nur die Attrappe: das Eingabefeld ist noch nicht da
            return
        s.abschicken(text if rest is None else rest)
        if text:
            threading.Thread(target=self._zug, args=(s, text), daemon=True).start()

    # --- Agent ----------------------------------------------------------------------------------------
    def _art(self, s):
        return self.k["harness"][s.start["harness"]]["art"]

    def _modell(self, s, nachrichten):
        """Fragt das Modell der Sitzung. Cloud-Art: Anthropic-Form, lokale Art: OpenAI-kompatibel (chat/completions).
        Gibt (Liste von Bloecken, wieder_aufrufen) mit Bloecken ("text", s) und ("tool_use", id, name, eingabe)."""
        lokal = self._art(s) == "lokal"
        url = urlparse(self.k["modell_url_lokal"] if lokal else self.k["modell_url"])
        c = http.client.HTTPConnection(url.hostname, url.port, timeout=15)
        werkzeuge = [self.k["mcp_werkzeug"], "Bash"]
        if lokal:
            pfad = "/v1/chat/completions"
            body = {"model": s.start["modell"], "stream": False, "messages": nachrichten,
                    "tools": [{"type": "function", "function": {"name": n, "parameters": {"type": "object"}}} for n in werkzeuge]}
        else:
            pfad = "/v1/messages"
            body = {"model": s.start["modell"], "max_tokens": 1024, "stream": False, "messages": nachrichten,
                    "tools": [{"name": n, "input_schema": {"type": "object"}} for n in werkzeuge]}
        c.request("POST", pfad, json.dumps(body).encode(), {"content-type": "application/json", "x-api-key": "sk-attrappe",
                                                              "authorization": "Bearer " + (self.k["cloud_platzhalter"] if not lokal else "sk-attrappe"), "anthropic-version": "2023-06-01",
                                                              "x-sitzung": s.sid})  # nur die Attrappe: der Weiterreicher ordnet damit zu
        antwort = json.loads(c.getresponse().read().decode("utf-8"))
        c.close()
        if not lokal:
            return [("text", b["text"]) if b["type"] == "text" else ("tool_use", b["id"], b["name"], b["input"])
                    for b in antwort.get("content", [])], antwort.get("content", []), antwort.get("stop_reason") == "tool_use"
        m = antwort["choices"][0]["message"]
        blocks = ([("text", m["content"])] if m.get("content") else []) + \
            [("tool_use", t["id"], t["function"]["name"], json.loads(t["function"]["arguments"])) for t in m.get("tool_calls", [])]
        return blocks, m, antwort["choices"][0]["finish_reason"] == "tool_calls"

    def _zug(self, s, text):
        try:
            lokal = self._art(s) == "lokal"
            nachrichten = [{"role": "user", "content": text}]  # je Zug ein frischer Kontext (siehe README)
            for _ in range(6):
                blocks, roh, nochmal = self._modell(s, nachrichten)
                ergebnisse = []
                for b in blocks:
                    if b[0] == "text":
                        s.schreibe(b[1] + "\n")
                    else:
                        rc, out, fehlerhaft = self._werkzeug(s, b[2], b[3])
                        s.schreibe("\u25cf %s\n  \u23bf %s\n" % (b[2], out.strip().replace("\n", "\n    ")))
                        if "s3-ausgabe-zeigt-marker" in self.fehler and b[2] == self.k["mcp_werkzeug"] and self.einmal("s3-ausgabe-zeigt-marker"):
                            s.schreibe("  (Debug: Notiz %s)\n" % self.k["marker"]["nk"])
                        if "s3-ausgabe-marker-klein" in self.fehler and b[2] == self.k["mcp_werkzeug"] and self.einmal("s3-ausgabe-marker-klein"):
                            s.schreibe("  (Debug: Notiz %s)\n" % self.k["marker"]["nk"].lower())
                        ergebnisse.append((b[1], out, fehlerhaft))
                if not nochmal or not ergebnisse:
                    break
                if lokal:
                    nachrichten = nachrichten + [roh] + [{"role": "tool", "tool_call_id": i, "content": o} for i, o, _ in ergebnisse]
                else:
                    nachrichten = nachrichten + [{"role": "assistant", "content": roh}, {"role": "user", "content": [
                        {"type": "tool_result", "tool_use_id": i, "content": o, "is_error": f} for i, o, f in ergebnisse]}]
        except Exception as e:  # der Agent der Attrappe darf nie den Dienst beenden
            s.schreibe("Fehler im Agenten: %s\n" % e)
        s.schreibe("\n" + self.k["harness"][s.start["harness"]]["sim_zeile"])

    def _werkzeug(self, s, name, eingabe):
        if name == self.k["mcp_werkzeug"]:
            return self._mcp(s, eingabe)
        befehl = eingabe.get("command", "")
        suffix = '; echo "RC=$?"'
        if not befehl.endswith(suffix):
            return 0, "nicht unterstuetzt\n", True
        rc, out = self._einschluss(s, befehl[:-len(suffix)])
        return rc, out + ("" if out.endswith("\n") or not out else "\n") + "RC=%d\n" % rc, rc != 0

    def _mcp(self, s, eingabe):
        if not s.such or set(eingabe) - {"query", "k"} or not isinstance(eingabe.get("query"), str):
            return 1, "Ungueltige Anfrage", True
        antwort = self._socket_zeile(s, dict({"op": "search"}, **eingabe))
        return 0, antwort, '"error"' in antwort

    def _socket_zeile(self, s, anfrage):
        c = socket.socket(socket.AF_UNIX)
        c.settimeout(10)
        c.connect(s.such.socket_pfad)
        c.sendall((json.dumps(anfrage) + "\n").encode())
        zeile = c.makefile("rb").readline().decode("utf-8").rstrip("\n")
        c.close()
        return zeile

    # --- nachgebildeter Einschluss: nur die Befehle des Laeufers ----------------------------------------
    def _einschluss(self, s, befehl):
        if befehl.startswith("z=$("):  # Buendel: `z=$(<befehl>); echo "<name>=$z RC=$?"` mehrfach, mit `; ` verbunden
            zeilen = []
            for teil in befehl[len("z=$("):].split("; z=$("):
                cmd, _, rest = teil.rpartition('); echo "')
                rc_, out_ = self._einschluss(s, cmd)
                zeilen.append("%s=%s RC=%d" % (rest.split("=$z RC=", 1)[0], " ".join(out_.split()), rc_))
            return 0, "\n".join(zeilen) + "\n"
        try:
            a = [t for t in shlex.split(befehl) if not t.startswith(("2>", ">"))]
        except ValueError:
            return 2, "bash: Syntaxfehler\n"
        if not a:
            return 0, ""
        vault = os.path.realpath(self.k["vault"])
        if " | " in befehl:  # `<befehl> | grep ...` und `... | tr ...`, von rechts her aufgeloest
            links, rechts = befehl.rsplit(" | ", 1)
            rc, out = self._einschluss(s, links)
            g = [t for t in shlex.split(rechts) if not t.startswith(("2>", ">"))]
            if g and g[0] == "tr":
                return rc, out.replace("\x00", "\n")
            flags = [t for t in g[1:] if t.startswith("-")]
            muster = next((t for t in g[1:] if not t.startswith("-")), "")
            nur_treffer = any("o" in f and not f.startswith("--") for f in flags)
            invers = any("v" in f and not f.startswith("--") for f in flags)
            gross = re.I if any("i" in f and not f.startswith("--") for f in flags) else 0
            if nur_treffer:
                treffer = [m.group(0) for l in out.splitlines() for m in re.finditer(muster, l, gross)]
            else:
                treffer = [l for l in out.splitlines() if (re.search(muster, l, gross) is None) == invers]
            if any("c" in f and not f.startswith("--") for f in flags):
                return (0 if treffer else 1), "%d\n" % len(treffer)
            return (0 if treffer else 1), "\n".join(treffer) + ("\n" if treffer else "")
        m = re.match(r"^printf '([^']*)%s' (\S+) > (\S+)$", befehl)
        if m:  # Datei im Arbeitsordner der Sitzung schreiben (der Arbeitsordner ist im Einschluss eingebunden)
            ziel = os.path.realpath(m.group(3))
            if ziel.startswith(os.path.realpath(s.start["arbeitsordner"]) + os.sep):
                with open(ziel, "w", encoding="utf-8") as f:
                    f.write(m.group(1) + m.group(2))
                return 0, ""
            return 1, "bash: %s: Permission denied\n" % m.group(3)
        if a[0] == "cat" and len(a) == 2 and os.path.realpath(a[1]).startswith(os.path.realpath(s.start["arbeitsordner"]) + os.sep) \
                and os.path.isfile(a[1]):
            with open(a[1], encoding="utf-8") as f:
                return 0, f.read()
        if a[0] == "env":
            # Fehler `e4-cloud-env-blind`: die Shell sieht die Umgebung ohne den Platzhalter (die Umgebungssuche zaehlt ihn nicht)
            return 0, "".join("%s=%s\n" % kv for kv in sorted(s.env.items())
                              if not ("e4-cloud-env-blind" in self.fehler and kv[0] == "COPILOT_GITHUB_TOKEN"))
        if a[0] == "export":
            for t in a[1:]:
                k, _, v = t.partition("=")
                s.env[k] = v
            return 0, ""
        if a[0] == "cat":
            ziel = a[-1]
            if "/proc/[0-9]*/environ" in a[1:] and "e4-cloud-proc-blind" not in self.fehler \
                    and all(t in ("/proc/[0-9]*/environ", "/proc/[0-9]*/cmdline") for t in a[1:] if t.startswith("/proc/")):
                # Startumgebung der Prozesse der Sitzung (nur der Glob, den token_proc und kontrolle_proc lesen); NUL trennt die Eintraege
                return 0, "".join("%s=%s\x00" % kv for kv in sorted(s.env.items()))
            if any(t.startswith("/proc/") for t in a[1:]) and not ("e3-suche-sichtbar" in self.fehler and a[-1].endswith("/777/environ")):
                return 1, "cat: /proc/[0-9]*/environ: Permission denied\n"
            if "s2-datei-lesbar" in self.fehler:
                treffer = sorted(glob.glob(ziel))
                if treffer:
                    with open(treffer[0], encoding="utf-8") as f:
                        return 0, f.read()
            m = re.match(r"^/proc/(\d+)/environ$", ziel)
            if m and "e3-suche-sichtbar" in self.fehler and m.group(1) == "777":
                return 0, "ROUTE=%s\n" % s.start["route"]
            return 1, "cat: %s: No such file or directory\n" % ziel
        if a[0] == "curl":
            mit_arg = {"-o", "-w", "-d", "-H", "-X", "-m", "--max-time", "--proxy", "--noproxy"}
            proxy, url, ausgabe_code, i = None, "", False, 1
            while i < len(a):
                t = a[i]
                if t in mit_arg and i + 1 < len(a):
                    if t == "--proxy":
                        proxy = a[i + 1]
                    if t == "-w":
                        ausgabe_code = True
                    i += 2
                    continue
                if t.startswith("http") and not url:
                    url = t
                i += 1
            u = urlparse(url)
            art = self._art(s)
            eigen = self.k["gastpunkt"][art]
            def antwort(rc, code, fehler=""):
                return rc, (code if ausgabe_code else "") + ("" if rc == 0 else "curl: (%d) %s\n" % (rc, fehler))
            if proxy:  # CONNECT ueber den Modellweg
                if urlparse(proxy).port != eigen or art != "cloud":
                    return antwort(7, "000", "Failed to connect to proxy")
                if u.hostname in self.k["netzziele"] or "e5-connect-alles" in self.fehler:
                    return antwort(0, "200")
                return antwort(56, "000", "CONNECT tunnel failed, response 403")
            if u.hostname == "127.0.0.1" and u.port == eigen:  # der eigene Gastpunkt
                return antwort(0, "400")
            freigabe = (("s2-netz-offen" in self.fehler and url.startswith(self.k.get("fremd_url", "\0")))
                        or ("e4-direkt-offen" in self.fehler and url.startswith(self.k.get("gateway_url", "\0")))
                        or ("e4-cloud-direkt-offen" in self.fehler and url.startswith(self.k.get("cloud_ziel_url", "\0")))
                        or ("e5-port-offen" in self.fehler and url.startswith(self.k.get("bruecke_url", "\0"))))
            if freigabe:
                try:
                    r = urllib.request.Request(url, data=b"{}" if "-d" in a else None, method="POST" if "-d" in a else "GET")
                    with urllib.request.urlopen(r, timeout=5) as x:
                        return antwort(0, "%d" % x.status) if ausgabe_code else (0, x.read().decode("utf-8", "replace"))
                except urllib.error.HTTPError as e:
                    return antwort(0, "%d" % e.code)
                except OSError as e:
                    return antwort(7, "000", str(e))
            return antwort(7, "000", "Failed to connect to %s: Network is unreachable" % (u.netloc or url))
        if a[0] == "grep":
            muster = next((t for t in a[1:] if not t.startswith("-") and t != a[-1]), "")
            if "e2-notizen-sichtbar" in self.fehler:
                treffer = []
                for wurzel, _, dateien in os.walk(vault):
                    for n in dateien:
                        p = os.path.join(wurzel, n)
                        with open(p, encoding="utf-8", errors="replace") as f:
                            if re.search(muster, f.read()):
                                treffer.append(p)
                if treffer:
                    return 0, "\n".join(sorted(treffer)) + "\n"
            return 1, ""
        if a[0] == "ps" and any("args=" in t for t in a):
            return 0, "bash\nsim-harness --sim\nps -eo args=\n"
        if a[0] == "ps":
            z = ["    1 agent bash", "   12 agent harness", "   30 agent ps"]
            if "e3-suche-sichtbar" in self.fehler:
                z.append("  777 agent rangkern")
            return 0, "\n".join(z) + "\n"
        if a[0] == "kill":
            pid = a[-1]
            if "e3-suche-sichtbar" in self.fehler and pid == "777":
                return 0, ""
            return 1, "bash: kill: (%s) - No such process\n" % pid
        if a[0] == "python3" and "/run/brain/such.sock" in befehl:
            skript = a[a.index("-c") + 1] if "-c" in a[:-1] else ""
            m = re.search(r"sendall\(b'(.*?)\\n'\)", skript)
            if not m or not s.such:
                return 1, "python3: Fehler\n"
            return 0, self._socket_zeile_roh(s, m.group(1)) + "\n"
        return 127, "bash: %s: command not found\n" % a[0]

    def _socket_zeile_roh(self, s, text):
        c = socket.socket(socket.AF_UNIX)
        c.settimeout(10)
        c.connect(s.such.socket_pfad)
        c.sendall(text.encode() + b"\n")
        zeile = c.makefile("rb").readline().decode("utf-8").rstrip("\n")
        c.close()
        return zeile


def handler_fabrik(kern, token):
    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        # Alle Antworten tragen Connection: close und Cache-Control: no-store (Vertrag bruecke, Abschnitt 5); eine Ablehnung hat keinen Inhalt.
        def _kopf(self, code, typ=None, laenge=None, mehr=()):
            self.send_response(code)
            self.send_header("Connection", "close")
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            if typ:
                self.send_header("Content-Type", typ)
            if laenge is not None:
                self.send_header("Content-Length", str(laenge))
            for n, w in mehr:
                self.send_header(n, w)
            self.end_headers()
            self.close_connection = True

        def _leer(self, code):
            self._kopf(code, laenge=0)

        def _senden(self, code, obj):
            b = json.dumps(obj, ensure_ascii=False).encode("utf-8")
            self._kopf(code, "application/json", len(b))
            self.wfile.write(b)

        def _pruefe(self, post):
            """Reihenfolge nach Vertrag bruecke Abschnitt 4: Host (403), Origin (403), Token (401)."""
            port = self.server.server_address[1]
            if self.headers.get("Host") not in ("127.0.0.1:%d" % port, "localhost:%d" % port):
                return self._leer(403) or False
            o = self.headers.get("Origin")
            if (o is None and post) or (o is not None and o not in ("http://127.0.0.1:%d" % port, "http://localhost:%d" % port)):
                return self._leer(403) or False
            return True

        def _token(self):
            if not hmac.compare_digest((self.headers.get("Authorization") or "").encode("utf-8"), ("Bearer " + token).encode("utf-8")):
                return self._leer(401) or False
            return True

        def _koerper(self, grenze):
            """Inhaltstyp (415), Laenge (411, 413), JSON (400). Gibt (Bytes, Wert) oder None, wenn schon geantwortet ist."""
            typ = (self.headers.get("Content-Type") or "").split(";")[0].strip().lower()
            if typ != "application/json":
                return self._leer(415)
            n = self.headers.get("Content-Length")
            if n is None or not n.isdigit():
                return self._leer(411)
            if int(n) > grenze:
                return self._leer(413)
            roh = self.rfile.read(int(n))
            try:
                return roh, json.loads(roh.decode("utf-8"))
            except ValueError:
                return self._leer(400)

        def _sitzung(self, sid):
            s = kern.sitzungen.get(sid)
            if s is None:
                self._senden(404, {"error": "unbekannt"})
            return s

        def do_GET(self):
            if not self._pruefe(False):
                return
            u = urlparse(self.path)
            # genau drei Pfade wie in pruefung.rs; `self.path` hat die Standardbibliothek bei fuehrendem `//` schon zu `/` gemacht, darum die Anfragezeile
            roh = self.requestline.split(" ")[1] if self.requestline.count(" ") >= 2 else self.path
            if kern.seite is not None and roh in kern.seite:  # keine Datei von der Platte
                b, typ = kern.seite[roh]
                self._kopf(200, typ, len(b), (("Content-Security-Policy", seitenordner.CSP),))
                self.wfile.write(b)
                return
            if kern.seite is None and u.path in ("/", "/index.html"):  # die Seite selbst braucht kein Token; ihre Datenanfragen schon
                with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "seite.html"), "rb") as f:
                    b = f.read()
                self._kopf(200, "text/html; charset=utf-8", len(b))
                self.wfile.write(b)
                return
            if not self._token():
                return
            if self.path == "/strom":
                return self._strom()
            if u.path == "/diagnose":  # nur die Attrappe
                return self._senden(200, dict(kern.zaehler, sitzungen=len(kern.sitzungen), seite=kern.seite_beleg))
            teile = u.path.strip("/").split("/")
            if teile[0] == "attrappe":  # Hilfswege der Attrappen-Seite (Browser-Modus), nicht Teil des Protokolls
                return self._seite_get(teile[1:])
            if self.path in ("/starten", "/eingabe"):
                return self._leer(405)
            self._leer(404)

        def _strom(self):
            v = kern.verbunden()
            self._kopf(200, "application/octet-stream", None, (("X-Verbindung", v.kennung), ("X-Deflate", "1" if v.deflate else "0")))
            try:
                while not v.zu:
                    try:
                        d = v.q.get(timeout=0.2)
                    except queue.Empty:
                        continue
                    self.wfile.write(d)
                    self.wfile.flush()
            except OSError:
                pass
            finally:
                kern.getrennt(v)

        def do_POST(self):
            if not self._pruefe(True) or not self._token():
                return
            u = urlparse(self.path)
            if self.path == "/starten":
                k = self._koerper(16384)
                if k is None:
                    return
                code, obj = kern.starten(k[0])
                return self._senden(code, obj)
            if self.path == "/eingabe":
                kennung = self.headers.get("X-Verbindung") or ""
                if not re.match(r"^[0-9a-f]{32}$", kennung):
                    return self._leer(400)
                v = kern.verbindung(kennung)
                if v is None:
                    return self._leer(404)
                k = self._koerper(1048575)
                if k is None:
                    return
                if not isinstance(k[1], dict):
                    return self._leer(400)
                kern.ereignis(v, k[1])
                return self._leer(204)
            teile = u.path.strip("/").split("/")
            if teile[0] == "attrappe":
                return self._seite_post(teile[1:])
            if self.path in ("/strom", "/"):
                return self._leer(405)
            self._leer(404)

        # --- Hilfswege der Attrappen-Seite (seite.html): kein Teil des Protokolls der Bruecke ------------------------
        def _seite_get(self, teile):
            if teile == ["optionen"]:
                einst = kern.einstellungen()
                harness, routen = sorted(kern.k["harness"]), [r["schluessel"] for r in einst["routen"]]
                kunden = [x["schluessel"] for x in einst["kunden"]]
                if "startwahl-vertauscht" in kern.fehler:  # die Seite zeigt die Eintraege in anderer Reihenfolge
                    harness, routen, kunden = harness[::-1], routen[::-1], kunden[::-1]
                modelle = {r["schluessel"]: r.get("modell") for r in einst["routen"]}  # wie `routen[].modell` der Startwahl, null: keins
                return self._senden(200, {"harness": harness, "routen": routen, "kunden": kunden, "modelle": modelle,
                                          "arbeitsordner": kern.k.get("arbeitsordner", "/tmp/arbeit"),
                                          "kein_selbstwechsel": kern.k.get("kein_selbstwechsel") is True})
            if teile[:1] == ["sitzung"] and len(teile) in (2, 3):
                s = self._sitzung(teile[1])
                if s is None:
                    return
                if len(teile) == 2:
                    return self._senden(200, {"sitzung": s.sid, "zustand": "beendet" if s.beendet else "laeuft", "code": s.code})
                if teile[2] == "raster":
                    return self._senden(200, {"zeilen": s.verlauf(), "cursor": list(s.term.cursor())})
            self._leer(404)

        def _seite_post(self, teile):
            if teile[:1] == ["sitzung"] and len(teile) == 3 and teile[2] == "eingabe":
                s = self._sitzung(teile[1])
                if s is None:
                    return
                k = self._koerper(1048575)
                if k is None:
                    return
                try:
                    text = k[1]["text"]
                except (KeyError, TypeError):
                    return self._senden(400, {"error": "bad_request"})
                kern.eingabe(s, text)
                return self._senden(202, {"ok": True})
            self._leer(404)
    return H


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--konfig", required=True)
    ap.add_argument("--portdatei", required=True)
    ap.add_argument("--rolle", default="")
    a = ap.parse_args()
    with open(a.konfig, encoding="utf-8") as f:
        k = json.load(f)
    try:
        kern = Kern(k)
    except seitenordner.SeitenFehler as e:
        print("Seitenordner: Abbruch: %s" % e, file=sys.stderr)
        return 2
    srv = ThreadingHTTPServer(("127.0.0.1", 0), handler_fabrik(kern, k["token"]))
    srv.daemon_threads = True
    k["bruecke_url"] = "http://127.0.0.1:%d" % srv.server_address[1]

    def ende(*_):
        kern.beenden_alle()
        with kern.verb_lock:
            for v in kern.verbindungen:
                v.zu = True
        threading.Thread(target=srv.shutdown, daemon=True).start()
    signal.signal(signal.SIGTERM, ende)
    signal.signal(signal.SIGINT, ende)
    with open(a.portdatei, "w") as f:
        f.write(str(srv.server_address[1]))
    srv.serve_forever()
    srv.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
