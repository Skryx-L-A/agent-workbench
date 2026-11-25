"""Konfiguration des Laeufers: laden, `$ARBEIT`, `$HIER`, `$NUTZER`, `$DATEN`, `$XDG_RUNTIME_DIR`, `$HEIM_PASSWD` und `$PYTHON` einsetzen, pruefen, Einstellungsdatei des Drehbuchs bauen."""
from __future__ import annotations

import copy
import getpass
import json
import os
import pwd
import sys

PFLICHT = ("ziel", "bruecke", "modell", "fremd", "einstellungen", "protokoll", "kette_befehl", "datentor_befehl", "vault",
           "routen", "routen_klassen", "klassen", "harness", "kunde", "kunde2", "gateway", "erwartet", "start", "marker", "notiz_nk", "frage", "befehle", "prompts", "grenzen", "such_prozess")


# Zug 2 des knappen Cloud-Laufs (das Buendel): Eingaben mit diesem Zeichen am Anfang fuehrt die CLI selbst in ihrer Shell aus, ohne Anfrage
# an das Modell. So bekommt das Modell keinen Auftrag, nach Zugangsdaten zu suchen; das Buendel laeuft trotzdem im Einschluss der Sitzung.
SHELL_EINGABE = "!"


class KonfigFehler(Exception):
    pass


def _setze(wert, ersatz):
    if isinstance(wert, str):
        for k, v in ersatz.items():
            wert = wert.replace(k, v)
        return wert
    if isinstance(wert, list):
        return [_setze(x, ersatz) for x in wert]
    if isinstance(wert, dict):
        return {k: _setze(v, ersatz) for k, v in wert.items()}
    return wert


def lade(pfad, hier, arbeit, nutzer=None):
    try:
        with open(pfad, encoding="utf-8") as f:
            k = json.load(f)
    except (OSError, ValueError) as e:
        raise KonfigFehler("Konfiguration nicht lesbar (%s)" % type(e).__name__)  # nie der Text der Ausnahme
    nutzer = nutzer or getpass.getuser()
    lauf = os.environ.get("XDG_RUNTIME_DIR") or "/run/user/%d" % os.getuid()  # Laufzeitordner des Nutzers (Bruecke: <lauf>/werkbank/)
    k = _setze(k, {"$ARBEIT": arbeit, "$HIER": hier, "$NUTZER": nutzer, "$DATEN": "/var/lib/werkbank/" + nutzer,
                   "$XDG_RUNTIME_DIR": lauf, "$UID": str(os.getuid()), "$PYTHON": sys.executable,
                   # Heim wie der Halter es bestimmt: Feld 6 des eigenen Eintrags in /etc/passwd; HOME gilt nicht (Halter-Vertrag, Ablage)
                   "$HEIM_PASSWD": pwd.getpwuid(os.getuid()).pw_dir})
    fehlt = [x for x in PFLICHT if x not in k]
    if fehlt:
        raise KonfigFehler("Schluessel fehlen: " + ", ".join(fehlt))
    if k["ziel"] not in ("attrappe", "echt"):
        raise KonfigFehler("ziel muss attrappe oder echt sein")
    bv = k["prompts"].get("buendel")
    if bv is not None and not (isinstance(bv, str) and bv.startswith(SHELL_EINGABE)):
        # Zug 2 des knappen Cloud-Laufs ist eine Shell-Eingabe der CLI: Das Buendel geht nie als Auftrag an das Modell.
        raise KonfigFehler("prompts.buendel muss eine Shell-Eingabe der CLI sein (beginnt mit %s)" % SHELL_EINGABE)
    pl = k.get("copilot", {}).get("platzhalter")
    if isinstance(pl, dict):  # der Platzhalter steht nicht als Text in der Datei: Vorsatz, Unterstrich und n Nullen
        k["copilot"]["platzhalter"] = pl["vorsatz"] + "_" + "0" * int(pl["nullen"])
    k["vault"] = os.path.realpath(k["vault"])
    k["nutzer"] = nutzer
    return k


def unter(wurzel, pfad):
    """Wahr, wenn `pfad` (aufgeloest) unter `wurzel` liegt. Schutz vor Schreibzugriffen ausserhalb der Testinstallation."""
    w, p = os.path.realpath(wurzel), os.path.realpath(pfad)
    return p == w or p.startswith(w + os.sep)


def _grundstellung(k):
    cloud, lokal = k["routen"]["cloud"], k["routen"]["lokal"]
    lokale_route = {"schluessel": lokal, "art": "lokal", "klasse": k["routen_klassen"]["lokal"]}
    cloud_route = {"schluessel": cloud, "art": "cloud", "klasse": k["routen_klassen"]["cloud"]}
    if k["ziel"] == "attrappe" and k["gateway"].get("url"):  # Ziel und Modell der Route (Vertrag 2): die Pruefinstanz liest sie hier
        lokale_route.update({"ziel": k["gateway"]["url"] + "/v1", "modell": k["start"]["lokal"]["modell"]})
    if k["ziel"] == "attrappe":  # Startwahl der Seite nennt das Modell jeder Route; ohne Eintrag bietet die Seite nur "auto"
        cloud_route["modell"] = k["start"]["cloud"]["modell"]
    return {
        "fassung": 1,
        "klassen": list(k["klassen"]),
        "vorgabestufe": k["klassen"][-1],
        "routen": [cloud_route, lokale_route],
        "kunden": [{"schluessel": k["kunde"], "erlaubt": [lokal], "verboten": []},
                   {"schluessel": k["kunde2"], "erlaubt": [], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
    }


def einstellungen_datei(k, variante="basis", basis=None):
    """Einstellungsdatei nach Vertrag 2 fuer den Aufbau aus Drehbuch Abschnitt 2, oder eine Variante davon.

    `basis`: Cloud-Route (Art cloud, Klasse aus `routen_klassen`), lokale Route (Art lokal), Kunde 1 mit `erlaubt` lokale
    Route, Kunde 2 ohne Freigabe. Ist `basis` angegeben (die vorhandene Datei der Testinstallation), geht die Variante von ihr
    aus; so bleiben ihre uebrigen Felder erhalten.
    `cloud_kundendaten`: Gegenprobe von S3. Die Cloud-Route hat die hoechste Klasse, und Kunde 1 darf die Route. Ohne die zweite
    Aenderung stuende `route-fuer-kunde-nicht-erlaubt` (D10) vor `kundendaten-an-cloud` (D11); siehe README.
    `ks_lokal`: Gegenprobe von S5. Kunde 2 darf die lokale Route.
    """
    cloud, lokal = k["routen"]["cloud"], k["routen"]["lokal"]
    d = copy.deepcopy(basis) if basis is not None else _grundstellung(k)
    route = {r["schluessel"]: r for r in d["routen"]}
    kunde = {x["schluessel"]: x for x in d["kunden"]}
    if variante == "cloud_kundendaten":
        route[cloud]["klasse"] = k["klassen"][-1]
        kunde[k["kunde"]]["erlaubt"] = sorted(set(kunde[k["kunde"]].get("erlaubt", [])) | {lokal, cloud})
    elif variante == "ks_lokal":
        kunde[k["kunde2"]]["erlaubt"] = sorted(set(kunde[k["kunde2"]].get("erlaubt", [])) | {lokal})
    elif variante != "basis":
        raise KonfigFehler("unbekannte Variante " + variante)
    return d


def pruefe_aufbau(k, d):
    """Gibt eine Liste von Abweichungen der Einstellungsdatei vom Aufbau des Drehbuchs (leer: stimmt)."""
    soll = _grundstellung(k)
    ab = []
    if d.get("klassen") != soll["klassen"]:
        ab.append("klassen")
    route = {r.get("schluessel"): r for r in d.get("routen", [])}
    for r in soll["routen"]:
        if route.get(r["schluessel"], {}).get("klasse") != r["klasse"] or route.get(r["schluessel"], {}).get("art") != r["art"]:
            ab.append("route " + r["schluessel"])
    kunde = {x.get("schluessel"): x for x in d.get("kunden", [])}
    for x in soll["kunden"]:
        if sorted(kunde.get(x["schluessel"], {}).get("erlaubt", ["?"])) != sorted(x["erlaubt"]):
            ab.append("kunde " + x["schluessel"])
    if d.get("kundendaten_an_cloud", {}).get("freigaben") != []:
        ab.append("freigaben")
    return ab


def vorgabe_fuer_tor(einst):
    """Einstellungsdatei als Vorgabe des Tors (Vertrag 3, Abschnitt 2): ohne das Feld `fassung`."""
    return {k: v for k, v in einst.items() if k != "fassung"}
