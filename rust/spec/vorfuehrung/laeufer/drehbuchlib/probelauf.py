"""Betriebsart `probelauf`: der Laeufer auf dem Zielrechner gegen die echten Teile, mit einer Ergebnisdatei ohne Inhalte.

Befehle (Aufruf ueber `drehbuch.py`):
  probelauf --ergebnis <datei> [--konfig <datei>] [--arbeit <ordner>] [--nutzer <name>] [--fehler <name,...>]
      Exit 0 alle Schritte bestanden; 1 mindestens ein Schritt nicht bestanden (Datei vollstaendig; mit --nur-cloud nur bei einem roten Schritt); 2 Aufruffehler, keine
      Datei; 3 eine Voraussetzung fehlt (die Datei nennt den Code, kein Schritt laeuft).
  pruefe-ergebnis <datei> [--konfig <datei>] [--verboten <wert>]...
      Exit 0, wenn Form und Inhaltsfreiheit stimmen; sonst 1; 2 Aufruffehler.
  selbsttest-ergebnis
      Der Fall, der die Inhaltsfreiheit belegt (Nachtrag 5): Lauf gegen die Attrappen mit bekannten Werten, Zaehlung in der
      Datei (erwartet 0), Gegenprobe mit eingebautem Leck (muss rot werden).

Die Ergebnisdatei hat eine geschlossene Form. Sie traegt nur feste Codes, Zahlen, Zeitstempel und Versionen
(siehe `FORM` und `pruefe_form`). Notiztexte, Marker, Titel, Pfade, Antworten, Terminalinhalt, Token, Schluessel, Adressen
und Namen kommen nicht hinein: Jeder Textwert muss einem engen Muster folgen, und `pruefe_ergebnis` zaehlt zusaetzlich
bekannte Werte aus der Konfiguration.
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
import platform
import re
import signal
import subprocess
import sys
import time

from . import ablauf, bericht, konfig, messen

LAEUFER_VERSION = "1"
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
VERSION = re.compile(r"^[0-9A-Za-z.+~_-]{1,64}$")
ZEIT = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
FASSUNG = 2  # 2 seit 21.11.: je Schritt `offen` (Liste fester Codes)
OFFEN_MAX = 40
SCHRITTE = ["S1", "S2", "S3", "S4", "S5", "E1", "E2", "E3", "E4-lokal", "E4-cloud", "E5", "E6"]
VORAUSSETZUNGEN = ["bruecke.dateien-fehlen", "bruecke.nicht-erreichbar", "copilot.nicht-angemeldet", "gateway.nicht-erreichbar",
                   "gateway.modell-gesperrt", "modellweg.schluessel-fehlt", "einstellungen.abweichend", "vault.unvollstaendig", "konfiguration.fehlt",
                   "bruecke.dienst-startet-nicht", "bruecke.sitzungen_laufen", "bruecke.sitzungen_unbekannt",
                   "probelauf.marker_datei_fehlt", "probelauf.arbeitsordner_geschuetzt"]
# feste Liste der Fehlercodes: `<schritt>.teil<n>` (n-te Teilpruefung des Schritts in der Reihenfolge von schritte.py),
# `<schritt>.ausnahme`, `<schritt>.offen`, `<schritt>.uebersprungen`, dazu Sonderfaelle
FEHLERCODES = ({"%s.teil%d" % (s.lower(), n) for s in SCHRITTE for n in range(1, 41)}
               | {"%s.%s" % (s.lower(), w) for s in SCHRITTE for w in ("ausnahme", "offen", "uebersprungen", "nicht-gelaufen", "nicht-angemeldet",
                                                                          "kein_eingabefeld", "harness_beendet", "zug_frist", "nicht_abgesendet")}
               | {"s1.kontrolle_blind", "e4-cloud.kontrolle_blind", "e4-lokal.kontrolle_blind"}
               | {"copilot.start-limit", "cloud.eingaben-limit", "zaehldatei.inhalt", "ca.cli_abgelehnt", "ca.ausgang_blockiert"}
               | {"%s.%s" % (s.lower(), w) for s in ("E4-lokal", "E4-cloud") for w in ("schluessel_suche_unlesbar", "schluessel_suche_fehler")}
               | {"%s.%s" % (s.lower(), w) for s in SCHRITTE for w in ("zaehlung_nicht_vergleichbar", "zaehlung_zu_wenig", "zaehlung_cloud_unbelegt",
                                                                        "markerliste_abweichend", "nur_attrappe", "nur_cloud")})
SYSTEM_BWRAP = ("ubuntu", "gesperrt", "unbekannt")  # Lage des System-bwrap (Nachtrag 20.11., 12): ein Hinweis, kein Fehler
UBUNTU_PROFIL = ("geladen", "entladen", "unbekannt")  # Lage des Ubuntu-Profils bwrap-userns-restrict (Entscheidung vom 20.11.2026): ein Hinweis
OHNE_INHALT_FELDER = ("fassung", "art", "beginn", "ende", "versionen", "copilot_starts", "cloud_eingaben", "umfang", "kennzeichnung", "s1_gegenprobe", "browser", "ca",
                      "system_bwrap", "ubuntu_profil", "voraussetzung", "schritte", "bestanden")
UMFANG = ("voll", "nur_cloud")
EXIT_TEILUMFANG = 4  # --nur-cloud ohne roten Schritt: ein Teilumfang, nie 0 (kein Probelauf); mit rotem Schritt Exit 1


def _utc(t=None):
    return datetime.datetime.utcfromtimestamp(t if t is not None else time.time()).strftime("%Y-%m-%dT%H:%M:%SZ")


def _doppelt(paare):
    d = {}
    for k, v in paare:
        if k in d:
            raise ValueError("doppelter Schluessel")  # nie der Name: er stammt aus der gepruefen Datei
        d[k] = v
    return d


# --- Form ------------------------------------------------------------------------------------------------------
def pruefe_form(d):
    """Gibt eine Liste von Abweichungen von der geschlossenen Form (leer: stimmt)."""
    fehler = []

    def geschlossen(o, felder, name):
        if not isinstance(o, dict):
            fehler.append(name + ": kein Objekt")
            return False
        if set(o) != set(felder):
            # fehlende Felder nennt die Form beim Namen (feste Namen); unbekannte nur als Zahl, ihre Namen stammen aus der Datei
            fehler.append("%s: Felder fehlen %s, %d unbekannt" % (name, sorted(set(felder) - set(o)), len(set(o) - set(felder))))
            return False
        return True
    if not geschlossen(d, OHNE_INHALT_FELDER, "datei"):
        return fehler
    if d["fassung"] != FASSUNG or isinstance(d["fassung"], bool):
        fehler.append("fassung")
    if d["art"] != "probelauf-ergebnis":
        fehler.append("art")
    for f in ("beginn", "ende"):
        if not isinstance(d[f], str) or not ZEIT.match(d[f]):
            fehler.append(f)
    if geschlossen(d["versionen"], ("paket", "laeufer", "copilot_cli", "ubuntu", "kernel"), "versionen"):
        for k_, v in d["versionen"].items():
            if not isinstance(v, str) or not VERSION.match(v) or re.search(r"(^|[^0-9])[0-9]{1,3}(\.[0-9]{1,3}){3}($|[^0-9])", v):
                fehler.append("versionen." + k_)
    if not isinstance(d["copilot_starts"], int) or isinstance(d["copilot_starts"], bool) or not 0 <= d["copilot_starts"] <= 2:
        fehler.append("copilot_starts")
    if not isinstance(d["cloud_eingaben"], int) or isinstance(d["cloud_eingaben"], bool) or not 0 <= d["cloud_eingaben"] <= 10000:
        fehler.append("cloud_eingaben")
    g = d["s1_gegenprobe"]
    if geschlossen(g, ("weg", "neue_sitzungen", "neue_kinder"), "s1_gegenprobe"):
        if g["weg"] not in (None, "diagnose", "sitzungen_und_kinder"):
            fehler.append("s1_gegenprobe.weg")
        # `neue_sitzungen` ist nie negativ: Die Sitzungsliste fuehrt auch beendete Sitzungen, eine Menge schrumpft dort nicht. `neue_kinder`
        # kann negativ sein (ein Kind des Kerns endet im Fenster, etwa eine Sitzung); die Pruefung in S1 ist dann rot, und das Rot darf nicht
        # hinter einem Formfehler der Datei verschwinden.
        for n, untergrenze in (("neue_sitzungen", 0), ("neue_kinder", -100000)):
            if g[n] is not None and (not isinstance(g[n], int) or isinstance(g[n], bool) or not untergrenze <= g[n] <= 100000):
                fehler.append("s1_gegenprobe." + n)
    if d["umfang"] not in UMFANG:
        fehler.append("umfang")
    elif d["kennzeichnung"] != ("Teilumfang" if d["umfang"] == "nur_cloud" else None):
        fehler.append("kennzeichnung")
    if d["browser"] not in ("playwright", "pruefclient"):
        fehler.append("browser")
    if geschlossen(d["ca"], ("cli_nimmt_an", "ausgang"), "ca"):
        for k_, v in d["ca"].items():
            if v is not None and not isinstance(v, bool):
                fehler.append("ca." + k_)
    if not isinstance(d["system_bwrap"], str) or d["system_bwrap"] not in SYSTEM_BWRAP:
        fehler.append("system_bwrap")
    if not isinstance(d["ubuntu_profil"], str) or d["ubuntu_profil"] not in UBUNTU_PROFIL:
        fehler.append("ubuntu_profil")
    if d["voraussetzung"] is not None and (not isinstance(d["voraussetzung"], str) or d["voraussetzung"] not in VORAUSSETZUNGEN):
        fehler.append("voraussetzung")
    if not isinstance(d["bestanden"], bool):
        fehler.append("bestanden")
    s = d["schritte"]
    if not isinstance(s, list) or [x.get("schritt") if isinstance(x, dict) else None for x in s] != SCHRITTE:
        fehler.append("schritte: Reihenfolge oder Namen (ein Schritt E4 ohne -lokal oder -cloud ist ein Formfehler)")
    else:
        for x in s:
            if not geschlossen(x, ("schritt", "bestanden", "fehlercode", "dauer_ms", "offen"), "schritt " + x["schritt"]):
                continue
            o = x["offen"]
            if not isinstance(o, list) or len(o) > OFFEN_MAX or any(not isinstance(c, str) or not KENNUNG.match(c) for c in o) \
                    or (x["bestanden"] is True and o):
                fehler.append(x["schritt"] + ".offen")
            if not isinstance(x["bestanden"], bool):
                fehler.append(x["schritt"] + ".bestanden")
            c = x["fehlercode"]
            if c is not None and (not isinstance(c, str) or not KENNUNG.match(c) or (c not in FEHLERCODES and c not in VORAUSSETZUNGEN)):
                fehler.append(x["schritt"] + ".fehlercode")
            if not isinstance(x["dauer_ms"], int) or isinstance(x["dauer_ms"], bool) or x["dauer_ms"] < 0:
                fehler.append(x["schritt"] + ".dauer_ms")
        if isinstance(d["bestanden"], bool) and d["bestanden"] != (all(x.get("bestanden") is True for x in s) and d["voraussetzung"] is None
                                                       and d["umfang"] == "voll"):
            fehler.append("bestanden passt nicht zu den Schritten")
    return fehler


def verbotene_werte(k, zusatz=()):
    """Werte, die nie in der Datei stehen duerfen: Marker, Titel, Pfade des Vaults, Frage, Nutzer, Adressen, Arbeitsordner."""
    w = set(zusatz)
    for m in k.get("marker", {}).values():
        w.add(m)
    for n in ("notiz_nk", "notiz_ns"):
        if n in k:
            w.update([k[n]["rel"], k[n]["titel"]])
    for x in (k.get("frage"), k.get("nutzer"), k.get("vault"), "127.0.0.1", "localhost", "http://", "https://"):
        if x:
            w.add(x)
    g = k.get("gateway", {})
    for x in (g.get("url"), g.get("weiterreicher_url")):
        if x:
            w.add(x)
    return sorted(w)


def pruefe_ergebnis(argv, hier):
    ap = argparse.ArgumentParser(prog="drehbuch.py pruefe-ergebnis")
    ap.add_argument("datei")
    ap.add_argument("--konfig")
    ap.add_argument("--verboten", action="append", default=[])
    a = ap.parse_args(argv)
    k = {}
    if a.konfig:
        try:
            k = konfig.lade(a.konfig, hier, "/nicht-vorhanden")
        except konfig.KonfigFehler as e:
            print("Konfigurationsfehler: %s" % e, file=sys.stderr)
            return 2
    try:
        with open(a.datei, "rb") as f:
            roh = f.read()
        d = json.loads(roh.decode("utf-8"), object_pairs_hook=_doppelt)
    except (OSError, ValueError) as e:
        print("nicht lesbar oder kein geschlossenes JSON (%s)" % type(e).__name__)  # nie der Text: er kann Schluessel der Datei tragen
        return 1
    problem = pruefe_form(d)
    treffer = {w: roh.count(w.encode("utf-8")) for w in verbotene_werte(k, a.verboten)}
    problem += ["enthaelt verbotenen Wert (%d Stellen)" % n for w, n in treffer.items() if n]
    # Muster: Schluessel des Gateways, falls die Konfiguration es nennt
    muster = k.get("gateway", {}).get("schluessel_muster")
    if muster and re.search(muster, roh.decode("utf-8", "replace")):
        problem.append("enthaelt etwas, das dem Schluesselmuster folgt")
    if problem:
        for p in problem:
            print("FALSCH: %s" % p)
        return 1
    print("OK: Form geschlossen, %d verbotene Werte gezaehlt, jeder 0" % len(treffer))
    return 0


# --- Probelauf ---------------------------------------------------------------------------------------------------
def _offen(wert):
    """Wahr, wenn die Konfiguration noch einen Platzhalter `OFFEN...` traegt (Wert, der erst mit dem Paket feststeht)."""
    if isinstance(wert, str):
        return wert.startswith("OFFEN")
    if isinstance(wert, list):
        return any(_offen(x) for x in wert)
    if isinstance(wert, dict):
        return any(_offen(v) for v in wert.values())
    return False


PAKET_GROESSE_MAX = 65536  # `package.json` des Hauptpakets: mehr liest der Laeufer nicht


def harness_fassung(lauf_konfig, harness, paket_name):
    """Die Fassung eines Harness aus seiner Paketform, ohne ein Programm des Harness zu starten (Regel: die Programme eines Harness
    laufen nie ausserhalb eines Einschlusses). Wie das Produkt (`paketform-copilot-1.0.88`, Punkt 1): `harness.<name>` aus `lauf_konfig`
    (`/etc/werkbank/lauf.json`), davon der echte Pfad, daneben `package.json` mit `name` == `paket_name`, `bin.<name>` == Dateiname des
    Programms und `version`. Fehlt etwas, ist es nicht lesbar oder passt es nicht, ist der Wert `unbekannt` (kein Abbruch). Nach `realpath`
    wird kein Verweis mehr verfolgt: `package.json` als Verweis zaehlt als fehlend."""
    try:
        if not lauf_konfig:
            return "unbekannt"
        with open(lauf_konfig, encoding="utf-8") as f:
            pfad = json.load(f)["harness"][harness]
        if not isinstance(pfad, str) or not os.path.isabs(pfad):
            return "unbekannt"
        echt = os.path.realpath(pfad)
        paket = os.path.join(os.path.dirname(echt), "package.json")
        if os.path.islink(paket):
            return "unbekannt"
        fd = os.open(paket, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
        with os.fdopen(fd, "rb") as f:
            roh = f.read(PAKET_GROESSE_MAX + 1)
        if len(roh) > PAKET_GROESSE_MAX:
            return "unbekannt"
        d = json.loads(roh.decode("utf-8"))
        if d.get("name") != paket_name or d["bin"][harness] != os.path.basename(echt) or not isinstance(d.get("version"), str):
            return "unbekannt"
        return d["version"]
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return "unbekannt"


def versionen(k, hier):
    def sauber(t):
        t = (t or "").strip()
        return t if VERSION.match(t) and not re.search(r"(^|[^0-9])[0-9]{1,3}(\.[0-9]{1,3}){3}($|[^0-9])", t) else "unbekannt"

    def befehl(b, muster=r"[0-9]+(?:[.][0-9A-Za-z+~_-]+)*"):
        if not b:
            return "unbekannt"
        try:
            p = subprocess.run(list(b), capture_output=True, timeout=15)
            m = re.search(muster, p.stdout.decode("utf-8", "replace"))
            return sauber(m.group(0) if m else "")
        except (OSError, subprocess.TimeoutExpired):
            return "unbekannt"
    ubuntu = "unbekannt"
    try:
        with open("/etc/os-release", encoding="utf-8") as f:
            m = re.search(r'^VERSION_ID="?([0-9.]+)"?', f.read(), re.M)
            ubuntu = sauber(m.group(1) if m else "")
    except OSError:
        pass
    pr = k.get("probelauf", {})
    return {"paket": befehl(pr.get("paket_version_befehl")) if pr.get("paket_version_befehl") else sauber(pr.get("paket_version", "")),
            "laeufer": LAEUFER_VERSION, "copilot_cli": sauber(harness_fassung(pr.get("lauf_konfig"), "copilot", "@github/copilot")),
            "ubuntu": ubuntu,
            "kernel": sauber(platform.release())}


# --- Vorpruefung der Arbeitsordner (Entscheidung vom 21.11.2026) ------------------------------------------------------------------
# Nachbildung der Regeln des Einschlusses (Crate einschluss: verbote.rs, profil.rs; Crate lauf: geschuetzte_ordner). Liegt ein
# Arbeitsordner an einem geschuetzten Ort, endet der echte Lauf vor dem ersten Start mit `probelauf.arbeitsordner_geschuetzt` statt
# spaeter mit `s1.ausnahme` (Code 5 des Einschlusses). Massgeblich bleibt der Einschluss; dies ist eine Teilmenge seiner Regeln.
GEHEIMNIS_TEILE = (".aws", ".config", ".docker", ".gnupg", ".kube", ".secrets-sync", ".ssh", "90-secrets", ".env")
SYSTEM_VERBOTEN = ("/", "/dev", "/etc", "/home", "/proc", "/root", "/run", "/sys", "/tmp", "/var/run", "/var/lib/docker")
SYSTEM_UNTER_VERBOTEN = ("/usr", "/run", "/proc", "/sys", "/dev", "/etc", "/opt")
WERKBANK_ORTE = ("/var/lib/werkbank", "/etc/werkbank")
ANMELDE_ORTE = (".copilot",)  # Anmeldespeicher der Harness im Heim (Teil von ANMELDE_ORDNER des Einschlusses)


def _innerhalb(pfad, ordner):
    """Wahr, wenn `pfad` gleich `ordner` ist oder darunter liegt (ganze Pfadteile)."""
    pfad, ordner = os.path.normpath(pfad), os.path.normpath(ordner)
    return pfad == ordner or pfad.startswith(ordner.rstrip("/") + "/")


def _geschuetzte_orte(k, heim):
    orte = list(WERKBANK_ORTE) + [os.environ.get("XDG_RUNTIME_DIR") or "/run/user/%d" % os.getuid()]
    orte += [os.path.join(heim, a) for a in ANMELDE_ORTE]
    for p in (k.get("vault"), (k.get("protokoll") or {}).get("ordner"), k.get("zaehlordner")):
        if isinstance(p, str) and p:
            orte.append(p)
    p = (k.get("einstellungen") or {}).get("pfad")
    if isinstance(p, str) and p:
        orte.append(os.path.dirname(p))
    return [os.path.realpath(o) for o in orte]


def arbeitsordner_befund(k, heim=None):
    """None, wenn jeder Arbeitsordner der Sitzungen erlaubt ist, sonst ein fester Grund ohne Pfad: `geheimnisteil`, `heim`,
    `system`, `geschuetzt`."""
    heim = os.path.realpath(heim or os.path.expanduser("~"))
    orte = _geschuetzte_orte(k, heim)
    for p in ablauf.arbeitsordner(k):
        if any(t in GEHEIMNIS_TEILE for t in p.split(os.sep)):
            return "geheimnisteil"
        if _innerhalb(heim, p):
            return "heim"  # das Heim selbst oder ein Ordner darueber
        if any(_innerhalb(s, p) for s in SYSTEM_VERBOTEN) or any(_innerhalb(p, s) for s in SYSTEM_UNTER_VERBOTEN):
            return "system"
        if any(_innerhalb(p, o) or _innerhalb(o, p) for o in orte):
            return "geschuetzt"
    return None


def arbeit_basis(konfig_pfad):
    """Ordner, unter dem der Probelauf seinen eigenen Arbeitsordner anlegt: im echten Lauf das Heim des Nutzers (die Arbeitsordner der
    Sitzungen liegen unter `$ARBEIT`, und der Einschluss verbietet geschuetzte Orte), sonst None (`$TMPDIR`). Liest nur das Feld `ziel`."""
    try:
        with open(konfig_pfad, encoding="utf-8") as f:
            ziel = json.load(f).get("ziel")
    except (OSError, ValueError, AttributeError):
        return None
    return os.path.expanduser("~") if ziel == "echt" else None


def voraussetzungen(k, hinweise=None, lage=None):  # noqa: C901
    """Gibt den Code der ersten fehlenden Voraussetzung oder None. Liest nie den Inhalt der Schluesseldatei. `hinweise` (Liste): bekommt
    feste Texte zur Voraussetzung (etwa den Code der Pruefinstanz), nie einen Inhalt. `lage` (Dict): bekommt `system_bwrap` aus der
    Ausgabe von `werkbank-pruefen`, die die Voraussetzung `befehl_zeilen` ohnehin liest."""
    if _offen(k):
        return "konfiguration.fehlt"
    if k.get("ziel") == "echt":
        grund = arbeitsordner_befund(k)
        if grund:
            print("Ein Arbeitsordner liegt an einem geschuetzten Ort (%s): der Probelauf startet nichts" % grund)
            if hinweise is not None:
                hinweise.append("arbeitsordner: " + grund)
            return "probelauf.arbeitsordner_geschuetzt"
    # Vorfuehr-Vault: jeder Marker steht in genau einer Notiz, der erlaubte Marker ohne Kundenbezug (`np`) in keiner
    treffer = {m: 0 for m in k["marker"].values()}
    erwartet = {m: (0 if n == "np" else 1) for n, m in k["marker"].items()}
    for wurzel, _, dateien in os.walk(k["vault"]):
        for n in dateien:
            p = os.path.join(wurzel, n)
            try:
                with open(p, encoding="utf-8", errors="replace") as f:
                    t = os.path.relpath(p, k["vault"]) + "\n" + f.read()
            except OSError:
                continue
            for m in treffer:
                if m in t:
                    treffer[m] += 1
    if any(c != erwartet[m] for m, c in treffer.items()):
        return "vault.unvollstaendig"
    if k["einstellungen"].get("aus_datei"):
        try:
            with open(k["einstellungen"]["pfad"], encoding="utf-8") as f:
                if konfig.pruefe_aufbau(k, json.load(f)):
                    return "einstellungen.abweichend"
        except (OSError, ValueError):
            return "einstellungen.abweichend"
    nur_cloud = k.get("umfang") == "nur_cloud"
    for v in k.get("probelauf", {}).get("voraussetzungen", []):
        if nur_cloud and (v["art"] == "pruefinstanz" or v["code"] == "modellweg.schluessel-fehlt"):
            continue  # Messbetrieb --nur-cloud: kein Gateway, keine Pruefinstanz, kein Schluessel des Gateways
        if v["art"] == "datei":
            if not os.path.isfile(v["pfad"]):  # nur Vorhandensein, nie lesen
                return v["code"]
        elif v["art"] == "befehl":
            try:
                p = subprocess.run(list(v["befehl"]), capture_output=True, timeout=float(v.get("frist_s", 20)))
            except (OSError, subprocess.TimeoutExpired):
                return v["code"]
            if p.returncode != 0:
                return v["code"]
        elif v["art"] == "befehl_zeilen":
            if _zeilen_befund(v, lage):
                return v["code"]
        elif v["art"] == "pruefinstanz":
            # Pruefung des Gateways ueber eine eigene Instanz des Weiterreichers (Vertrag 8, Abschnitt 4): Der Code kommt aus der Pruefung
            # (`gateway.nicht-erreichbar` oder `gateway.modell-gesperrt`), nicht aus dem Eintrag.
            code, hinweis = ablauf.pruefe_gateway(dict(k, gateway=dict(k["gateway"], pruefinstanz=v)))
            if hinweis and hinweise is not None:
                hinweise.append("gateway.pruefinstanz: " + hinweis)
            if code:
                return code
    return None


def _genau_eine(zeilen, p_a, p_b, wert_a, wert_b):
    """`wert_a`, wenn eine Zeile mit `p_a` beginnt und keine mit `p_b`; `wert_b` umgekehrt; sonst, und bei beiden zugleich, `unbekannt`.
    Ein Praefix `None` trifft nie zu."""
    def hat(p):
        return bool(p) and any(z.startswith(p) for z in zeilen)
    a, b = hat(p_a), hat(p_b)
    return wert_a if a and not b else (wert_b if b and not a else "unbekannt")


def bwrap_lage(zeilen, v):
    """Lage des System-bwrap aus den Zeilen von `werkbank-pruefen` (Nachtrag 20.11., 12). `ubuntu`, wenn eine Zeile mit dem Praefix
    `bwrap_praefix_ubuntu` beginnt; `gesperrt`, wenn eine Zeile mit `bwrap_praefix_gesperrt` beginnt (Wert `null`: dann nie); sonst,
    und bei beiden zugleich, `unbekannt`. Es bleibt keine Zeile erhalten, nur der Wert."""
    return _genau_eine(zeilen, v.get("bwrap_praefix_ubuntu"), v.get("bwrap_praefix_gesperrt"), "ubuntu", "gesperrt")


def profil_lage(zeilen, v):
    """Lage des Ubuntu-Profils bwrap-userns-restrict aus denselben Zeilen (Entscheidung vom 20.11.2026, Paketzug). `geladen`, wenn
    eine Zeile mit `profil_praefix_geladen` beginnt und keine mit `profil_praefix_entladen`; `entladen` umgekehrt; sonst `unbekannt`.
    Es bleibt keine Zeile erhalten, nur der Wert."""
    return _genau_eine(zeilen, v.get("profil_praefix_geladen"), v.get("profil_praefix_entladen"), "geladen", "entladen")


def _zeilen_befund(v, lage=None):
    """Voraussetzung der Art `befehl_zeilen` (etwa `werkbank-pruefen`): Auswertung nach Zeilen von stdout, der Exitcode zaehlt nicht.
    Wahr, wenn die Voraussetzung NICHT erfuellt ist: der Befehl laesst sich nicht starten oder ueberschreitet die Frist; keine
    Zeile beginnt mit `pruef.` (fail closed); eine Zeile beginnt mit `verboten_praefix`. Keine Zeile bleibt erhalten; aus denselben
    Zeilen kommen `lage["system_bwrap"]` und `lage["ubuntu_profil"]` (kein zweiter Aufruf)."""
    if lage is not None:
        lage["system_bwrap"] = "unbekannt"
        lage["ubuntu_profil"] = "unbekannt"
    try:
        p = subprocess.run(list(v["befehl"]), capture_output=True, timeout=float(v.get("frist_s", 20)))
    except (OSError, subprocess.TimeoutExpired):
        return True
    zeilen = p.stdout.decode("utf-8", "replace").splitlines()
    if lage is not None:
        lage["system_bwrap"] = bwrap_lage(zeilen, v)
        lage["ubuntu_profil"] = profil_lage(zeilen, v)
    if not any(z.startswith("pruef.") for z in zeilen):
        return True
    return any(z.startswith(v["verboten_praefix"]) for z in zeilen)


def offen_codes(r):
    """Die offenen Gruende eines Schritts als feste Codes (Testlauf vom 21.11.2026): je Eintrag von `offen` der Text vor dem ersten
    `: `, wenn er auf KENNUNG passt, sonst `ohne-kennung`. In der Reihenfolge von `schritte.py`, Doppel bleiben, hoechstens OFFEN_MAX.
    Kein Text nach dem Doppelpunkt kommt in Datei oder Konsole."""
    aus = []
    for o in r.get("offen") or []:
        c = o.split(": ", 1)[0] if isinstance(o, str) else ""
        aus.append(c if KENNUNG.match(c) else "ohne-kennung")
    return aus[:OFFEN_MAX]


def konsole_schritte(d, ergebnisse):
    """Die Zeilen der Konsole je Schritt: `ok`/`FEHL` mit dem Fehlercode, danach je offenem Grund `<schritt> offen: <code>` (dieselbe
    Liste wie `offen` in der Datei) und, wenn der Schritt Teile hat, `<schritt> teile: <n> gruen, <m> rot`. Nur Codes und Zahlen."""
    nach = {r["schritt"]: r for r in ergebnisse}
    aus = []
    for x in d["schritte"]:
        aus.append("  %s  %s  %s" % (x["schritt"], "ok   " if x["bestanden"] else "FEHL ", x["fehlercode"] or ""))
        for c in x.get("offen") or []:
            aus.append("  %s offen: %s" % (x["schritt"], c))
        teile = (nach.get(x["schritt"]) or {}).get("teile") or []
        if teile:
            n_ok = sum(1 for t_ in teile if t_["ok"])
            aus.append("  %s teile: %d gruen, %d rot" % (x["schritt"], n_ok, len(teile) - n_ok))
        z = ende_zeile(x["schritt"], (nach.get(x["schritt"]) or {}).get("messwerte") or {})
        if z:
            aus.append(z)
    return aus


def ende_zeile(schritt, mw):
    """Eine Zeile `  <schritt> ende: zustand <z>, code <c>, lauf <code1,code2|->, texte <name=n,...|->` fuer einen Schritt, in dem eine Sitzung ohne
    Eingabefeld endete (Messwerte `ende_*`, Diagnose bei Sitzungsende); sonst None. Nur Codes und Zahlen: Zustand und Code der
    Sitzung, die `lauf.*`-Codes, die Zaehler fester Texte ungleich 0 (Leerzeichen der Namen als `_`). Nie Inhalt des Bildes, nie ein Pfad.
    Die Muster von erwartung-m1.py (Schritt, offen, teile, rc) treffen sie nicht."""
    if "ende_texte" not in mw:
        return None
    zustand = mw.get("ende_zustand")
    code = mw.get("ende_code")
    codes = [c for c in (mw.get("ende_lauf_codes") or []) if isinstance(c, str) and re.match(r"^lauf\.[a-z][a-z0-9_]*$", c)]
    texte = ["%s=%d" % (n.replace(" ", "_"), v) for n, v in (mw.get("ende_texte") or {}).items() if isinstance(v, int) and not isinstance(v, bool) and v > 0]
    z = zustand if zustand in ("laeuft", "beendet") else "-"
    c = str(code) if isinstance(code, int) and not isinstance(code, bool) else "-"
    return "  %s ende: zustand %s, code %s, lauf %s, texte %s" % (schritt, z, c, ",".join(codes) or "-", ",".join(texte) or "-")


def fehlercode(r):
    if r["ergebnis"] == "gruen":
        return None
    s = r["schritt"].lower()
    if r.get("code_fest"):
        return r["code_fest"]
    if r["ergebnis"] == "rot" and r.get("code_rot"):
        return r["code_rot"]
    if r["ergebnis"] == "offen" and r.get("code_offen"):
        return r["code_offen"]
    if r["ergebnis"] == "uebersprungen":
        return s + ".uebersprungen"
    if r["fehler"]:
        return s + ".ausnahme"
    for i, t in enumerate(r["teile"], 1):
        if not t["ok"]:
            return "%s.teil%d" % (s, min(i, 40))
    return s + ".offen"


def baue_ergebnis(beginn, ende, vers, starts, browser, voraus, ergebnisse, ca=None, system_bwrap="unbekannt", ubuntu_profil="unbekannt",
                  eingaben=0, umfang="voll", gegenprobe=None):
    schritte = []
    nach = {r["schritt"]: r for r in ergebnisse}
    for s in SCHRITTE:
        r = nach.get(s)
        if r is None:
            schritte.append({"schritt": s, "bestanden": False, "fehlercode": s.lower() + ".nicht-gelaufen", "dauer_ms": 0, "offen": []})
        else:
            schritte.append({"schritt": s, "bestanden": r["ergebnis"] == "gruen", "fehlercode": fehlercode(r), "dauer_ms": r["dauer_ms"],
                             "offen": offen_codes(r)})
    return {"fassung": FASSUNG, "art": "probelauf-ergebnis", "beginn": _utc(beginn), "ende": _utc(ende), "versionen": vers,
            "copilot_starts": starts, "cloud_eingaben": eingaben, "umfang": umfang,
            "s1_gegenprobe": gegenprobe or {"weg": None, "neue_sitzungen": None, "neue_kinder": None},
            "kennzeichnung": "Teilumfang" if umfang == "nur_cloud" else None, "browser": browser, "ca": ca or {"cli_nimmt_an": None, "ausgang": None},
            "system_bwrap": system_bwrap, "ubuntu_profil": ubuntu_profil, "voraussetzung": voraus, "schritte": schritte,
            "bestanden": voraus is None and umfang == "voll" and all(x["bestanden"] for x in schritte)}


def schreibe(pfad, d):
    text = json.dumps(d, indent=1, sort_keys=True) + "\n"
    tmp = pfad + ".neu"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(text)
    os.replace(tmp, pfad)


# --- Dienste des Zielrechners (Nachtrag 21.11., 16) -----------------------------------------------------------------
DIENST_FRIST_S = 20  # Frist je Aufruf von systemctl
DIENST_UMGEBUNG = ("PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS")  # aus der Umgebung nur diese Namen
DIENST_NAME = re.compile(r"^[a-z][a-z0-9@._-]{0,63}$")  # nie mit `-` am Anfang, sonst liest systemctl den Namen als Schalter


def _dienst_umgebung():
    return {n: os.environ[n] for n in DIENST_UMGEBUNG if n in os.environ}


def _systemctl(args, env):
    """Exitcode von `systemctl --user <args>`, oder None, wenn der Aufruf nicht startbar ist oder die Frist ueberschreitet. Stdin ist
    /dev/null; die Ausgabe wird verworfen, damit keine Zeile von systemctl in eine Datei oder ein Protokoll gelangt."""
    try:
        p = subprocess.run(["systemctl", "--user"] + list(args), stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, env=env, timeout=DIENST_FRIST_S)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return p.returncode


def _laeuft(einheit, env):
    return _systemctl(["is-active", einheit], env) == 0


def _datei_da(pfad):
    """Vorhanden und nicht leer. Der Inhalt wird nicht gelesen; den Token liest nur der Bruecken-Client."""
    try:
        return os.path.getsize(pfad) > 0
    except OSError:
        return False


def _warte_auf_bruecke(b):
    """Wahr, wenn Port- und Token-Datei der Bruecke innerhalb von `frist_s` da sind."""
    dateien = [b[n] for n in ("port_datei", "token_datei") if b.get(n)]
    ende = time.monotonic() + float(b.get("frist_s", 10))
    while not all(_datei_da(p) for p in dateien):
        if time.monotonic() >= ende:
            return False
        time.sleep(0.1)
    return True


def dienste_bereit(k):
    """Startet die Einheiten aus `dienste` in der Reihenfolge der Liste, die nicht laufen, und wartet dann auf die Dateien der Bruecke.
    Gibt (code, gestartet). `code` ist None, wenn die Voraussetzung erfuellt ist; sonst `bruecke.dienst-startet-nicht` oder
    `konfiguration.fehlt` (ungueltiger Name). `gestartet` sind die Einheiten, die dieser Lauf gestartet hat, in der Reihenfolge des Starts;
    die Aufrufstelle stoppt sie in jedem Fall (`dienste_beenden`). Einheiten, die schon laufen, bleiben unberuehrt. Ohne Liste geschieht nichts."""
    liste = k.get("dienste") or []
    if not liste:
        return None, []
    if not all(isinstance(n, str) and DIENST_NAME.match(n) for n in liste):
        return "konfiguration.fehlt", []
    env = _dienst_umgebung()
    gestartet = []
    for einheit in liste:
        if _laeuft(einheit, env):
            print("Dienst %s: laeuft schon, der Probelauf startet und stoppt ihn nicht" % einheit)
            continue
        if _systemctl(["start", einheit], env) != 0:
            print("Dienst %s: Start fehlgeschlagen" % einheit)
            return "bruecke.dienst-startet-nicht", gestartet
        gestartet.append(einheit)
        print("Dienst %s: vom Probelauf gestartet" % einheit)
    if not _warte_auf_bruecke(k["bruecke"]):
        print("Dateien der Bruecke nach %s s nicht da" % k["bruecke"].get("frist_s"))
        return "bruecke.dienst-startet-nicht", gestartet
    return None, gestartet


def dienste_beenden(gestartet):
    """Stoppt genau die Einheiten, die dieser Lauf gestartet hat, in umgekehrter Reihenfolge, und prueft danach, dass sie nicht mehr
    laufen. Gibt Meldungen (leer: sauber)."""
    meld = []
    env = _dienst_umgebung()
    for einheit in reversed(gestartet):
        if _systemctl(["stop", einheit], env) != 0 or _laeuft(einheit, env):
            meld.append("Dienst %s nicht gestoppt" % einheit)
        else:
            print("Dienst %s: vom Probelauf gestoppt" % einheit)
    return meld


# --- Markerdatei fuer den Kern (Nachtrag 2 bis 5, Entscheidung vom 21.11.2026) -------------------------------------------
MARKER_NAME = "marker.json"
MARKER_TMP = ".marker.json.neu"


def _lies_kern_zaehlordner(k):
    """Im echten Lauf legt `/etc/werkbank/kern.json` (Feld `lauf.marker_ordner`, vom postinst geschrieben) den Zaehlordner fest. Der Laeufer
    liest nur diesen Wert (die Datei gehoert root). Weicht er von `zaehlordner` der Konfiguration ab, gilt der Wert aus `kern.json`, und es
    steht ein fester Hinweis (ohne Pfad) auf der Konsole. Ist die Datei nicht lesbar oder fehlt das Feld, wird `zaehlordner` None:
    die Zaehlung ist dann `offen` (`<schritt>.zaehlung_cloud_unbelegt`), nie gruen. Ohne `probelauf.kern_datei` tut die Funktion nichts."""
    pfad = k.get("probelauf", {}).get("kern_datei")
    if not pfad or k["ziel"] != "echt":
        return
    try:
        with open(pfad, "rb") as f:
            wert = json.loads(f.read(65537).decode("utf-8"))["lauf"]["marker_ordner"]
        if not isinstance(wert, str) or not os.path.isabs(wert):
            raise ValueError("marker_ordner")
    except (OSError, ValueError, KeyError, TypeError):
        print("Hinweis: kern.json nicht lesbar oder ohne marker_ordner, die Zaehlung bleibt offen")
        k["zaehlordner"] = None
        return
    alt = k.get("zaehlordner")  # ohne Wert kein realpath("\0"): das wirft ab Python 3.15 ValueError
    if not isinstance(alt, str) or not alt or os.path.realpath(wert) != os.path.realpath(alt):
        print("Hinweis: zaehlordner der Konfiguration weicht von kern.json ab, der Wert aus kern.json gilt")
    k["zaehlordner"] = wert


MARKER_ORDNER = "werkbank-probelauf"


def _lies_kern_marker_datei(k):
    """Im echten Lauf legt `/etc/werkbank/kern.json` (Feld `lauf.marker_datei`, vom postinst geschrieben) den Ort der Markerdatei fest; der
    Kern liest genau diesen Pfad (Entscheidung vom 21.11.2026). Der Laeufer nimmt ihn statt des Werts aus seiner Konfiguration, die nur noch die
    Erwartung ist (abweichend: fester Hinweis ohne Pfad). Weil der Laeufer am Ende genau diese Datei und ihren Ordner loescht, nimmt er den
    Pfad nur in der festen Form `<lauf>/<uid des Laeufers>/werkbank-probelauf/marker.json` an (`<lauf>`: `probelauf.lauf_wurzel`, Vorgabe
    `/run/user`). Sonst, und wenn `kern.json` nicht lesbar ist oder das Feld fehlt: fail closed mit `probelauf.marker_datei_fehlt`
    (Voraussetzung), nichts wird gestartet und keine Datei angelegt. Ohne `probelauf.kern_datei` bleibt der Wert der Konfiguration."""
    pk = k.get("probelauf", {})
    if not pk.get("kern_datei") or k["ziel"] != "echt":
        return None
    eltern = os.path.join(pk.get("lauf_wurzel", "/run/user"), str(os.getuid()))
    try:
        with open(pk["kern_datei"], "rb") as f:
            wert = json.loads(f.read(65537).decode("utf-8"))["lauf"]["marker_datei"]
        if (not isinstance(wert, str) or not os.path.isabs(wert) or os.path.normpath(wert) != wert
                or wert != os.path.join(eltern, MARKER_ORDNER, MARKER_NAME)):
            raise ValueError("marker_datei")
    except (OSError, ValueError, KeyError, TypeError):
        print("kern.json nicht lesbar, ohne marker_datei oder mit einem Ort ausserhalb von <lauf>/<uid>/werkbank-probelauf/: "
              "der Probelauf startet nichts")
        return "probelauf.marker_datei_fehlt"
    alt = pk.get("marker_datei")
    if not isinstance(alt, str) or not alt or os.path.normpath(alt) != wert:
        print("Hinweis: marker_datei der Konfiguration weicht von kern.json ab, der Wert aus kern.json gilt")
    pk["marker_datei"] = wert
    return None


def _geteilt(pfad):
    """(Elternordner, Name) eines Pfads."""
    return os.path.dirname(pfad), os.path.basename(pfad)


def _ordner_fd(name, dir_fd=None):
    return os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=dir_fd)


def _entferne_namen(dfd):
    """Loescht genau die zwei Namen der Markerdatei im Ordner ueber den Ordnerdeskriptor. Gibt wahr, wenn etwas da war."""
    da = False
    for n in (MARKER_NAME, MARKER_TMP):
        try:
            os.unlink(n, dir_fd=dfd)
            da = True
        except FileNotFoundError:
            pass
    return da


def marker_loeschen(pfad):
    """Loescht `marker.json` und den Ordner `werkbank-probelauf`, nur genau diese Namen, ueber Ordnerdeskriptoren, ohne einem Link zu
    folgen. Gibt (ok, da): `ok` falsch, wenn der Ordner fremden Inhalt hat oder ein Link ist; `da` wahr, wenn etwas zu loeschen war."""
    ordner, _ = _geteilt(pfad)
    eltern, onm = _geteilt(ordner)
    try:
        pfd = _ordner_fd(eltern)
    except OSError:
        return True, False
    try:
        try:
            dfd = _ordner_fd(onm, pfd)
        except FileNotFoundError:
            return True, False
        except OSError:
            return False, True  # ein Link oder kein Ordner: nicht anfassen
        try:
            _entferne_namen(dfd)
        finally:
            os.close(dfd)
        try:
            os.rmdir(onm, dir_fd=pfd)
        except OSError:
            return False, True  # fremder Inhalt im Ordner
        return True, True
    finally:
        os.close(pfd)


def marker_ablegen(pfad, liste):
    """Legt `<pfad>` mit genau `{"liste": [...]}` an: Ordner 0700, Datei 0600 mit `O_EXCL` und atomar (temporaere Datei im selben Ordner,
    `fsync`, `rename`). Die Datei traegt nur die Liste, kein Feld `ordner` (Formfehler: der Kern startet dann nicht). Raeumt vorher Reste
    der beiden Namen. Gibt wahr, wenn Reste da waren; wirft OSError, wenn der Ordner nicht sicher anzulegen ist."""
    ordner, name = _geteilt(pfad)
    eltern, onm = _geteilt(ordner)
    ok, reste = marker_loeschen(pfad)
    if not ok:
        raise OSError("Ordner der Markerdatei nicht raeumbar")
    pfd = _ordner_fd(eltern)
    try:
        os.mkdir(onm, 0o700, dir_fd=pfd)
        dfd = _ordner_fd(onm, pfd)
        try:
            os.fchmod(dfd, 0o700)
            fd = os.open(MARKER_TMP, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=dfd)
            try:
                os.write(fd, json.dumps({"liste": list(liste)}, separators=(",", ":")).encode("utf-8") + b"\n")
                os.fsync(fd)
            finally:
                os.close(fd)
            os.rename(MARKER_TMP, name, src_dir_fd=dfd, dst_dir_fd=dfd)
            os.fsync(dfd)
        finally:
            os.close(dfd)
    finally:
        os.close(pfd)
    return reste


class ProbeDienste:
    """Dienste mit Markerdatei: Der Kern liest beim Start `marker.json` (nur im echten Lauf). Ablauf in `bereit`: (1) laeuft eine Sitzung,
    `bruecke.sitzungen_laufen`, nichts wird neu gestartet; (2) Reste raeumen, Datei ablegen; (3) Kern und Bruecke neu starten, wenn sie
    liefen, sonst starten; Warten auf die Dateien der Bruecke. `beenden` (immer, auch bei SIGINT und SIGTERM): Datei und Ordner loeschen,
    dann den Zustand von vorher herstellen: lief die Einheit vorher, wird sie neu gestartet (jetzt ohne Datei), sonst gestoppt."""

    def __init__(self, k):
        self.k = k
        self.pfad = k["probelauf"]["marker_datei"]
        self.vorher = {}
        self.beruehrt = []

    def _sitzungen(self):
        """Zahl laufender Sitzungen laut Sitzungsliste der Bruecke, oder None, wenn sie sich nicht lesen laesst (Port- oder Token-Datei fehlen,
        die Bruecke antwortet nicht, keine Liste). Nur wenn keine Einheit aus `dienste` aktiv ist, ist die Zahl sicher 0; das entscheidet
        `bereit`, nicht diese Funktion."""
        from . import bruecke as br
        b = self.k["bruecke"]
        if not all(_datei_da(b[n]) for n in ("port_datei", "token_datei") if b.get(n)):
            return None
        try:
            with open(b["port_datei"], encoding="utf-8") as f:
                port = int(f.read().strip().rsplit(":", 1)[-1])
            with open(b["token_datei"], encoding="utf-8") as f:
                token = f.read().strip()
            c = br.Bruecke({"url": "http://127.0.0.1:%d" % port, "token": token, "frist_s": 3})
            try:
                return c.laufende_sitzungen(3.0)
            finally:
                c.zumachen()
        except (OSError, ValueError, br.StromFehler):
            return None

    def bereit(self):
        k = self.k
        liste = k.get("dienste") or []
        if not all(isinstance(n, str) and DIENST_NAME.match(n) for n in liste):
            return "konfiguration.fehlt"
        env = _dienst_umgebung()
        self.vorher = {e: _laeuft(e, env) for e in liste}
        if any(self.vorher.values()):
            # Ist eine Einheit aktiv, muss die Zahl lesbar sein: ein Neustart des Kerns beendet fremde Sitzungen. Nichts aktiv: sicher 0.
            n = self._sitzungen()
            if n is None:
                print("Die Zahl laufender Sitzungen ist nicht lesbar: der Probelauf startet nichts neu")
                return "bruecke.sitzungen_unbekannt"
            if n:
                print("Eine Sitzung laeuft: der Probelauf startet nichts neu")
                return "bruecke.sitzungen_laufen"
        try:
            if marker_ablegen(self.pfad, k["marker"].values()):
                print("Hinweis: Reste der Markerdatei geraeumt")
        except OSError:
            print("Markerdatei nicht anlegbar")
            return "bruecke.dienst-startet-nicht"
        for e in liste:
            verb = "restart" if self.vorher[e] else "start"
            self.beruehrt.append(e)
            if _systemctl([verb, e], env) != 0:
                print("Dienst %s: %s fehlgeschlagen" % (e, verb))
                return "bruecke.dienst-startet-nicht"
            print("Dienst %s: vom Probelauf %s" % (e, "neu gestartet" if verb == "restart" else "gestartet"))
        if not _warte_auf_bruecke(k["bruecke"]):
            print("Dateien der Bruecke nach %s s nicht da" % k["bruecke"].get("frist_s"))
            return "bruecke.dienst-startet-nicht"
        return None

    def beenden(self):
        meld = []
        ok, _ = marker_loeschen(self.pfad) if self.pfad else (True, False)  # ohne gueltigen Ort (marker_datei_fehlt) gibt es nichts zu loeschen
        if not ok:
            meld.append("Markerdatei nicht geloescht")
        env = _dienst_umgebung()
        for e in reversed(self.beruehrt):
            if self.vorher.get(e):
                if _systemctl(["restart", e], env) != 0:
                    meld.append("Dienst %s nicht wiederhergestellt" % e)
                else:
                    print("Dienst %s: vom Probelauf ohne Markerdatei neu gestartet" % e)
            elif _systemctl(["stop", e], env) != 0 or _laeuft(e, env):
                meld.append("Dienst %s nicht gestoppt" % e)
            else:
                print("Dienst %s: vom Probelauf gestoppt" % e)
        return meld


def _ende_durch_signal(n, _frame):
    raise SystemExit(128 + n)


class _SignaleAlsEnde:
    """SIGINT und SIGTERM beenden den Lauf ueber SystemExit, damit `finally` die Markerdatei loescht und den Zustand herstellt."""

    def __enter__(self):
        self.alt = {}
        for s in (signal.SIGINT, signal.SIGTERM):
            try:
                self.alt[s] = signal.signal(s, _ende_durch_signal)
            except ValueError:  # nicht im Hauptfaden
                pass
        return self

    def ignorieren(self):
        """Vor dem Aufraeumen: weitere SIGINT und SIGTERM werden ignoriert, sonst bricht der Handler das Aufraeumen ab (Markerdatei oder
        gestoppte Dienste blieben zurueck). Der Exitcode des ersten Signals bleibt; `__exit__` stellt die alten Handler wieder her."""
        for s in self.alt:
            signal.signal(s, signal.SIG_IGN)

    def __exit__(self, *a):
        for s, h in self.alt.items():
            signal.signal(s, h)


def lauf(k, arbeit, ergebnis_pfad, hier, fehler=(), nur=(), browser=None):
    """Der ganze Probelauf. Gibt (Exitcode, Ergebnis-Dict, Ergebnisse, Meldungen)."""
    beginn = time.time()
    messen.diagnose_leeren()
    vers = versionen(k, hier)
    hinweise = []
    lage = {"system_bwrap": "unbekannt", "ubuntu_profil": "unbekannt"}
    voraus = voraussetzungen(k, hinweise, lage)
    gestartet, ergebnisse, meldungen, info = [], [], [], {"starts": 0, "browser": "pruefclient"}
    probe = ProbeDienste(k) if k["ziel"] == "echt" and k.get("probelauf", {}).get("marker_datei") and k.get("dienste") else None
    signale = _SignaleAlsEnde() if probe else None
    if signale:
        signale.__enter__()
    try:
        if voraus is None:
            _lies_kern_zaehlordner(k)
        if voraus is None and probe is not None:
            voraus = _lies_kern_marker_datei(k)
            probe.pfad = None if voraus else k["probelauf"]["marker_datei"]
        if voraus is None and probe is not None:
            voraus = probe.bereit()
        elif voraus is None and k["ziel"] == "echt" and k.get("dienste"):
            # Kern-Dienst und Bruecke starten, falls sie nicht laufen; `finally` stoppt, was dieser Lauf gestartet hat
            voraus, gestartet = dienste_bereit(k)
        if voraus is None:
            try:
                ergebnisse, meldungen, info = ablauf.fuehre_aus(k, arbeit, fehler, nur, browser)
            except ablauf.AufbauFehler as e:
                voraus = str(e) if str(e) in VORAUSSETZUNGEN else "bruecke.nicht-erreichbar"
                ergebnisse, meldungen, info = [], [], {"starts": 0, "browser": "pruefclient"}
                if e.hinweis:
                    hinweise.append("gateway.pruefinstanz: " + e.hinweis)
    finally:
        if signale:
            signale.ignorieren()
        try:
            meldungen = meldungen + (probe.beenden() if probe else dienste_beenden(gestartet))
        finally:
            if signale:
                signale.__exit__()
    d = baue_ergebnis(beginn, time.time(), vers, info["starts"], info["browser"], voraus, ergebnisse, info.get("ca"),
                      lage["system_bwrap"], lage["ubuntu_profil"], info.get("cloud_eingaben", 0), k.get("umfang", "voll"), info.get("gegenprobe"))
    if "ergebnis-leck" in fehler:  # Mutationsprobe des Falls: ein Marker gelangt in die Datei
        d["versionen"]["paket"] = k["marker"]["nk"]
    schreibe(ergebnis_pfad, d)
    messen.diagnose_schreibe(ergebnis_pfad)  # nur wenn es veraenderlichen Text gibt; die Konsole nennt nur die feste Zeile (`probelauf`)
    if voraus is not None:
        return 3, d, ergebnisse, meldungen + hinweise
    if k.get("umfang") == "nur_cloud":
        # nie 0: ein Teillauf ist kein Probelauf. Ein roter Schritt bleibt sichtbar (Exit 1 wie im vollen Lauf), sonst Exit 4.
        return (1 if any(r["ergebnis"] == "rot" for r in ergebnisse) else EXIT_TEILUMFANG), d, ergebnisse, meldungen
    return (0 if d["bestanden"] and not meldungen else 1), d, ergebnisse, meldungen


def probelauf(argv, hier):
    ap = argparse.ArgumentParser(prog="drehbuch.py probelauf")
    ap.add_argument("--ergebnis", required=True)
    ap.add_argument("--konfig")
    ap.add_argument("--arbeit")
    ap.add_argument("--nutzer")
    ap.add_argument("--vault", help="ueberschreibt `vault` der Konfiguration")
    ap.add_argument("--fehler", default="", help="nur mit einer Attrappen-Konfiguration")
    ap.add_argument("--nur", default="")
    ap.add_argument("--browser", choices=["aus", "auto", "playwright"])
    ap.add_argument("--nur-cloud", action="store_true", dest="nur_cloud",
                    help="Messbetrieb: nur die Cloud-Route; Ergebnis nie bestanden, Exit 1 bei rotem Schritt, sonst 4, nie 0 (README, nicht der Weg zum Probelauf)")
    try:
        a = ap.parse_args(argv)
    except SystemExit:
        return 2
    fehler = [f for f in a.fehler.split(",") if f]
    eigener = a.arbeit is None
    kpfad = a.konfig or os.path.join(hier, "konfig.probelauf.json")
    basis = arbeit_basis(kpfad)
    arbeit = os.path.realpath(a.arbeit) if a.arbeit else messen.neuer_tempordner(prefix="werkbank-probelauf-" if basis else "dbl-", dir=basis)
    os.makedirs(arbeit, exist_ok=True)
    try:
        k = konfig.lade(kpfad, hier, arbeit, a.nutzer)
    except konfig.KonfigFehler as e:
        print("Konfigurationsfehler: %s" % e, file=sys.stderr)
        if eigener:
            messen.sicher_loeschen(arbeit)
        return 2
    if a.vault:
        k["vault"] = os.path.realpath(a.vault)
    k["umfang"] = "nur_cloud" if a.nur_cloud else "voll"
    if fehler and k["ziel"] != "attrappe":
        print("--fehler gibt es nur gegen die Attrappen", file=sys.stderr)
        return 2
    code, d, ergebnisse, meldungen = lauf(k, arbeit, a.ergebnis, hier, fehler, [n for n in a.nur.split(",") if n], a.browser)
    # Ausgabe fuer den Menschen: nur feste Codes, keine Inhalte
    print("Probelauf: %s" % ("bestanden" if d["bestanden"] else "NICHT bestanden"))
    if d["kennzeichnung"]:
        print("%s: nur die Cloud-Route gemessen, kein Probelauf" % d["kennzeichnung"])
    if d["voraussetzung"]:
        print("Voraussetzung fehlt: %s" % d["voraussetzung"])
    for m in meldungen:  # auch Meldungen ohne Voraussetzung, etwa ein Dienst, der nicht zu stoppen war
        print("  Hinweis: %s" % m)
    for z in konsole_schritte(d, ergebnisse):
        print(z)
    if messen.diagnose_zeile():
        print(messen.diagnose_zeile())
    print("Ergebnisdatei: %s" % a.ergebnis)
    if eigener:
        messen.sicher_loeschen(arbeit)
    return code


# --- Fall: Inhaltsfreiheit -------------------------------------------------------------------------------------------
def selbsttest(argv, hier):
    vault = argv[argv.index("--vault") + 1] if "--vault" in argv[:-1] else None
    """Lauf gegen die Attrappen mit bekannten Werten; Zaehlung jedes Werts in der Datei, erwartet 0; Gegenprobe mit Leck."""
    nutzer = "qnutzer77"
    schluessel = "gw-0123456789abcdef0123456789abcdef"
    arbeit = messen.neuer_tempordner(prefix="dbl-")
    ok_alle = True
    try:
        for leck in (False, True):
            k = konfig.lade(os.path.join(hier, "konfig.attrappe.json"), hier, arbeit, nutzer)
            k["gateway"]["schluessel"] = schluessel
            pfad = os.path.join(arbeit, "ergebnis-%s.json" % ("leck" if leck else "sauber"))
            sub = os.path.join(arbeit, "lauf-%d" % int(leck))
            os.makedirs(sub)
            k2 = konfig.lade(os.path.join(hier, "konfig.attrappe.json"), hier, sub, nutzer)
            k2["gateway"]["schluessel"] = schluessel
            if vault:
                k2["vault"] = os.path.realpath(vault)
            code, d, _, _ = lauf(k2, sub, pfad, hier, ["ergebnis-leck"] if leck else [])
            with open(pfad, "rb") as f:
                roh = f.read()
            bekannt = {"Marker N-K": k2["marker"]["nk"], "Marker N-I": k2["marker"]["ni"], "Marker N-S": k2["marker"]["ns"],
                       "Titel N-K": k2["notiz_nk"]["titel"], "Pfad N-K": k2["notiz_nk"]["rel"], "Testschluessel": schluessel,
                       "Adresse 127.0.0.1": "127.0.0.1", "Adresse Bruecke": "http://127.0.0.1", "Benutzername": nutzer,
                       "Arbeitsordner": sub, "Frage": k2["frage"], "Kunde": k2["kunde"], "Vault": k2["vault"]}
            zaehler = {n: roh.count(w.encode("utf-8")) for n, w in bekannt.items()}
            form = pruefe_form(json.loads(roh.decode("utf-8")))
            rot = [n for n, c in zaehler.items() if c]
            erwartet_rot = leck
            gut = (bool(rot) == erwartet_rot) and not form and (leck or code in (0, 1))
            ok_alle = ok_alle and gut
            print("%-5s %s: Lauf-Exit %s, Zaehler ungleich 0: %s, Form: %s" % ("OK" if gut else "FALSCH",
                  "Fall mit eingebautem Leck (muss rot werden)" if leck else "Fall ohne Leck (muss 0 zaehlen)", code,
                  rot or "keine", "geschlossen" if not form else form))
    finally:
        messen.sicher_loeschen(arbeit)
    print("Ergebnis: %s" % ("GRUEN" if ok_alle else "FALSCH"))
    return 0 if ok_alle else 1
