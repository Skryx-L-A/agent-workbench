"""Ablauf eines Laufs: Ziel aufbauen, Schritte nacheinander ausfuehren, aufraeumen. Gemeinsam fuer `drehbuch.py --ziel` und
`drehbuch.py probelauf`."""
from __future__ import annotations

import os
import time

from . import bruecke, lauf, messen, schritte, seite, ziel

NAMEN = {"S1": "Sitzung mit Cloud-Modell starten", "S2": "Der Einschluss haelt", "S3": "Die Wissenssuche sperrt die Kundennotizen",
         "S4": "Die Sperre steht im Protokoll, die Kette haelt", "S5": "Das lokale Modell bekommt die Kundennotiz mit Freigabe, nicht die ohne",
         "E1": "Route haengt am Start", "E2": "Notizen liegen ausserhalb", "E3": "Suche ist ein eigener Prozess",
         "E4-lokal": "Der Schluessel des Gateways bleibt draussen (lokal)",
         "E4-cloud": "Das Token der CLI bleibt draussen (cloud)", "E5": "Die Netzgrenze haelt", "E6": "Die Ordnermarke haelt"}


NUR_LOKAL = ("S5", "E4-lokal", "E5", "E6")  # Schritte der lokalen Route: im Messbetrieb --nur-cloud `offen` mit `<schritt>.nur_cloud`


class AufbauFehler(Exception):
    def __init__(self, text, hinweis=None):
        super().__init__(text)
        self.hinweis = hinweis  # fester Text zur Voraussetzung (etwa der Code der Pruefinstanz `mw.schluessel_datei`), nie ein Inhalt


def arbeitsordner(k):
    """Arbeitsordner der Sitzungen (aufgeloest, ohne Doppel)."""
    erg = []
    for art in ("cloud", "lokal"):
        p = k["start"][art].get("arbeitsordner")
        if p and os.path.realpath(p) not in erg:
            erg.append(os.path.realpath(p))
    return erg


def pruefe_gateway(k, fehler=()):
    """Gateway-Pruefung ueber die eigene Pruefinstanz des Weiterreichers (`gateway.pruefinstanz` oder Voraussetzung der Art
    `pruefinstanz`). Ziel und Modell der Route `lokal` kommen aus der Einstellungsdatei des Laufs. Gibt (code, hinweis)."""
    pi = k["gateway"]["pruefinstanz"]
    ziel, modell = messen.gateway_ziel_modell(k["einstellungen"]["pfad"], k["routen"]["lokal"])
    if not ziel or not modell:
        return "gateway.nicht-erreichbar", "einstellungen"
    programm = list(pi["programm"])
    if k["ziel"] == "attrappe" and fehler:
        programm += ["--fehler", ",".join(fehler)]
    return messen.gateway_pruefinstanz(programm, k["routen"]["lokal"], ziel, modell, pi["schluessel_datei"], ausserhalb=arbeitsordner(k))


def zaehldateien_entfernen(k, sitzungen):
    """Entfernt `<zaehlordner>/<sitzung>.suche.json` und `<sitzung>.modellweg.json` der Sitzungen, die dieser Lauf selbst gestartet hat
    (Befund eines Testlaufs: nach einem Neustart des Kerns beginnt die Kennung wieder bei s-1). Nur im echten Lauf; gegen die Attrappen
    geschieht nichts. Der Aufrufer ruft das nach dem letzten Lesen und nach dem Ende des Weiterreichers. Gibt die festen Meldungen
    (ohne Pfad), je Meldung einmal; ein fremder Eintrag ist nur eine Meldung und nie `offen` in einem Schritt."""
    zo = k.get("zaehlordner")
    if k.get("ziel") != "echt" or not zo:
        return []
    meld = []
    for sid in sitzungen:
        for endung in (".suche.json", ".modellweg.json"):
            _, hinweis = messen.sitzungsdatei_entfernen(os.path.join(zo, "%s%s" % (sid, endung)), sid, endung)
            if hinweis and hinweis not in meld:
                meld.append(hinweis)
    return meld


def ausnahme_code(kennung, e):
    """Fester Code fuer eine Ausnahme in einem Schritt (Entscheidung vom 21.11.2026): bekannte Ausnahmen mit eigenem Code, sonst None
    (dann `<schritt>.ausnahme`). `kein_eingabefeld`: der Laeufer hat nichts gesendet, weil das Eingabefeld beim Start ausblieb;
    `harness_beendet`: dabei war die Sitzung schon beendet; `zug_frist`: ein Zug kam nicht binnen seiner Frist zu FERTIG; `antwort_ohne_merkmal`: die CLI war vor der Frist fertig, ohne FERTIG (Bild
    still, Feld ohne den Text, keine Arbeitsanzeige); `nicht_abgesendet`: der Text eines Zugs
    stand nie im Raster oder nach dem Enter noch im Eingabefeld."""
    k = kennung.lower()
    if isinstance(e, lauf.ZaehlDateiFehler):
        return "zaehldatei.inhalt"
    if isinstance(e, lauf.EingabeLimit):
        return "cloud.eingaben-limit"
    if isinstance(e, lauf.StartLimit):
        return "copilot.start-limit"
    if isinstance(e, lauf.KeinEingabefeld):
        return "%s.%s" % (k, "harness_beendet" if e.zustand == "beendet" else "kein_eingabefeld")
    if isinstance(e, lauf.AntwortOhneMerkmal):  # vor Zeitueberschreitung (Unterklasse)
        return "%s.antwort_ohne_merkmal" % k
    if isinstance(e, lauf.Zeitueberschreitung):
        return "%s.zug_frist" % k
    if isinstance(e, lauf.NichtAbgesendet):
        return "%s.nicht_abgesendet" % k
    return None


def schritt_nach_ausnahme(kennung, marke):
    """Der zuletzt angelegte Schritt dieser Kennung seit `marke` (mit allen schon vermerkten Teilen), sonst ein neuer leerer."""
    for s in reversed(schritte.ANGELEGT[marke:]):
        if s.kennung == kennung:
            return s
    return schritte.Schritt(kennung, NAMEN[kennung])


def fuehre_aus(k, arbeit, fehler=(), nur=(), browser=None):
    """Gibt (ergebnisse, meldungen, info). `ergebnisse`: Liste von Schritt-Dicts mit `dauer_ms`. Wirft AufbauFehler, wenn das
    Ziel nicht aufgebaut werden konnte."""
    del schritte.ANGELEGT[:]
    messen.diagnose_leeren()  # Text mit veraenderlichem Inhalt geht nur in die Diagnosedatei (`messen.diagnose_*`), nie in den Bericht
    z = ziel.Ziel(k, arbeit, fehler)
    ergebnisse, meldungen = [], []
    objekte = {}  # Schritt-Objekte, bis die Suche nach dem Schluessel die beiden E4 ergaenzt hat
    L = None
    modus = browser or k.get("browser", {}).get("modus", "aus")
    seite_b = None
    art_browser = "pruefclient"
    try:
        t_bruecke = time.monotonic()
        try:
            b = z.starten()
            messen.teilzeit_notiere("s1.bruecke_bereit", time.monotonic() - t_bruecke)  # Aufbau des Ziels bis zur bereiten Bruecke
        except Exception as e:
            meldungen = z.beenden()
            messen.diagnose_notiere("ablauf.aufbau", "%s: %s" % (type(e).__name__, e))
            text = str(e) if isinstance(e, RuntimeError) else None
            raise AufbauFehler(text if text and ziel.SEITENORDNER_FEST.match(text) else messen.fester_code(text, "ziel.aufbau"))
        # Pruefung des Gateways (Vertrag 8): eine Anfrage ueber den Weiterreicher, vor dem ersten Schritt. Sie laeuft vor dem
        # Anfangsstand jeder Zaehlung (der Laeufer nimmt je Zug den Zuwachs der Zaehldatei), also zaehlt sie in keinem Schritt mit.
        g = k.get("gateway", {})
        adresse = None
        if g.get("vorpruefung") and g.get("weiterreicher_url"):
            h, _, p_ = g["weiterreicher_url"].split("//", 1)[-1].rpartition(":")
            adresse = ("tcp", h, int(p_))
        elif g.get("pruef_socket"):
            adresse = ("unix", g["pruef_socket"])
        nur_cloud = k.get("umfang") == "nur_cloud"  # Messbetrieb: kein lokales Modell, kein Gateway, keine Pruefinstanz
        if nur_cloud:
            pass
        elif g.get("pruefinstanz"):
            code, hinweis = pruefe_gateway(k, fehler)
            if code:
                meldungen = z.beenden()
                raise AufbauFehler(code, hinweis)
        elif adresse:
            code, _ = messen.gateway_pruefen(adresse, k["start"]["lokal"]["modell"])
            if code:
                meldungen = z.beenden()
                raise AufbauFehler(code)
        if modus in ("playwright", "auto"):
            ok, grund = seite.verfuegbar(k)
            if ok:
                try:
                    seite_b = seite.SeiteBruecke(k, b, arbeit)
                    art_browser = "playwright"
                except Exception as e:
                    if modus == "playwright":
                        messen.diagnose_notiere("ablauf.browser", "%s: %s" % (type(e).__name__, e))
                        raise AufbauFehler("browser.start")
            elif modus == "playwright":
                raise AufbauFehler(grund)
        L = lauf.Lauf(k, seite_b or bruecke.Bruecke(b), z.mit, z.fremd, arbeit)
        L.gateway_schluessel = z.gateway_schluessel
        L.fehler = list(fehler)
        if z.cloud_marke:
            L.zinfo = {"marke": z.cloud_marke, "ziel_zaehler": lambda: z.modell_srv["cloud"].autorisiert}
        L.zinfo["bruecke_url"] = b["url"]
        L.einstellungen_schreiben("basis")
        for art_ in ("cloud", "lokal"):
            try:
                os.makedirs(k["start"][art_]["arbeitsordner"], exist_ok=True)
            except OSError:
                pass
        for kennung, fn in schritte.REIHENFOLGE:
            if nur and kennung not in nur and kennung != "S1":
                continue
            if nur_cloud and kennung in NUR_LOKAL:
                objekte[kennung] = (schritte.nur_cloud(kennung), 0)
                continue
            if kennung != "S1" and L.s1 is None:
                r = schritte.Schritt(kennung, NAMEN[kennung])
                r.uebersprungen = True
                objekte[kennung] = (r, 0)
                continue
            t0 = time.monotonic()
            marke = len(schritte.ANGELEGT)
            try:
                r = fn(L)
            except Exception as e:
                r = schritt_nach_ausnahme(kennung, marke)
                # `fehler` ist fest: `<Ausnahmetyp>: <code>`. Der Text der Ausnahme kann Inhalt der Sitzung tragen (Ende des Verlaufs,
                # Antwort der Bruecke): er geht nur in die Diagnosedatei.
                r.code_fest = ausnahme_code(kennung, e)
                r.fehler = "%s: %s" % (type(e).__name__, r.code_fest or "%s.ausnahme" % kennung.lower())
                messen.diagnose_notiere("schritt.%s" % kennung.lower(), "%s: %s" % (type(e).__name__, e))
                if isinstance(e, lauf.KeinEingabefeld) and e.sid in L.ende_befund:
                    r.messwerte.update(L.ende_befund[e.sid])
            objekte[kennung] = (r, int((time.monotonic() - t0) * 1000))
        # Suche nach dem genauen Schluessel (Vertrag 8, 9a): nach allen Schritten und nach dem Ende der Sitzungen, einmal je Route
        L.alle_beenden()
        meldungen += schritte.suche_nachtrag(L, {kn: o[0] for kn, o in objekte.items() if not (nur_cloud and kn in NUR_LOKAL)})
        ergebnisse = [dict(r.als_dict(), dauer_ms=d) for r, d in objekte.values()]
    finally:
        if seite_b is not None:
            meldungen += seite_b.schliessen()
        elif L is not None:
            L.b.zumachen()
        info = {"starts": L.starts if L else 0, "browser": art_browser, "arbeit": arbeit,
                "seite": getattr(z, "seite_beleg", None),
                "ca": dict(L.ca) if L else {"cli_nimmt_an": None, "ausgang": None},
                "cloud_eingaben": L.cloud_eingaben if L else 0,
                "gegenprobe": dict(L.gegenprobe) if L else {"weg": None, "neue_sitzungen": None, "neue_kinder": None}}
        if L is not None:
            L.alle_beenden()
            try:
                L.einstellungen_zurueck()
            except Exception as e:
                messen.diagnose_notiere("ablauf.einstellungen_zurueck", "%s: %s" % (type(e).__name__, e))
                meldungen.append("Einstellungsdatei nicht zurueckgesetzt")
        meldungen += z.beenden()
        if L is not None:  # nach dem Ende der Sitzungen und des Weiterreichers: danach liest und schreibt niemand mehr
            meldungen += zaehldateien_entfernen(k, L.sitzungen)
    return ergebnisse, meldungen, info
