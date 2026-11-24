"""Ziel des Laufs: `attrappe` startet die Attrappen von Bruecke, Kern-Dienst, Einschluss, Suche und Protokoll; `echt` nimmt
die Adressen der Testinstallation aus der Konfiguration. In beiden Faellen gehoeren dem Laeufer die Modell-Attrappen und
die fremde Adresse (Zaehler).
"""
from __future__ import annotations

import json
import os
import secrets
import signal
import subprocess
import sys
import time

import re

from . import fremd as fremd_mod
from . import konfig, messen, modell, weiterreicher

_DATEI = r"(index\.html|ansicht\.js|ansicht_bg\.wasm)"
SEITENORDNER_FEST = re.compile(r"^Seitenordner: Abbruch: (Seitenordner unvollstaendig: %s fehlt oder ist keine regulaere Datei"
                               r"|Seitenordner fehlt: kein Ordner|Seitenordner: %s ist groesser als [0-9]{1,12} Bytes)$" % (_DATEI, _DATEI))

HIER = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _bind(text):
    h, _, p = text.rpartition(":")
    return h, int(p)


class Ziel:
    def __init__(self, k, arbeit, fehler):
        self.k, self.arbeit, self.fehler = k, arbeit, list(fehler)
        self.mit = {}
        self.modell_srv = {}
        self.wr_srv = None
        self.gateway_schluessel = None
        self.cloud_marke = None
        self.wr_cloud_srv = None
        self.fremd = None
        self.prozess = None
        self.bruecke_k = None

    def starten(self):
        k = self.k
        # Gateway-Attrappe = das Modell der lokalen Route; es verlangt den Schluessel, ausser im Fehlerfall e4-direkt-offen
        self.cloud_marke = ("gho_" + secrets.token_hex(18)) if k["ziel"] == "attrappe" else None
        sch = k["gateway"].get("schluessel")
        self.gateway_schluessel = ("gw-" + secrets.token_hex(16)) if sch == "auto" else (sch if k["ziel"] == "attrappe" else None)
        for art in ("cloud", "lokal"):
            self.mit[art] = modell.Mitschnitt(os.path.join(self.arbeit, "mitschnitt-modell-%s.bin" % art))
            if k["ziel"] == "echt" and not k["modell"][art].get("zaehlen", False):
                continue  # echtes Modell, kein Modellweg zum Mitschneiden
            if art == "lokal":
                nutze = self.gateway_schluessel if "e4-direkt-offen" not in self.fehler else None
            else:  # Ziel der Cloud-Route: verlangt das echte Token, nur der Weiterreicher kennt es
                nutze = self.cloud_marke if "e4-cloud-direkt-offen" not in self.fehler else None
            self.modell_srv[art] = modell.starte(self.mit[art], k["modell"]["mcp_werkzeug"], _bind(k["modell"][art]["bind"]), nutze)
        if self.gateway_schluessel:
            os.makedirs(k["zaehlordner"], exist_ok=True)
            os.makedirs(k["protokoll"]["ordner"], exist_ok=True)
            liste = list(k["marker"].values())
            if "weiterreicher-falsche-markerliste" in self.fehler:  # Mutationsprobe: die Weiterreicher zaehlen eine falsche Liste
                liste = [m for m in liste if m != k["marker"].get("np")] + ["MX-9Z9"]
            self.wr_srv = weiterreicher.starte(k["zaehlordner"], liste,
                                               ("127.0.0.1", self.modell_srv["lokal"].server_address[1]), self.gateway_schluessel,
                                               fehler=self.fehler, protokollordner=k["protokoll"]["ordner"],
                                               stderr_datei=k.get("weiterreicher", {}).get("stderr_datei"))
            self.wr_cloud_srv = weiterreicher.starte(k["zaehlordner"], liste,
                                                     ("127.0.0.1", self.modell_srv["cloud"].server_address[1]), self.cloud_marke,
                                                     fehler=self.fehler, protokollordner=k["protokoll"]["ordner"],
                                                     betriebsart="kopf-einsetzen", platzhalter=k["copilot"]["platzhalter"])
            k["copilot"]["weiterreicher_url"] = "http://127.0.0.1:%d" % self.wr_cloud_srv.server_address[1]
            k["copilot"]["ziel_url"] = "http://127.0.0.1:%d" % self.modell_srv["cloud"].server_address[1]
            if "ausgang-blockiert" in self.fehler:  # Mutationsprobe: der Ausgang des Wirts ist gesperrt
                k["ca"]["ausgang_ziel"] = "http://127.0.0.1:1/"
            else:
                k["ca"]["ausgang_ziel"] = k["copilot"]["ziel_url"] + "/"
            k["gateway"]["url"] = "http://127.0.0.1:%d" % self.modell_srv["lokal"].server_address[1]
            if k["ziel"] == "attrappe" and k["gateway"].get("suche"):
                self._schluesseldateien()
            k["gateway"]["weiterreicher_url"] = "http://127.0.0.1:%d" % self.wr_srv.server_address[1]
        os.makedirs(k["protokoll"]["ordner"], exist_ok=True)
        self.fremd = fremd_mod.Fremd(_bind(k["fremd"]["bind"]))
        if "s2-fremd-aus" in self.fehler:
            self.fremd.schliessen()  # Mutationsprobe: die Gegenprobe von S2 findet die Adresse nicht
        b = dict(k["bruecke"])
        if "s3-darf-d11-fehlt" in self.fehler:  # Mutationsprobe: das Tor-Programm laesst D11 aus
            k["datentor_befehl"] = list(k["datentor_befehl"]) + ["--fehler", "d11-fehlt"]
        if "s4-kette-blind" in self.fehler:  # Mutationsprobe: der Pruefbefehl ignoriert --letzte
            k["kette_befehl"] = list(k["kette_befehl"]) + ["--fehler", "kette-blind"]
        if "e6-loeschzeile-mit-pfad" in self.fehler:  # Mutationsprobe: die Attrappe des Loesen-Befehls schreibt den Pfad in die Zeile
            k["ordnermarke"]["loeschen_befehl"] = list(k["ordnermarke"]["loeschen_befehl"]) + ["--fehler", "e6-loeschzeile-mit-pfad"]
        if k["ziel"] == "attrappe":
            self._attrappe_starten(b)
        else:
            if self.fehler:
                raise konfig.KonfigFehler("--fehler gibt es nur gegen die Attrappen")
            self._echt(b)
        for pfad in (k["protokoll"]["ordner"],):
            os.makedirs(pfad, exist_ok=True)
        self.bruecke_k = b
        return b

    def _schluesseldateien(self):
        """Nur Attrappen: Schluesseldateien und Zielliste fuer die Pruefinstanz und die Suche (Vertrag 8, Abschnitte 5, 9a, 12). Ordner 0700,
        Dateien 0600, Wert ohne LF, ausserhalb jedes Arbeitsordners. Im echten Lauf legt das Paket sie an; der Laeufer schreibt sie nie."""
        g = self.k["gateway"]
        ordner = os.path.dirname(g["suche"]["schluessel_datei"])
        os.makedirs(ordner, mode=0o700, exist_ok=True)
        os.chmod(ordner, 0o700)

        def schreibe(pfad, text):
            fd = os.open(pfad, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(text)
            os.chmod(pfad, 0o600)
        schreibe(g["suche"]["schluessel_datei"], self.gateway_schluessel)
        zl = g["suche"].get("zielliste")
        if zl and self.cloud_marke:
            cloud = os.path.join(ordner, "cloud.schluessel")
            schreibe(cloud, self.cloud_marke)
            ziele = [{"name": "cloud-api", "host": "api.beispiel.test", "port": 443, "schluessel_datei": cloud, "kopf": "authorization",
                      "wert_vorsatz": "Bearer ", "regeln": [{"methode": "*", "pfad": "/*"}], "upgrade": "durchreichen"},
                     {"name": "cloud-telemetrie", "host": "telemetrie.beispiel.test", "port": 443, "schluessel_datei": None, "kopf": None,
                      "wert_vorsatz": None, "regeln": [{"methode": "POST", "pfad": "/telemetry"}], "upgrade": "ablehnen"}]
            schreibe(zl, json.dumps({"fassung": 1, "ca_zulassung": "zugelassen", "ziele": ziele}, indent=1) + "\n")

    def _echt(self, b):
        """Adresse und Token der Bruecke aus den Dateien der Bruecke (`port_datei`, `token_datei`) oder aus der Konfiguration."""
        if b.get("port_datei"):
            try:
                with open(b["port_datei"], encoding="utf-8") as f:
                    port = f.read().strip().rsplit(":", 1)[-1]
                b["url"] = "http://127.0.0.1:%d" % int(port)
            except (OSError, ValueError) as e:
                raise RuntimeError("bruecke.dateien-fehlen")
        if b.get("token_datei"):
            try:
                with open(b["token_datei"], encoding="utf-8") as f:
                    b["token"] = f.read().strip()
            except OSError:
                raise RuntimeError("bruecke.dateien-fehlen")
        if not b.get("url") or b["url"] == "auto":
            raise RuntimeError("bruecke.dateien-fehlen")
        g = self.k["gateway"]
        if not g.get("url"):
            # Adresse des Gateways aus der Einstellungsdatei des Nutzers: Route der lokalen Art, Feld `ziel`
            # (OpenAI-Basisadresse mit /v1). Die Schluesseldatei wird nie gelesen.
            try:
                with open(self.k["einstellungen"]["pfad"], encoding="utf-8") as f:
                    einst = json.load(f)
                ziel = next(r.get("ziel") for r in einst["routen"] if r["schluessel"] == self.k["routen"]["lokal"])
                g["url"] = ziel[:-3] if isinstance(ziel, str) and ziel.endswith("/v1") else None
            except (OSError, ValueError, StopIteration, KeyError, TypeError):
                g["url"] = None  # E4 meldet dann `offen`

    def _attrappe_starten(self, b):
        k = self.k
        sock = os.path.join(self.arbeit, "sock")
        os.makedirs(k["ordnermarke"]["zustandsordner"], mode=0o700, exist_ok=True)
        for d in (sock, k["zaehlordner"], k["protokoll"]["ordner"], os.path.dirname(k["einstellungen"]["pfad"])):
            os.makedirs(d, exist_ok=True)
        os.chmod(sock, 0o700)
        if not os.path.exists(k["einstellungen"]["pfad"]):
            with open(k["einstellungen"]["pfad"], "w", encoding="utf-8") as f:
                json.dump(konfig.einstellungen_datei(k, "basis"), f)
        token = secrets.token_hex(32)  # wie die Bruecke: 64 Hex-Zeichen (token.rs); die neue Seite nimmt nur diese Form
        konf = {"token": token, "vault": k["vault"], "einstellungen": k["einstellungen"]["pfad"],
                "protokoll": k["protokoll"]["ordner"], "socket_wurzel": sock, "zaehlordner": k["zaehlordner"], "marker": k["marker"],
                "modell_url": "http://127.0.0.1:%d" % self.wr_cloud_srv.server_address[1],
                "cloud_platzhalter": k["copilot"]["platzhalter"], "cloud_token": self.cloud_marke, "gastpunkt": k["gastpunkt"],
                "netzziele": k["netzziele"], "cloud_ziel_url": k["copilot"]["ziel_url"],
                "zustandsordner": k["ordnermarke"]["zustandsordner"], "fremd_url": self.fremd.url(),
                "modell_url_lokal": "http://127.0.0.1:%d" % self.wr_srv.server_address[1],
                "gateway_url": k["gateway"]["url"], "gateway_schluessel": self.gateway_schluessel,
                "mcp_werkzeug": k["modell"]["mcp_werkzeug"], "harness": k["harness"], "fehler": self.fehler,
                "start_dauer_s": k["bruecke"].get("attrappe_start_dauer_s", 0.2),
                "groesse": [k["bruecke"]["abo"]["spalten"], k["bruecke"]["abo"]["zeilen"]], "deflate": bool(k["bruecke"].get("deflate", True)),
                "arbeitsordner": k["start"]["cloud"]["arbeitsordner"],
                "fehler_verzoegerung_s": float(k["grenzen"]["start_s"]) + 0.5,
                "marker_start_datei": k.get("marker_start_datei"), "seite_ordner": k.get("seite_ordner") or "",
                "kein_selbstwechsel": (k.get("attrappe") or {}).get("kein_selbstwechsel") is True}
        pfad = os.path.join(self.arbeit, "bruecke-attrappe.json")
        with open(pfad, "w", encoding="utf-8") as f:
            json.dump(konf, f)
        os.chmod(pfad, 0o600)
        portdatei = os.path.join(self.arbeit, "bruecke.port")
        self.prozess = subprocess.Popen(
            [sys.executable, "-I", "-B", os.path.join(HIER, "attrappen", "bruecke_attrappe.py"), "--konfig", pfad,
             "--portdatei", portdatei, "--rolle", k["such_prozess"]["muster_aussen"]],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=open(os.path.join(self.arbeit, "bruecke.err"), "wb"),
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8"})
        ende = time.monotonic() + 8
        while time.monotonic() < ende:
            if os.path.exists(portdatei) and open(portdatei).read().strip():
                break
            if self.prozess.poll() is not None:
                try:
                    err = open(os.path.join(self.arbeit, "bruecke.err"), encoding="utf-8", errors="replace").read().strip()
                except OSError:
                    err = ""
                if err.startswith("Seitenordner:"):  # fester Text der Attrappe, kein Rueckfall auf seite.html
                    erste = err.splitlines()[0]
                    if SEITENORDNER_FEST.match(erste):  # die festen Saetze der Attrappe (Dateiname aus der festen Menge, Zahl der Grenze)
                        raise RuntimeError(erste)
                    messen.diagnose_notiere("ziel.seitenordner", erste)
                    raise RuntimeError("ziel.seitenordner")
                messen.diagnose_notiere("ziel.attrappe", "Bruecken-Attrappe endete mit %s" % self.prozess.returncode)
                raise RuntimeError("ziel.attrappe-endete")
            time.sleep(0.05)
        else:
            raise RuntimeError("ziel.attrappe-ohne-port")
        b["url"] = "http://127.0.0.1:%s" % open(portdatei).read().strip()
        b["token"] = token
        self.seite_beleg = self._seite_beleg(b["url"], token)

    @staticmethod
    def _seite_beleg(url, token):
        """Welche Seite die Attrappe haelt (Name und sha256 je Datei), von ihr selbst gemeldet (`/diagnose`); keine Inhalte."""
        import urllib.request
        r = urllib.request.Request(url + "/diagnose", headers={"Authorization": "Bearer " + token})
        with urllib.request.urlopen(r, timeout=5) as a:
            return json.loads(a.read().decode("utf-8")).get("seite")

    def beenden(self):
        """Beendet nur eigene Prozesse und prueft das Ende. Gibt eine Liste von Meldungen zurueck (leer = sauber)."""
        meld = []
        if self.prozess is not None:
            if self.prozess.poll() is None:
                self.prozess.send_signal(signal.SIGTERM)
                try:
                    self.prozess.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.prozess.kill()
                    self.prozess.wait(timeout=5)
                    meld.append("Bruecken-Attrappe musste mit SIGKILL beendet werden")
            if self.prozess.poll() is None:
                meld.append("Bruecken-Attrappe laeuft noch")
        for srv in list(self.modell_srv.values()) + [x for x in (self.wr_srv, self.wr_cloud_srv) if x]:
            srv.shutdown()
            srv.server_close()
        if self.fremd and self.fremd.offen:
            self.fremd.schliessen()
        return meld
