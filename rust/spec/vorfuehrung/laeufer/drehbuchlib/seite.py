"""Bedienung der Browser-Seite der Bruecke mit Playwright (Nachtrag 3 und 4 zum Auftrag).

`SeiteBruecke` hat dieselbe Schnittstelle wie `bruecke.Bruecke`. Starten, Eingabe und Terminal laufen ueber die Seite
(`data-testid` aus der Konfiguration, `browser.selektoren`); Sitzungsliste, Beenden und Diagnose gehen ueber den Strom der Bruecke (`bruecke.Bruecke`), den dieser Client selbst haelt.
Der Browser laeuft kopflos mit eigenem Profilordner unter dem Arbeitsordner, nie mit einem Profil des Nutzers. Es wird eine
VORHANDENE Playwright-Installation benutzt (`browser.playwright_pfad`); nichts wird geladen oder installiert.
"""
from __future__ import annotations

import json
import os
import subprocess
import threading
import time

from . import messen
from .bruecke import Bild, Bruecke, Protokoll, schub_zwischen

HIER = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MODELL_FRIST_S = 10  # Frist, bis die Seite nach Harness und Route die Modellliste zeigt


class BrowserFehler(Exception):
    pass


def verfuegbar(k):
    """Gibt (ok, Grund). Prueft nur, ob Node und die Playwright-Installation da sind; startet nichts."""
    b = k.get("browser", {})
    pfad = os.path.expanduser(b.get("playwright_pfad", ""))
    if not pfad or not os.path.isfile(os.path.join(pfad, "package.json")):
        return False, "browser.playwright-fehlt"
    try:
        subprocess.run([b.get("node", "node"), "--version"], capture_output=True, timeout=10, check=True)
    except (OSError, subprocess.SubprocessError):
        return False, "browser.node-fehlt"
    return True, None


class Helfer:
    """Der Node-Prozess `browser.mjs` mit einem Zeilenprotokoll."""

    def __init__(self, k, profil):
        b = k["browser"]
        self.p = subprocess.Popen(
            [b.get("node", "node"), os.path.join(HIER, "browser", "browser.mjs"), os.path.expanduser(b["playwright_pfad"]), profil],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1,
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": os.environ.get("HOME", "/"),
                 "TMPDIR": os.path.dirname(profil)})
        self.n = 0
        self.lock = threading.Lock()
        self.profil = profil

    def rufe(self, op, frist_s=30, **args):
        with self.lock:
            self.n += 1
            m = dict(args, id=self.n, op=op)
            self.p.stdin.write(json.dumps(m) + "\n")
            self.p.stdin.flush()
            ende = time.monotonic() + frist_s
            antwort = {}
            def lies():
                antwort["z"] = self.p.stdout.readline()
            t = threading.Thread(target=lies, daemon=True)
            t.start()
            t.join(max(ende - time.monotonic(), 0.1))
            if t.is_alive() or not antwort.get("z"):
                raise BrowserFehler("keine Antwort auf %s" % op)
            r = json.loads(antwort["z"])
            if not r.get("ok"):
                messen.diagnose_notiere("seite.browser", "%s: %s" % (op, r.get("fehler")))  # Fehlertext des Browsers: nur in die Diagnosedatei
                raise BrowserFehler("%s: browser.fehler" % op)
            return r.get("wert")

    def schliessen(self):
        """Schliesst Browser und Helfer. Gibt Meldungen ueber Reste zurueck (leer: sauber)."""
        meld = []
        try:
            if self.p.poll() is None:
                self.rufe("schliessen", frist_s=15)
        except Exception:
            pass
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait(timeout=5)
            meld.append("Browser-Helfer musste beendet werden")
        # Reste mit dem eigenen Profilordner im Aufruf pruefen (nur unsere eigenen Prozesse)
        try:
            r = subprocess.run(["pgrep", "-f", self.profil], capture_output=True, timeout=10)
            if r.stdout.strip():
                meld.append("Browser-Prozesse mit eigenem Profil laufen noch")
        except (OSError, subprocess.TimeoutExpired):
            pass
        return meld


class SeiteBruecke(Bruecke):
    text_und_taste = False  # die Seite sendet ueber ihr Formular (`eingabe` unten), nicht ueber `Bruecke.text` und `Bruecke.taste`

    def __init__(self, k, b, arbeit):
        super().__init__(b)
        self.k = k
        self.sel = k["browser"]["selektoren"]
        self.profil = os.path.join(arbeit, "browser-profil")
        os.makedirs(self.profil, exist_ok=True)
        self.helfer = Helfer(k, self.profil)
        adresse = self.url + "/#t=" + (self.token or "")
        self.helfer.rufe("oeffnen", url=adresse, frist_s=60)
        self.verlaeufe = {}   # sid -> (Protokoll, letzte Zeilen der Seite)
        self.rahmen = 0
        self.aktuell = None

    # --- Auswahl der Sitzung ------------------------------------------------------------------------------------
    def _waehle_sitzung(self, sid):
        if self.aktuell != sid:
            self.helfer.rufe("klicken", testid=self.sel["eintrag"].replace("{id}", sid))
            self.aktuell = sid

    def _zeilen(self):
        return self.helfer.rufe("zeilen", praefix=self.sel["zeile"].replace("{n}", ""))

    def _angeboten(self, kennung, wert):
        """Wartet bis zur Frist, dass das select `kennung` die Option `wert` anbietet. Gibt an, ob sie erschien."""
        try:
            self.helfer.rufe("warteoption", frist_s=MODELL_FRIST_S + 10, testid=kennung, wert=wert, frist_ms=int(MODELL_FRIST_S * 1000))
            return True
        except BrowserFehler:
            return False

    def _waehle_nach_laden(self, kennung, wert):
        """Harness und Route: Die Seite laedt ihre Listen nach dem Oeffnen. Wer vorher waehlt, faelscht eine Option, die das Laden
        wieder loescht. Darum erst auf die Option warten; erscheint sie nicht, bleibt es bei der gefaelschten Eingabe des Clients.
        Gibt an, ob die Seite den Wert angeboten hat."""
        angeboten = wert != "" and self._angeboten(kennung, wert)
        self.helfer.rufe("waehlen", testid=kennung, wert=wert)
        return angeboten

    def _waehle_modell(self, wert, route_angeboten):
        """`modell-wahl` ist in der echten Seite ein select, dessen Liste von Harness und Route abhaengt (nach beiden aufrufen). Erst
        wenn die Seite die Option anbietet, wird sie gewaehlt; sonst endet der Schritt mit festem Text ohne Inhalt der Seite.
        Hat die Seite die Route nicht angeboten (Start ohne Route, unbekannte Route: Faelle des Drehbuchs), kennt sie keine Liste dafuer:
        dann bleibt es bei der Eingabe des Clients."""
        kennung = self.sel["modell"]
        if not route_angeboten:
            self.helfer.rufe("waehlen", testid=kennung, wert=wert)
            return
        if not self._angeboten(kennung, wert):
            raise BrowserFehler("Auswahl %s bietet den Wert %s nicht an (Frist %d s)" % (kennung, wert, MODELL_FRIST_S))
        self.helfer.rufe("waehlen", testid=kennung, wert=wert)

    # --- Schnittstelle von Bruecke ------------------------------------------------------------------------------
    def starten(self, start, ohne_token=False):
        if ohne_token or not isinstance(start, dict):
            return super().starten(start, ohne_token)  # ohne Token geht kein Browser
        if "route" not in start:
            # Die Seite schickt `route` immer mit (Vertrag der Seite SE3, Startformular) und hat keine Option ohne Route: einen Start ohne das
            # Feld kann sie nicht erzeugen. Die Gegenprobe prueft die Bruecke, nicht die Seite; sie geht darum ueber den Pruefclient.
            return super().starten(start, ohne_token)
        s = self.sel
        vorher = {e["id"] for e in self.helfer.rufe("liste", praefix=s["eintrag"].replace("{id}", ""))}
        self._waehle_nach_laden(s["harness"], start["harness"])
        route_angeboten = self._waehle_nach_laden(s["route"], start.get("route", ""))
        self._waehle_modell(start["modell"], route_angeboten)
        self.helfer.rufe("waehlen", testid=s["kunde"], wert=start["kunde"] or "")
        self.helfer.rufe("fuellen", testid=s["arbeitsordner"], wert=start["arbeitsordner"])
        # Die echte Seite blendet das Fehlerfeld erst bei einem Erfolg aus (Vertrag der Seite SE3), nicht beim Klick. Ein Fehler eines frueheren
        # Starts steht darum noch da, wenn der neue Start laeuft. Eine Meldung mit demselben Code und Text wie vor dem Klick zaehlt erst, wenn
        # nach der Frist nichts anderes kam.
        vor = self._fehler_der_seite()
        t0 = time.monotonic()
        self.helfer.rufe("klicken", testid=s["start"])
        ende = t0 + self.frist
        while time.monotonic() < ende:
            neu = [e["id"] for e in self.helfer.rufe("liste", praefix=s["eintrag"].replace("{id}", "")) if e["id"] not in vorher]
            if neu:
                # Die echte Seite wechselt nicht selbst in eine neu gestartete Sitzung (Vertrag der Seite): erst der Klick auf den
                # Eintrag waehlt sie und schickt das Abo. `aktuell` steht erst nach dem Klick; t0 bleibt der Klick auf `sitzung-start`.
                self.verlaeufe[neu[0]] = (Protokoll(), [])
                self.helfer.rufe("klicken", testid=s["eintrag"].replace("{id}", neu[0]))
                self.aktuell = neu[0]
                return 200, {"sitzung": neu[0]}, t0
            jetzt = self._fehler_der_seite()
            if jetzt is not None and jetzt != vor:
                return 422, {"code": jetzt[0], "text": jetzt[1]}, t0
            time.sleep(0.02)
        if vor is not None and self._fehler_der_seite() == vor:
            return 422, {"code": vor[0], "text": vor[1]}, t0  # dieselbe Ablehnung noch einmal: nicht von der alten zu unterscheiden
        return 0, {"fehler": "keine Reaktion der Seite"}, t0

    def _fehler_der_seite(self):
        """(Code, Text) des sichtbaren Fehlerfelds, sonst None."""
        s = self.sel
        if not self.helfer.rufe("sichtbar", testid=s["fehler"]):
            return None
        code = (self.helfer.rufe("text", testid=s["fehler_code"]) or "").strip()
        if not code:
            return None
        return code, (self.helfer.rufe("text", testid=s["fehler_text"]) or "").strip()

    def cursor(self, sid):
        """`data-cursor="<zeile>,<spalte>,<sichtbar>"` am Raster (Vertrag der Seite, SE7): Zeile und Spalte ab 0, sichtbar true
        oder false. Gibt (zeile, spalte, sichtbar) oder None, wenn das Attribut nicht dieser Form folgt."""
        import re
        self._waehle_sitzung(sid)
        w = self.helfer.rufe("attribut", testid=self.sel["raster"], name="data-cursor")
        m = re.match(r"^(\d+),(\d+),(true|false)$", w or "")
        return (int(m.group(1)), int(m.group(2)), m.group(3) == "true") if m else None

    def raster(self, sid):
        self._waehle_sitzung(sid)
        self.rahmen += 1
        return {"rahmen": self.rahmen, "zeilen": self._zeilen()}

    def verlauf(self, sid, ab=0):
        """Der Verlauf ist das Protokoll der gesehenen Rasterzeilen (`bruecke.Protokoll`), gebaut aus den Zeilen der Seite. Die Seite
        zeigt nur das Bild; einen Schub meldet sie nicht, der Laeufer leitet ihn aus zwei Bildern ab (`bruecke.schub_zwischen`). Was
        zwischen zwei Abfragen entsteht und wieder verschwindet, geht verloren (Grenze der Bedienung ueber die Seite)."""
        self._waehle_sitzung(sid)
        p, letzte = self.verlaeufe.get(sid) or (Protokoll(), [])
        jetzt = self._zeilen()
        if jetzt != letzte:
            b = Bild(self.abo["spalten"], len(jetzt))
            b.rows = [z.rstrip() for z in jetzt]
            p.aktualisiere(b, schub_zwischen(letzte, jetzt))
            self.verlaeufe[sid] = (p, jetzt)
        return len(p.text), p.text[ab:]

    def eingabe(self, sid, text):
        self._waehle_sitzung(sid)
        probe = "".join(text.split())[:8]
        for _ in range(2):  # die Seite kann einen Klick verschlucken: Echo im Raster abwarten, einmal wiederholen
            self.helfer.rufe("fuellen", testid=self.sel["eingabe"], wert=text)
            self.helfer.rufe("klicken", testid=self.sel["senden"])
            ende = time.monotonic() + 2.0
            while time.monotonic() < ende:
                if probe in "".join("".join(self._zeilen()).split()):
                    return 202, {"ok": True}
                time.sleep(0.05)
        return 202, {"ok": True}

    def schliessen(self):
        self.zumachen()
        return self.helfer.schliessen()
