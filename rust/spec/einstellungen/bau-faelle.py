#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags Einstellungsdatei (faelle/, erwartet/), vorfuehrung.json und erststart.json.

Aufruf: python3 -I rust/spec/einstellungen/bau-faelle.py
Jeder Fall trägt die Datei als Text (`text`) oder als Bytes in Base64 (`b64`).
"""
import base64
import copy
import json
import re
from pathlib import Path

HIER = Path(__file__).resolve().parent
ALLE = "/" + "*"  # Pfad, der jeden Pfad eines Hosts trifft

VORFUEHRUNG = {
    "fassung": 1,
    "klassen": ["oeffentlich", "intern", "kundendaten"],
    "vorgabestufe": "kundendaten",
    "routen": [
        {"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern"},
        {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten"},
    ],
    "kunden": [
        {"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []},
    ],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
}

# Vorführdatei (Fassung 2): eigener Datensatz, damit die abgeleiteten Fälle oben stabil bleiben.
# Die Netzziele der Route copilot sind gemessen (Messzug zum Token der Copilot CLI, Copilot CLI 1.0.88, ohne echtes Konto);
# Plan und exp-Ziel aus dem echten Lauf (1.0.88, Testkonto, 20.11.).
VORFUEHRDATEI = {
    "fassung": 2,
    "klassen": ["public", "internal", "confidential", "customer"],
    "vorgabestufe": "customer",
    "routen": [
        {"schluessel": "copilot", "art": "cloud", "klasse": "internal", "netzziele": [
            # Nur genaue Pfade an api.github.com, kein Teilbaum (Entscheid der Leitung, 25.11.2026, Sicherheitsprüfung
            # F1): `GET /copilot_internal/v2/token` gäbe ein abgeleitetes Token aus, das die Leck-Sperre nicht kennt.
            # alt: `GET /copilot_internal/*`.
            {"host": "api.github.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
             "pfade": [{"methode": "GET", "pfad": "/copilot_internal/user"},
                       {"methode": "GET", "pfad": "/copilot/mcp_registry"}]},
            {"host": "api.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
             "pfade": [{"methode": "GET", "pfad": "/agents/swe/internal/memory/v0/enabled"}]},
            # Wert von endpoints.api (gemessen im echten Lauf mit dem Testkonto). Nur genaue Pfade, kein Teilbaum
            # (Entscheid der Leitung, 25.11.2026, Sicherheitsprüfung N1): Ein Pfad, der ein Token ausgibt, steht nie in
            # einer Regel eines gebundenen Ziels (Halter-Vertrag, Abschnitt 4). alt: GET /* und POST /* dazu.
            {"host": "api.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
             "pfade": [# gemessen (Vertrag 8, Messung der Copilot CLI 1.0.88 am Wert von endpoints.api): Modellliste
                       {"methode": "GET", "pfad": "/models"},
                       # gemessen im zweiten angemeldeten Start (1.0.88): Modellaufruf als WebSocket, Upgrade 101
                       {"methode": "GET", "pfad": "/responses"},
                       # gemessen mit 1.0.88 (Messung zum Token der CLI): Modellaufrufe, per CONNECT am Proxy
                       {"methode": "POST", "pfad": "/auto"}, {"methode": "POST", "pfad": "/models/session"},
                       {"methode": "POST", "pfad": "/models/session/intent"},
                       {"methode": "POST", "pfad": "/chat/completions"},
                       {"methode": "POST", "pfad": "/agents/sessions"}]},
            # Wert von endpoints.telemetry; ohne Schlüssel, nur POST /telemetry. Im echten Lauf NXDOMAIN, die CLI lief
            # weiter; der Eintrag bleibt (Entscheidung).
            {"host": "telemetry.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
             "schluessel": False, "pfade": [{"methode": "POST", "pfad": "/telemetry"}]},
            # gemessen im echten Lauf: Zuteilung von Experimenten, ohne Authorization.
            {"host": "exp.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
             "schluessel": False, "pfade": [{"methode": "POST", "pfad": "/api/v1/assignments"}]},
            # Plan Business (Nachtrag Business, 26.11.2026): Die GitHub-Doku "Copilot allowlist reference" nennt für den Plan nur
            # die Unterdomänen-Form *.business.githubcopilot.com. Statt dieser Form stehen hier einzelne Hosts darunter, die
            # engere Form; der Schlüssel geht nie an eine Unterdomänen-Form (Stufe 31). Der Modell-Host trägt dieselben
            # genauen Pfade wie der Modell-Host des Plans individual (hergeleitet, mit Business nicht gemessen).
            {"host": "api.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
             "pfade": "MODELL_PFADE"},
            # Telemetrie des Plans Business (gemeldet aus dem Betrieb mit dem Plan, nicht selbst gemessen); ohne Schlüssel, nur POST /telemetry
            {"host": "telemetry.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
             "schluessel": False, "pfade": [{"methode": "POST", "pfad": "/telemetry"}]},
            # Experimente des Plans Business: hergeleitet aus exp.individual.githubcopilot.com, ohne Schlüssel
            {"host": "exp.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen",
             "schluessel": False, "pfade": [{"methode": "POST", "pfad": "/api/v1/assignments"}]},
        ]},
        {"schluessel": "lokal", "art": "lokal", "klasse": "customer",
         "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []},
    ],
    "kunden": [
        {"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []},
        {"schluessel": "k-sued", "erlaubt": [], "verboten": []},
    ],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
}
# Die Modell-Hosts der Pläne individual und Business: dieselben genauen Pfade, derselbe Schlüssel der Route.
MODELL_HOSTS = ("api.individual.githubcopilot.com", "api.business.githubcopilot.com")
_MODELL_PFADE = next(z["pfade"] for z in VORFUEHRDATEI["routen"][0]["netzziele"] if z["host"] == MODELL_HOSTS[0])
for _z in VORFUEHRDATEI["routen"][0]["netzziele"]:
    if _z["pfade"] == "MODELL_PFADE":
        _z["pfade"] = copy.deepcopy(_MODELL_PFADE)

# Die Vorführung setzt alle drei Werte (Entscheidung vom 20.11.2026). `vorfuehrung.json` trägt sie; der Fall
# `gueltig-vorfuehrung` bleibt mit dem alten Stand (ohne harness) unverändert, der neue Fall steht daneben.
VORFUEHRDATEI_MIT_WERKZEUGEN = {**VORFUEHRDATEI, "harness": {"copilot": {"werkzeuge_ohne_rueckfrage": ["shell", "write", "such-mcp"]}}}

# Erststart-Profil (Entscheid der Leitung, 25.11.2026): beim ersten echten Start am Modell-Host nur GET /models und
# POST /chat/completions, das Modell fest gewählt (kein auto). Die übrigen fünf gemessenen Pfade der Vorführdatei sind nicht
# freigeschaltet; ihre Anfragen enden beim Halter mit halter.pfad_verboten und stehen in der Kette. Alles andere wie in der
# Vorführdatei. Das Modell gpt-6-luna wählte die Copilot CLI 1.0.88 mit `--model auto --auto-tier efficiency` (gemessen am
# 20.11., Vertrag 1, rust/spec/start/README.md); es verbrauchte eine Chat-Einheit, kein Premium-Modell.
ERSTSTART_MODELL = "gpt-6-luna"
ERSTSTART_PFADE = [{"methode": "GET", "pfad": "/models"}, {"methode": "POST", "pfad": "/chat/completions"}]
ERSTSTARTDATEI = copy.deepcopy(VORFUEHRDATEI_MIT_WERKZEUGEN)
ERSTSTARTDATEI["routen"][0]["modell"] = ERSTSTART_MODELL
for _z in ERSTSTARTDATEI["routen"][0]["netzziele"]:
    if _z["host"] in MODELL_HOSTS:
        _z["pfade"] = copy.deepcopy(ERSTSTART_PFADE)
# Block effort des Erststart-Profils (Entscheid des Betreibers, 28.11.2026): für copilot an der Route des Profils (Klasse
# internal) mit gpt-6-luna genau none und low, ohne Vorgabe. Ein Start ohne effort bleibt ohne Option; mit none lässt die
# Copilot CLI 1.0.88 das Feld reasoning_effort weg (gemessen 27.11.2026, ohne Option sendet sie medium).
ERSTSTART_EFFORT = {"grenzen": [{"harness": "copilot", "klasse": "internal", "modell": ERSTSTART_MODELL,
                                 "erlaubt": ["none", "low"]}]}
ERSTSTARTDATEI["effort"] = copy.deepcopy(ERSTSTART_EFFORT)


def text(obj):
    return json.dumps(obj, ensure_ascii=False, indent=1) + "\n"


def mit(aendern):
    obj = copy.deepcopy(VORFUEHRUNG)
    aendern(obj)
    return obj


def f2(aendern=lambda o: None):
    """Fassung 2, abgeleitet aus VORFUEHRUNG: Netzziele an der Cloud-Route, Ziel an der lokalen Route."""
    def zu_f2(o):
        o["fassung"] = 2
        o["routen"][0]["netzziele"] = [{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden"}]
        o["routen"][1]["ziel"] = "http://127.0.0.1:11434/v1"
        o["routen"][1]["modell"] = "qwen3:8b"
        o["routen"][1]["netzziele"] = []
        aendern(o)
    return mit(zu_f2)


def ziel(wert):
    return f2(lambda o: o["routen"][1].update(ziel=wert))


def netzziel(host, port=443):
    return f2(lambda o: o["routen"][0].update(netzziele=[{"host": host, "port": port, "zugriff": "verbinden"}]))


def kopf(pfade=None, schluessel=True, host="api.cloud.example", dazu=()):
    """Cloud-Route mit einem Ziel der Zugriffsart kopf-einsetzen, optional weitere Ziele."""
    ziel = {"host": host, "port": 443, "zugriff": "kopf-einsetzen", "schluessel": schluessel,
            "pfade": pfade if pfade is not None else [{"methode": "GET", "pfad": "/copilot_internal/*"}]}
    return f2(lambda o: o["routen"][0].update(netzziele=[ziel, *dazu]))


FAELLE = []


def fall(name, beschreibung, inhalt, code=None):
    eintrag = {"beschreibung": beschreibung}
    if isinstance(inhalt, bytes):
        eintrag["b64"] = base64.b64encode(inhalt).decode("ascii")
    elif isinstance(inhalt, str):
        eintrag["text"] = inhalt
    else:
        eintrag["text"] = text(inhalt)
    erwartet = {"gueltig": True} if code is None else {"gueltig": False, "code": code}
    FAELLE.append((name, eintrag, erwartet))


# Gültige Fälle
fall("gueltig-vorfuehrung", "Vorführdatei (Fassung 2): Route copilot mit Netzzielen, Route lokal mit Ziel, Kunde k-nord",
     VORFUEHRDATEI)
fall("gueltig-fassung-1", "Basis der abgeleiteten Fälle: Fassung 1 ohne Ziel und Netzziele bleibt gültig", VORFUEHRUNG)
fall("gueltig-ohne-kunden", "keine Kunden: gültig, Kundendaten bekommen keine Route",
     mit(lambda o: o.update(kunden=[])))
fall("gueltig-kunde-ohne-erlaubt", "Kunde ohne erlaubt und verboten: gültig, der Kunde bekommt keine Route",
     mit(lambda o: o.update(kunden=[{"schluessel": "k-beispiel"}])))
fall("gueltig-freigabe-cloud", "eine Freigabe für Cloud-Route und Kunde: Form gültig (Schalter folgt später in der Konsole)",
     mit(lambda o: o["kundendaten_an_cloud"].update(freigaben=[{"route": "cloud-beispiel", "kunde": "k-beispiel"}])))
fall("gueltig-kennung-64", "Routenschlüssel mit 64 Zeichen ist die Grenze und gültig",
     mit(lambda o: (o["routen"][0].update(schluessel="c" * 64))))
fall("gueltig-reihenfolge-schluessel", "Schlüssel in anderer Reihenfolge: gültig",
     json.dumps(dict(reversed(list(VORFUEHRUNG.items()))), ensure_ascii=False))

basis = text(VORFUEHRUNG)
fall("gueltig-groesse-65536", "genau 65536 Byte (aufgefüllt mit Leerzeichen): gültig",
     basis + " " * (65536 - len(basis.encode())))

# einst.zu_gross
fall("zu-gross-65537", "65537 Byte: zu groß, auch wenn der Inhalt gültig ist",
     basis + " " * (65537 - len(basis.encode())), "einst.zu_gross")

# einst.json_ungueltig
fall("json-syntax", "abgeschnittenes JSON", basis[:40], "einst.json_ungueltig")
fall("json-bom", "UTF-8 mit BOM", b"\xef\xbb\xbf" + basis.encode(), "einst.json_ungueltig")
fall("json-kein-utf8", "Byte 0xFF ist kein UTF-8", basis.encode().replace(b"intern", b"intern\xff", 1),
     "einst.json_ungueltig")
fall("json-doppelter-schluessel", "Schlüssel fassung doppelt im Objekt",
     basis.replace('"fassung": 1,', '"fassung": 1,\n "fassung": 1,', 1), "einst.json_ungueltig")
fall("json-kein-objekt", "oberste Ebene ist eine Liste", "[]\n", "einst.json_ungueltig")
fall("json-nachlauf", "Text nach dem Objekt", basis + "{}\n", "einst.json_ungueltig")

# Verschachtelungstiefe (Nachtrag 20.11.): das Objekt der obersten Ebene ist Ebene 1; mehr als 127 Ebenen sind
# einst.json_ungueltig (Stackschutz, fail closed). Die Form kennt keinen tief verschachtelten Wert; die Tiefe steckt
# darum unter einem unbekannten Schlüssel x.
def tief(ebenen, innen="["):
    if innen == "{":
        kern = '{"a":' * (ebenen - 1) + "0" + "}" * (ebenen - 1)
    else:
        kern = "[" * (ebenen - 1) + "]" * (ebenen - 1)
    return basis.replace('"fassung": 1,', '"fassung": 1,\n "x": ' + kern + ",", 1)


fall("json-tiefe-127", "Tiefe 127 (Objekt plus 126 Listen) ist die Grenze: JSON gelesen, erst die Form lehnt x ab",
     tief(127), "einst.schluessel_unbekannt")
fall("json-tiefe-128", "Tiefe 128 (Objekt plus 127 Listen)", tief(128), "einst.json_ungueltig")
fall("json-tiefe-objekte-127", "Tiefe 127 aus Objekten: JSON gelesen, erst die Form lehnt x ab", tief(127, "{"),
     "einst.schluessel_unbekannt")
fall("json-tiefe-objekte-128", "Tiefe 128 aus Objekten", tief(128, "{"), "einst.json_ungueltig")
fall("json-tiefe-20000", "20000 Ebenen: das Lesen darf nicht an der Stapeltiefe scheitern", tief(20000),
     "einst.json_ungueltig")
fall("json-tiefe-vor-schluessel", "Tiefe 128 geht vor einem unbekannten Schlüssel (Stufe 2 vor 3)",
     tief(128).replace('"x":', '"y": 1,\n "x":', 1), "einst.json_ungueltig")

# einst.schluessel_unbekannt
fall("schluessel-unbekannt-oben", "unbekannter Schlüssel oben (geschlossene Form)",
     mit(lambda o: o.update(konsole=True)), "einst.schluessel_unbekannt")
fall("schluessel-unbekannt-route", "unbekannter Schlüssel in einer Route",
     mit(lambda o: o["routen"][0].update(zone="z1")), "einst.schluessel_unbekannt")
fall("schluessel-unbekannt-vor-ohne-klasse", "unbekannter Schlüssel geht vor Route ohne Klasse",
     mit(lambda o: (o.update(regeln=[]), o["routen"][0].pop("klasse"))), "einst.schluessel_unbekannt")

# einst.route_ohne_klasse
fall("route-ohne-klasse", "Route ohne Feld klasse: ungültig, das Tor sperrt sie",
     mit(lambda o: o["routen"][1].pop("klasse")), "einst.route_ohne_klasse")
fall("route-klasse-null", "Route mit klasse null zählt als Route ohne Klasse",
     mit(lambda o: o["routen"][1].update(klasse=None)), "einst.route_ohne_klasse")
fall("route-ohne-klasse-vor-fehlt", "Route ohne Klasse geht vor einem anderen fehlenden Schlüssel",
     mit(lambda o: (o["routen"][1].pop("klasse"), o.pop("kunden"))), "einst.route_ohne_klasse")

# einst.schluessel_fehlt
fall("schluessel-fehlt-kunden", "Feld kunden fehlt", mit(lambda o: o.pop("kunden")), "einst.schluessel_fehlt")
fall("gueltig-ohne-vorgabestufe", "ohne vorgabestufe: gültig, es gilt die höchste Klasse",
     mit(lambda o: o.pop("vorgabestufe")))
fall("schluessel-fehlt-freigaben", "kundendaten_an_cloud ohne freigaben",
     mit(lambda o: o["kundendaten_an_cloud"].pop("freigaben")), "einst.schluessel_fehlt")


# Jedes required des Schemas hat einen Fall "fehlt" (Nachtrag 21.11.): je Pfad Fassung 1 und Fassung 2, soweit der Pfad
# in beiden Fassungen gilt. Der Pfad "klasse" einer Route steht bei route_ohne_klasse (Stufe 4).
def freigabe_dazu(o):
    o["kundendaten_an_cloud"]["freigaben"] = [{"route": "cloud-beispiel", "kunde": "k-beispiel"}]


def pflicht_fehlt(kurz, beschreibung, aendern, nur_f2=False):
    """Zwei Fälle (Fassung 1 und 2), in denen der Pfad fehlt; das Ergebnis ist je einst.schluessel_fehlt."""
    if not nur_f2:
        fall(f"schluessel-fehlt-{kurz}-fassung-1", f"Fassung 1: {beschreibung}", mit(aendern),
             "einst.schluessel_fehlt")
    fall(f"schluessel-fehlt-{kurz}-fassung-2", f"Fassung 2: {beschreibung}", f2(aendern), "einst.schluessel_fehlt")


# Ohne fassung gilt die Form der Fassung 1; ein Fall der Fassung 2 wäre schon an netzziele ein unbekannter Schlüssel.
fall("schluessel-fehlt-fassungsfeld-fassung-1", "Fassung 1: Feld fassung fehlt", mit(lambda o: o.pop("fassung")),
     "einst.schluessel_fehlt")
pflicht_fehlt("klassen", "Feld klassen fehlt", lambda o: o.pop("klassen"))
pflicht_fehlt("routen", "Feld routen fehlt", lambda o: o.pop("routen"))
pflicht_fehlt("kunden", "Feld kunden fehlt", lambda o: o.pop("kunden"), nur_f2=True)  # Fassung 1: schluessel-fehlt-kunden
pflicht_fehlt("kundendaten", "Objekt kundendaten_an_cloud fehlt", lambda o: o.pop("kundendaten_an_cloud"))
pflicht_fehlt("vorgabe", "kundendaten_an_cloud ohne vorgabe", lambda o: o["kundendaten_an_cloud"].pop("vorgabe"))
pflicht_fehlt("freigaben", "kundendaten_an_cloud ohne freigaben", lambda o: o["kundendaten_an_cloud"].pop("freigaben"),
              nur_f2=True)  # Fassung 1: schluessel-fehlt-freigaben
pflicht_fehlt("freigabe-route", "Freigabe ohne route",
              lambda o: (freigabe_dazu(o), o["kundendaten_an_cloud"]["freigaben"][0].pop("route")))
pflicht_fehlt("freigabe-kunde", "Freigabe ohne kunde",
              lambda o: (freigabe_dazu(o), o["kundendaten_an_cloud"]["freigaben"][0].pop("kunde")))
pflicht_fehlt("route-schluessel", "Route ohne schluessel", lambda o: o["routen"][0].pop("schluessel"))
pflicht_fehlt("route-art", "Route ohne art", lambda o: o["routen"][0].pop("art"))
pflicht_fehlt("kunde-schluessel", "Kunde ohne schluessel", lambda o: o["kunden"][0].pop("schluessel"))
pflicht_fehlt("netzziel-host", "Netzziel ohne host",
              lambda o: o["routen"][0].update(netzziele=[{"port": 443, "zugriff": "verbinden"}]), nur_f2=True)
pflicht_fehlt("pfad-methode", "Pfadeintrag ohne methode",
              lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443,
                                                           "zugriff": "kopf-einsetzen", "schluessel": True,
                                                           "pfade": [{"pfad": "/a"}]}]), nur_f2=True)
pflicht_fehlt("pfad-pfad", "Pfadeintrag ohne pfad",
              lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443,
                                                           "zugriff": "kopf-einsetzen", "schluessel": True,
                                                           "pfade": [{"methode": "GET"}]}]), nur_f2=True)

# einst.typ_falsch
fall("typ-fassung-text", "fassung als Text", mit(lambda o: o.update(fassung="1")), "einst.typ_falsch")
fall("typ-fassung-bruch", "fassung als 1.0 ist keine Ganzzahl", basis.replace('"fassung": 1', '"fassung": 1.0', 1),
     "einst.typ_falsch")
fall("typ-vorgabe-bool", "kundendaten_an_cloud.vorgabe als true",
     mit(lambda o: o["kundendaten_an_cloud"].update(vorgabe=True)), "einst.typ_falsch")
fall("typ-vorgabe-einfaches-feld", "kundendaten_an_cloud als Text statt Objekt",
     mit(lambda o: o.update(kundendaten_an_cloud="nie")), "einst.typ_falsch")

# einst.fassung_unbekannt
fall("fassung-3", "fassung 3 ist unbekannt", mit(lambda o: o.update(fassung=3)), "einst.fassung_unbekannt")

# einst.anzahl
fall("anzahl-routen-leer", "keine Route", mit(lambda o: o.update(routen=[])), "einst.anzahl")
fall("anzahl-klassen-leer", "keine Klasse", mit(lambda o: o.update(klassen=[])), "einst.anzahl")

# einst.kennung_form
fall("kennung-form-gross", "Großbuchstabe im Routenschlüssel",
     mit(lambda o: o["routen"][0].update(schluessel="Cloud-beispiel")), "einst.kennung_form")
fall("kennung-laenge-65", "Routenschlüssel mit 65 Zeichen",
     mit(lambda o: o["routen"][0].update(schluessel="c" * 65)), "einst.kennung_form")
fall("kennung-form-klasse", "Klassenschlüssel beginnt mit Punkt",
     mit(lambda o: o["klassen"].append(".geheim")), "einst.kennung_form")
fall("kennung-form-kunde-leer", "leerer Kundenschlüssel",
     mit(lambda o: o["kunden"][0].update(schluessel="")), "einst.kennung_form")

# einst.kennung_doppelt
fall("doppelt-routenschluessel", "zwei Routen mit demselben Schlüssel",
     mit(lambda o: o["routen"][1].update(schluessel="cloud-beispiel")), "einst.kennung_doppelt")
fall("doppelt-klasse", "Klasse intern zweimal", mit(lambda o: o["klassen"].append("intern")), "einst.kennung_doppelt")
fall("doppelt-kunde", "Kunde zweimal",
     mit(lambda o: o["kunden"].append({"schluessel": "k-beispiel"})), "einst.kennung_doppelt")

# einst.art_unbekannt
fall("art-unbekannt", "art hybrid ist weder cloud noch lokal",
     mit(lambda o: o["routen"][0].update(art="hybrid")), "einst.art_unbekannt")

# einst.klasse_unbekannt
fall("klasse-unbekannt-route", "Routenklasse steht nicht in klassen",
     mit(lambda o: o["routen"][0].update(klasse="geheim")), "einst.klasse_unbekannt")

# einst.vorgabestufe_unbekannt
fall("vorgabestufe-unbekannt", "vorgabestufe steht nicht in klassen",
     mit(lambda o: o.update(vorgabestufe="customer")), "einst.vorgabestufe_unbekannt")
fall("vorgabestufe-null", "vorgabestufe null ist der falsche Typ (weglassen heißt höchste Klasse)",
     mit(lambda o: o.update(vorgabestufe=None)), "einst.typ_falsch")

# einst.route_unbekannt
fall("route-unbekannt-erlaubt", "erlaubt nennt eine Route, die es nicht gibt",
     mit(lambda o: o["kunden"][0].update(erlaubt=["lokal-anders"])), "einst.route_unbekannt")
fall("route-unbekannt-freigabe", "Freigabe nennt eine Route, die es nicht gibt",
     mit(lambda o: o["kundendaten_an_cloud"].update(freigaben=[{"route": "cloud-anders", "kunde": "k-beispiel"}])),
     "einst.route_unbekannt")

# einst.kunde_unbekannt
fall("kunde-unbekannt-freigabe", "Freigabe nennt einen Kunden, den es nicht gibt",
     mit(lambda o: o["kundendaten_an_cloud"].update(freigaben=[{"route": "cloud-beispiel", "kunde": "k-anders"}])),
     "einst.kunde_unbekannt")

# einst.vorgabe_wert
fall("vorgabe-wert-erlaubt", "Vorgabe mit anderem Wert als nie",
     mit(lambda o: o["kundendaten_an_cloud"].update(vorgabe="erlaubt")), "einst.vorgabe_wert")

# einst.freigabe_route_lokal
fall("freigabe-route-lokal", "Freigabe für eine lokale Route ist sinnlos und ungültig",
     mit(lambda o: o["kundendaten_an_cloud"].update(freigaben=[{"route": "lokal-beispiel", "kunde": "k-beispiel"}])),
     "einst.freigabe_route_lokal")


# Fassung 2: Ziel, Modell und Netzziele (Nachtrag 20.11.)
fall("gueltig-fassung-2", "Fassung 2: Cloud-Route mit einem Netzziel, lokale Route mit Ziel auf dem Loopback", f2())
fall("gueltig-netzziel-unterdomaene", "Netzziel in Unterdomänen-Form *.<name> mit zwei Teilen dahinter",
     netzziel("*.cloud.example"))
fall("gueltig-netzziel-port-65535", "Port 65535 ist die Grenze", netzziel("api.cloud.example", 65535))
fall("gueltig-cloud-ohne-netzziele", "Cloud-Route mit leerer Liste: gültig, der Einschluss lässt nichts hinaus",
     f2(lambda o: o["routen"][0].update(netzziele=[])))
fall("gueltig-ziel-https-name", "https zu einem Namen im Firmennetz, Pfad mit /v1 am Ende",
     ziel("https://modell.beispiel.example:8443/api/v1"))
fall("gueltig-ziel-privat-https", "https zu einer privaten IPv4-Adresse", ziel("https://10.0.0.5:8443/v1"))
fall("ziel-host-http-privat", "http zu einer privaten IPv4-Adresse: Klartext nur zu 127.0.0.1 (wie Vertrag 8)",
     ziel("http://10.0.0.5:8000/v1"), "einst.ziel_host")
fall("ziel-host-http-localhost", "http zu localhost: Klartext nur zu 127.0.0.1", ziel("http://localhost:11434/v1"),
     "einst.ziel_host")
fall("ziel-host-http-loopback-anders", "http zu 127.0.0.2: Klartext nur zu 127.0.0.1", ziel("http://127.0.0.2:11434/v1"),
     "einst.ziel_host")
fall("ziel-form-ohne-v1", "Ziel ohne /v1 am Ende", ziel("http://127.0.0.1:11434"), "einst.ziel_form")
fall("ziel-form-anderer-pfad", "Pfad endet nicht auf /v1", ziel("http://127.0.0.1:11434/v2"), "einst.ziel_form")

fall("gueltig-modell-gateway-name", "Modellname eines Gateways mit /, : und .",
     f2(lambda o: o["routen"][1].update(modell="beispiel-ollama/qwen3.8:27b")))
fall("modell-fehlt-lokal", "Fassung 2: lokale Route ohne modell", f2(lambda o: o["routen"][1].pop("modell")),
     "einst.ziel_art")
fall("gueltig-cloud-mit-modell", "Cloud-Route mit modell: erlaubt, die Route legt dann das Modell fest",
     f2(lambda o: o["routen"][0].update(modell="claude-sonnet-5.5")))
fall("modell-form-leerzeichen", "Modell mit Leerzeichen und Option",
     f2(lambda o: o["routen"][1].update(modell="qwen3:8b --frei")), "einst.modell_form")
fall("modell-form-129", "Modell mit 129 Zeichen", f2(lambda o: o["routen"][1].update(modell="m" * 129)),
     "einst.modell_form")
fall("typ-modell-zahl", "Modell als Zahl", f2(lambda o: o["routen"][1].update(modell=8)), "einst.typ_falsch")
fall("ziel-host-vor-modell", "Hostregel geht vor der Form des Modells",
     f2(lambda o: o["routen"][1].update(ziel="http://0.0.0.0:11434/v1", modell="")), "einst.ziel_host")
fall("fassung-2-ohne-netzziele", "Fassung 2: Route ohne netzziele", f2(lambda o: o["routen"][0].pop("netzziele")),
     "einst.schluessel_fehlt")
fall("fassung-1-mit-netzziele", "Fassung 1 kennt netzziele nicht",
     mit(lambda o: o["routen"][0].update(netzziele=[])), "einst.schluessel_unbekannt")
fall("fassung-1-mit-modell", "Fassung 1 kennt modell an der Route nicht",
     mit(lambda o: o["routen"][1].update(modell="qwen3:8b")), "einst.schluessel_unbekannt")
fall("fassung-1-mit-ziel", "Fassung 1 kennt ziel nicht",
     mit(lambda o: o["routen"][1].update(ziel="http://127.0.0.1:11434/v1")), "einst.schluessel_unbekannt")
fall("netzziel-schluessel-unbekannt", "Netzziel mit Schlüssel schema",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden", "schema": "https"}])),
     "einst.schluessel_unbekannt")
fall("netzziel-ohne-port", "Netzziel ohne port",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "zugriff": "verbinden"}])),
     "einst.schluessel_fehlt")
fall("netzziel-ohne-zugriff", "Netzziel ohne zugriff: Pflicht nach A141 (fehlende Angabe ist ein Ladefehler)",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443}])), "einst.schluessel_fehlt")
fall("typ-netzziel-port-text", "Port als Text", netzziel("api.cloud.example", "443"), "einst.typ_falsch")
fall("typ-ziel-objekt", "Ziel als Objekt statt Text",
     f2(lambda o: o["routen"][1].update(ziel={"host": "127.0.0.1", "port": 11434})), "einst.typ_falsch")
fall("anzahl-netzziele-65", "65 Netzziele an einer Route",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": p, "zugriff": "verbinden"} for p in range(1, 66)])),
     "einst.anzahl")
fall("doppelt-netzziel", "dasselbe Paar host und port zweimal",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden"}] * 2)),
     "einst.kennung_doppelt")

fall("ziel-fehlt-lokal", "Fassung 2: lokale Route ohne ziel", f2(lambda o: o["routen"][1].pop("ziel")),
     "einst.ziel_art")
fall("ziel-bei-cloud", "Cloud-Route mit ziel",
     f2(lambda o: o["routen"][0].update(ziel="https://api.cloud.example:443")), "einst.ziel_art")
fall("ziel-art-vor-schema", "Ziel an einer Cloud-Route geht vor dem falschen Schema",
     f2(lambda o: o["routen"][0].update(ziel="ftp://api.cloud.example:21")), "einst.ziel_art")
fall("klasse-unbekannt-vor-ziel", "unbekannte Klasse geht vor jedem Fehler des Ziels",
     f2(lambda o: o["routen"][1].update(klasse="geheim", ziel="ftp://x")), "einst.klasse_unbekannt")
fall("ziel-schema-ftp", "Schema ftp", ziel("ftp://127.0.0.1:21/v1"), "einst.ziel_schema")
fall("ziel-schema-gross", "Schema in Großbuchstaben", ziel("HTTP://127.0.0.1:11434/v1"), "einst.ziel_schema")
fall("ziel-ohne-schema", "Host und Port ohne Schema", ziel("127.0.0.1:11434"), "einst.ziel_schema")
fall("ziel-form-ohne-port", "ohne Port", ziel("http://127.0.0.1/v1"), "einst.ziel_form")
fall("ziel-form-nutzer", "mit Nutzerteil vor dem Host", ziel("http://nutzer@127.0.0.1:11434/v1"),
     "einst.ziel_form")
fall("ziel-form-anfrage", "mit Anfrageteil", ziel("http://127.0.0.1:11434/v1?schluessel=x"), "einst.ziel_form")
fall("ziel-form-schraegstrich-ende", "Pfad endet mit /", ziel("http://127.0.0.1:11434/v1/"), "einst.ziel_form")
fall("ziel-form-punktpunkt", "Pfad mit ..", ziel("http://127.0.0.1:11434/x/../v1"), "einst.ziel_form")
fall("ziel-form-ipv4-256", "IPv4 mit Teil 256", ziel("http://127.0.0.256:11434/v1"), "einst.ziel_form")
fall("ziel-form-ipv6", "IPv6 gehört nicht zu Fassung 2", ziel("http://[::1]:11434/v1"), "einst.ziel_form")
fall("ziel-form-port-null-vorn", "Port mit führender Null", ziel("http://127.0.0.1:011434/v1"),
     "einst.ziel_form")
fall("ziel-form-257", "Ziel mit 257 Byte", ziel("http://127.0.0.1:11434/" + "v" * 231 + "/v1"), "einst.ziel_form")
fall("ziel-port-0", "Port 0", ziel("http://127.0.0.1:0/v1"), "einst.ziel_port")
fall("ziel-port-65536", "Port 65536", ziel("http://127.0.0.1:65536/v1"), "einst.ziel_port")
fall("ziel-host-http-name", "http zu einem Namen: Klartext nur zu 127.0.0.1",
     ziel("http://modell.beispiel.example:8000/v1"), "einst.ziel_host")
fall("ziel-host-oeffentlich", "öffentliche IPv4-Adresse, auch mit https",
     ziel("https://203.0.113.7:443/v1"), "einst.ziel_host")
fall("ziel-host-unbestimmt", "0.0.0.0 ist kein Loopback", ziel("http://0.0.0.0:11434/v1"), "einst.ziel_host")

fall("netzziel-lokal", "lokale Route mit einem Netzziel",
     f2(lambda o: o["routen"][1].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden"}])),
     "einst.netzziel_lokal")
fall("netzziel-lokal-vor-form", "Netzziel an lokaler Route geht vor der Form des Netzziels",
     f2(lambda o: o["routen"][1].update(netzziele=[{"host": "localhost", "port": 0, "zugriff": "verbinden"}])), "einst.netzziel_lokal")
for name, host, was in [
    ("netzziel-form-ip", "203.0.113.7", "IPv4-Adresse statt Name"),
    ("netzziel-form-localhost", "localhost", "localhost"),
    ("netzziel-form-unter-localhost", "modell.localhost", "Name unter localhost"),
    ("netzziel-form-ein-teil", "intranet", "Name aus einem Teil"),
    ("netzziel-form-stern-tld", "*.example", "Unterdomänen-Form mit nur einem Teil dahinter"),
    ("netzziel-form-stern-mitte", "api.*.cloud.example", "Stern nicht am Anfang"),
    ("netzziel-form-stern-allein", "*", "Stern allein"),
    ("netzziel-form-schema", "https://api.cloud.example", "Schema im Host"),
    ("netzziel-form-gross", "API.cloud.example", "Großbuchstaben"),
]:
    fall(name, was, netzziel(host), "einst.netzziel_form")
fall("gueltig-netzziel-http-lesen", "Netzziel mit Zugriffsart http-lesen (A141)",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 80, "zugriff": "http-lesen"}])))
fall("doppelt-netzziel-andere-zugriffsart", "dasselbe Paar host und port mit zwei Zugriffsarten ist doppelt",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden"},
                                                     {"host": "api.cloud.example", "port": 443, "zugriff": "http-lesen"}])),
     "einst.kennung_doppelt")
fall("typ-zugriff-bool", "Zugriffsart als true",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": True}])),
     "einst.typ_falsch")
fall("zugriff-unbekannt", "Zugriffsart schreiben gibt es nicht",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "schreiben"}])),
     "einst.zugriff_unbekannt")
# Zugriffsart kopf-einsetzen (Nachtrag 6): TLS-Abschluss am Modellweg, Methode und Pfad je Ziel, Schlüssel gebunden oder nicht
fall("gueltig-kopf-einsetzen", "Ziel mit Schlüssel und Pfadliste", kopf())
fall("gueltig-kopf-ohne-schluessel-neben-verbinden", "Route ohne gebundenen Schlüssel darf daneben verbinden haben",
     kopf([{"methode": "POST", "pfad": "/telemetry"}], schluessel=False,
          dazu=[{"host": "dl.cloud.example", "port": 443, "zugriff": "verbinden"}]))
fall("gueltig-pfad-alles", "Pfad ALLE trifft jeden Pfad des Hosts", kopf([{"methode": "POST", "pfad": ALLE}]))
fall("gueltig-kopf-unterdomaene-ohne-schluessel", "Unterdomänen-Form ohne Schlüssel",
     kopf([{"methode": "POST", "pfad": "/telemetry"}], schluessel=False, host="*.cloud.example"))
fall("zugriff-kopf-ohne-pfade", "kopf-einsetzen ohne pfade",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen",
                                                     "schluessel": True}])), "einst.zugriff_felder")
fall("zugriff-kopf-ohne-schluessel", "kopf-einsetzen ohne Angabe schluessel",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen",
                                                     "pfade": [{"methode": "GET", "pfad": ALLE}]}])),
     "einst.zugriff_felder")
fall("zugriff-verbinden-mit-pfaden", "verbinden mit pfade: ein Tunnel kann keine Pfade durchsetzen",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "verbinden",
                                                     "pfade": [{"methode": "GET", "pfad": ALLE}]}])),
     "einst.zugriff_felder")
fall("zugriff-http-lesen-mit-schluessel", "http-lesen mit schluessel",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 80, "zugriff": "http-lesen",
                                                     "schluessel": False}])), "einst.zugriff_felder")
fall("typ-schluessel-text", "schluessel als Text", kopf(schluessel="ja"), "einst.typ_falsch")
fall("anzahl-pfade-leer", "kopf-einsetzen mit leerer Pfadliste", kopf([]), "einst.anzahl")
fall("anzahl-pfade-33", "33 Pfade an einem Ziel",
     kopf([{"methode": "GET", "pfad": f"/p{i}"} for i in range(33)]), "einst.anzahl")
fall("doppelt-pfad", "dasselbe Paar methode und pfad zweimal",
     kopf([{"methode": "GET", "pfad": "/a"}, {"methode": "GET", "pfad": "/a"}]), "einst.kennung_doppelt")
fall("pfad-schluessel-unbekannt", "Pfadeintrag mit Schlüssel kopf",
     kopf([{"methode": "GET", "pfad": "/a", "kopf": "Authorization"}]), "einst.schluessel_unbekannt")
for name, eintrag, was in [
    ("pfad-methode-klein", {"methode": "get", "pfad": "/a"}, "Methode klein geschrieben"),
    ("pfad-methode-connect", {"methode": "CONNECT", "pfad": "/a"}, "Methode CONNECT gibt es nicht"),
    ("pfad-relativ", {"methode": "GET", "pfad": "copilot_internal/user"}, "Pfad ohne / am Anfang"),
    ("pfad-wurzel", {"methode": "GET", "pfad": "/"}, "Pfad / allein (alle Pfade schreibt man mit Stern)"),
    ("pfad-stern-mitte", {"methode": "GET", "pfad": "/a/" + "*" + "/b"}, "Stern nicht am Ende"),
    ("pfad-stern-ohne-strich", {"methode": "GET", "pfad": "/a" + "*"}, "Stern ohne / davor"),
    ("pfad-punktpunkt", {"methode": "GET", "pfad": "/a/../b"}, "Pfad mit .."),
    ("pfad-anfrage", {"methode": "GET", "pfad": "/a?b=1"}, "Pfad mit Anfrageteil"),
    ("pfad-prozent", {"methode": "GET", "pfad": "/a%2Fb"}, "Pfad mit Prozentzeichen"),
    ("pfad-strich-ende", {"methode": "GET", "pfad": "/a/"}, "Pfad mit / am Ende"),
]:
    fall(name, was, kopf([eintrag]), "einst.pfad_form")
fall("verbinden-mit-schluessel", "Route mit gebundenem Schlüssel hat zusätzlich ein Ziel mit verbinden",
     kopf(dazu=[{"host": "dl.cloud.example", "port": 443, "zugriff": "verbinden"}]), "einst.verbinden_mit_schluessel")
fall("gueltig-schluessel-neben-http-lesen", "Route mit gebundenem Schlüssel darf http-lesen haben",
     kopf(dazu=[{"host": "dl.cloud.example", "port": 80, "zugriff": "http-lesen"}]))
fall("schluessel-unterdomaene", "Schlüssel an ein Ziel in Unterdomänen-Form gebunden",
     kopf(host="*.cloud.example"), "einst.schluessel_unterdomaene")
fall("zugriff-felder-vor-pfad-form", "fehlende Angabe schluessel geht vor der Form eines Pfads",
     f2(lambda o: o["routen"][0].update(netzziele=[{"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen",
                                                     "pfade": [{"methode": "get", "pfad": "x"}]}])),
     "einst.zugriff_felder")
fall("pfad-form-vor-verbinden", "Form eines Pfads geht vor verbinden_mit_schluessel",
     kopf([{"methode": "GET", "pfad": "x"}], dazu=[{"host": "dl.cloud.example", "port": 443, "zugriff": "verbinden"}]),
     "einst.pfad_form")

fall("netzziel-port-0", "Port 0", netzziel("api.cloud.example", 0), "einst.netzziel_port")
fall("netzziel-port-65536", "Port 65536", netzziel("api.cloud.example", 65536), "einst.netzziel_port")

# Block mcp (Nachtrag 20.11., Fassung 2; Form wortgleich nach Vertrag 6, Abschnitt 2): optional, je Server die
# Zuordnung der Werkzeuge. Fehlt der Block, ist kein MCP-Server freigegeben (fail closed).
MCP = {
    "tickets": {"lesen": ["get"], "schreiben": ["update"], "freigabe": True, "frist_ms": 60000, "einloesefrist_ms": 30000},
    "notizen": {"lesen": [], "schreiben": ["anlegen"], "freigabe": False},
    "such-mcp": {"lesen": ["brain_suche"], "schreiben": []},
}


def mcp(block, fassung=2):
    """Fassung 2 mit Block mcp; fassung=1 hängt den Block an die Basis der Fassung 1."""
    if fassung == 1:
        return mit(lambda o: o.update(mcp=block))
    return f2(lambda o: o.update(mcp=block))


def server(**felder):
    return {"tickets": {"lesen": [], "schreiben": [], **felder}}


fall("gueltig-mcp", "Fassung 2 mit Block mcp: drei Server, Vorgaben für freigabe und Fristen", mcp(MCP))
fall("gueltig-ohne-mcp", "Fassung 2 ohne Block mcp: gültig, kein Server freigegeben", f2())
fall("gueltig-mcp-leer", "Block mcp ohne Server: gültig, kein Server freigegeben", mcp({}))
fall("gueltig-mcp-fristen-grenzen", "Fristen 1 und 86400000 sind die Grenzen und gültig",
     mcp(server(frist_ms=1, einloesefrist_ms=86400000)))
fall("gueltig-mcp-werkzeug-128", "Werkzeugname mit 128 Zeichen ist die Grenze und gültig",
     mcp(server(lesen=["a" * 128])))
fall("gueltig-mcp-server-64", "64 Server sind die Grenze und gültig",
     mcp({f"s{i}": {"lesen": [], "schreiben": []} for i in range(64)}))
fall("mcp-in-fassung-1", "Block mcp in einer Datei der Fassung 1", mcp(MCP, fassung=1), "einst.schluessel_unbekannt")
fall("mcp-schluessel-unbekannt-server", "unbekannter Schlüssel im Block eines Servers",
     mcp(server(modus="lesen")), "einst.schluessel_unbekannt")
fall("mcp-schluessel-unbekannt-oben", "zweiter Block neben mcp: nur die Form der Fassung 2 gilt",
     f2(lambda o: o.update(mcp={}, mcp2={})), "einst.schluessel_unbekannt")
fall("mcp-lesen-fehlt", "lesen fehlt", mcp({"tickets": {"schreiben": []}}), "einst.schluessel_fehlt")
fall("mcp-schreiben-fehlt", "schreiben fehlt", mcp({"tickets": {"lesen": []}}), "einst.schluessel_fehlt")
fall("mcp-frist-text", "frist_ms als Text", mcp(server(frist_ms="60000")), "einst.typ_falsch")
fall("mcp-frist-null", "einloesefrist_ms null", mcp(server(einloesefrist_ms=None)), "einst.typ_falsch")
fall("mcp-freigabe-text", "freigabe als Text", mcp(server(freigabe="true")), "einst.typ_falsch")
fall("mcp-block-liste", "mcp als Liste statt Objekt", mcp([]), "einst.typ_falsch")
fall("mcp-anzahl-server-65", "65 Server", mcp({f"s{i}": {"lesen": [], "schreiben": []} for i in range(65)}),
     "einst.anzahl")
fall("mcp-anzahl-werkzeuge-257", "257 Werkzeuge in lesen", mcp(server(lesen=[f"w{i}" for i in range(257)])),
     "einst.anzahl")
fall("mcp-server-kennung-form", "Servername mit Großbuchstaben", mcp({"Tickets": {"lesen": [], "schreiben": []}}),
     "einst.kennung_form")
fall("mcp-werkzeug-form-129", "Werkzeugname mit 129 Zeichen", mcp(server(lesen=["a" * 129])), "einst.kennung_form")
fall("mcp-werkzeug-form-leerzeichen", "Werkzeugname mit Leerzeichen", mcp(server(lesen=["a b"])), "einst.kennung_form")
fall("mcp-werkzeug-doppelt-in-liste", "derselbe Name zweimal in lesen", mcp(server(lesen=["get", "get"])),
     "einst.kennung_doppelt")
fall("mcp-frist-0", "frist_ms 0", mcp(server(frist_ms=0)), "einst.frist_bereich")
fall("mcp-frist-86400001", "frist_ms 86400001", mcp(server(frist_ms=86400001)), "einst.frist_bereich")
fall("mcp-einloesefrist-0", "einloesefrist_ms 0", mcp(server(einloesefrist_ms=0)), "einst.frist_bereich")
fall("mcp-einloesefrist-86400001", "einloesefrist_ms 86400001", mcp(server(einloesefrist_ms=86400001)),
     "einst.frist_bereich")
fall("mcp-frist-negativ", "frist_ms -1", mcp(server(frist_ms=-1)), "einst.frist_bereich")
fall("mcp-doppelt", "derselbe Name in lesen und schreiben", mcp(server(lesen=["x"], schreiben=["x"])),
     "einst.mcp_doppelt")
fall("mcp-doppelt-in-zweitem-server", "Doppel in einem späteren Server",
     mcp({"a": {"lesen": [], "schreiben": []}, "b": {"lesen": ["x"], "schreiben": ["y", "x"]}}), "einst.mcp_doppelt")
fall("mcp-frist-vor-doppelt", "frist_bereich geht vor mcp_doppelt (Stufe 32 vor 33)",
     mcp(server(lesen=["x"], schreiben=["x"], frist_ms=0)), "einst.frist_bereich")
fall("mcp-typ-vor-frist", "falscher Typ geht vor Frist außerhalb des Bereichs",
     mcp({"a": {"lesen": [], "schreiben": [], "frist_ms": 0}, "b": {"lesen": [], "schreiben": [], "freigabe": "x"}}),
     "einst.typ_falsch")
fall("mcp-netzziel-vor-frist", "Fehler im Netzziel gehen vor Fristen des Blocks mcp",
     f2(lambda o: (o.update(mcp=server(frist_ms=0)), o["routen"][0].update(netzziele=[{"host": "localhost", "port": 443, "zugriff": "verbinden"}]))),
     "einst.netzziel_form")
fall("mcp-doppelter-server", "derselbe Server zweimal im Objekt mcp (doppelter JSON-Schlüssel)",
     text(mcp({"tickets": {"lesen": [], "schreiben": []}})).replace(
         '"tickets": {', '"tickets": {"lesen": [], "schreiben": []},\n  "tickets": {', 1), "einst.json_ungueltig")


# Block harness (Entscheidung vom 20.11.2026, Fassung 2): optional, je Harness. Für copilot nennt
# werkzeuge_ohne_rueckfrage, welche Werkzeugarten die Rückfrage der CLI in Läufen im Einschluss nicht stellen.
# Erlaubt sind genau shell, write und such-mcp; jeder andere Wert ist ein Ladefehler. Fehlt der Block, bleibt die Rückfrage.
def harness(block, fassung=2):
    """Fassung 2 mit Block harness; fassung=1 hängt den Block an die Basis der Fassung 1."""
    if fassung == 1:
        return mit(lambda o: o.update(harness=block))
    return f2(lambda o: o.update(harness=block))


def cop(liste):
    return {"copilot": {"werkzeuge_ohne_rueckfrage": liste}}


fall("gueltig-harness-alle-drei", "Fassung 2 mit harness.copilot: shell, write und such-mcp ohne Rückfrage",
     harness(cop(["shell", "write", "such-mcp"])))
fall("gueltig-harness-eins", "ein Wert: gültig", harness(cop(["write"])))
fall("gueltig-harness-liste-leer", "leere Liste: gültig, die Rückfrage bleibt für alles", harness(cop([])))
fall("gueltig-harness-leer", "Block harness ohne Eintrag: gültig, die Rückfrage bleibt", harness({}))
fall("gueltig-harness-und-mcp", "harness und mcp zugleich: gültig",
     f2(lambda o: o.update(harness=cop(["such-mcp"]), mcp=MCP)))
fall("gueltig-vorfuehrung-mit-werkzeugen",
     "Vorführdatei mit harness.copilot: shell, write und such-mcp ohne Rückfrage", VORFUEHRDATEI_MIT_WERKZEUGEN)
fall("harness-in-fassung-1", "Block harness in einer Datei der Fassung 1", harness(cop(["shell"]), fassung=1),
     "einst.schluessel_unbekannt")
fall("harness-harness-unbekannt", "harness.claude: nur copilot hat in Fassung 2 einen Eintrag",
     harness({"claude": {"werkzeuge_ohne_rueckfrage": []}}), "einst.schluessel_unbekannt")
fall("harness-schluessel-unbekannt", "unbekannter Schlüssel in harness.copilot",
     harness({"copilot": {"werkzeuge_ohne_rueckfrage": [], "alles_erlaubt": True}}), "einst.schluessel_unbekannt")
fall("harness-schluessel-fehlt", "harness.copilot ohne werkzeuge_ohne_rueckfrage", harness({"copilot": {}}),
     "einst.schluessel_fehlt")
fall("harness-block-liste", "harness als Liste statt Objekt", harness([]), "einst.typ_falsch")
fall("harness-copilot-text", "harness.copilot als Text", harness({"copilot": "shell"}), "einst.typ_falsch")
fall("harness-werkzeuge-text", "werkzeuge_ohne_rueckfrage als Text statt Liste", harness(cop("shell")),
     "einst.typ_falsch")
fall("harness-wert-zahl", "ein Wert ist eine Zahl", harness(cop(["shell", 1])), "einst.typ_falsch")
fall("harness-wert-null", "ein Wert ist null", harness(cop([None])), "einst.typ_falsch")
fall("harness-doppelt", "derselbe Wert zweimal", harness(cop(["shell", "shell"])), "einst.kennung_doppelt")
for _name, _wert in [("allow-all-tools", "allow-all-tools"), ("allow-all-paths", "allow-all-paths"),
                     ("allow-all-urls", "allow-all-urls"), ("allow-all", "allow-all"), ("yolo", "yolo"),
                     ("gross", "Shell"), ("leer", ""), ("muster", "shell(git:*)"), ("stern", "*"),
                     ("mit-option", "--allow-all-tools"), ("fremd", "read")]:
    fall(f"harness-wert-{_name}", f"Wert {_wert!r} steht nicht unter den erlaubten Werten",
         harness(cop(["shell", _wert])), "einst.werkzeug_wert")
fall("harness-wert-nur-einer-falsch", "ein falscher Wert unter gültigen macht die Datei ungültig",
     harness(cop(["shell", "write", "yolo", "such-mcp"])), "einst.werkzeug_wert")
fall("harness-doppelt-vor-wert", "Doppel geht vor falschem Wert (Stufe 10 vor 34)",
     harness(cop(["yolo", "yolo"])), "einst.kennung_doppelt")
fall("harness-typ-vor-wert", "falscher Typ geht vor falschem Wert (Stufe 6 vor 34)",
     harness(cop(["yolo", 1])), "einst.typ_falsch")
fall("harness-netzziel-vor-wert", "Fehler im Netzziel gehen vor dem Wert in harness (Stufe 25 vor 34)",
     f2(lambda o: (o.update(harness=cop(["yolo"])), o["routen"][0].update(netzziele=[{"host": "localhost", "port": 443, "zugriff": "verbinden"}]))),
     "einst.netzziel_form")
fall("harness-wert-nach-frist", "mcp-Frist (Stufe 32) geht vor dem Wert in harness (Stufe 34)",
     f2(lambda o: o.update(harness=cop(["yolo"]), mcp=server(frist_ms=0))), "einst.frist_bereich")
fall("harness-wert-nach-mcp-doppelt", "mcp_doppelt (Stufe 33) geht vor dem Wert in harness (Stufe 34)",
     f2(lambda o: o.update(harness=cop(["yolo"]), mcp=server(lesen=["x"], schreiben=["x"]))), "einst.mcp_doppelt")
fall("harness-doppelter-schluessel", "harness.copilot zweimal im Objekt harness (doppelter JSON-Schlüssel)",
     text(harness(cop([]))).replace(
         '"copilot": {', '"copilot": {"werkzeuge_ohne_rueckfrage": []},\n  "copilot": {', 1), "einst.json_ungueltig")


# Plan Business (Nachtrag Business, 26.11.2026): der Schlüssel nie an der Unterdomänen-Form des Plans (Stufe 31); die
# Unterdomänen-Form ohne Schlüssel bleibt der Form nach gültig.
fall("schluessel-unterdomaene-business", "Schlüssel an *.business.githubcopilot.com gebunden (die Form der Doku)",
     kopf([{"methode": "POST", "pfad": "/chat/completions"}], host="*.business.githubcopilot.com"), "einst.schluessel_unterdomaene")
fall("schluessel-unterdomaene-business-neben-modell-host",
     "Modell-Host mit Schlüssel und daneben *.business.githubcopilot.com mit Schlüssel",
     kopf([{"methode": "POST", "pfad": "/chat/completions"}], host="api.business.githubcopilot.com",
          dazu=[{"host": "*.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
                 "pfade": [{"methode": "POST", "pfad": "/telemetry"}]}]), "einst.schluessel_unterdomaene")


# Admin-Schalter eingeschraenktes_token (Nachtrag Business, 26.11.2026): optional an einer Route der Fassung 2, Vorgabe aus.
# true heißt: Der Halter setzt an den Zielen der Route mit Schlüssel das eingeschränkte Token der Admins ein, nicht das Token
# der Geräte-Anmeldung. Gesetzt an einer Route ohne Ziel mit schluessel: true ist es Stufe 37 (der Schalter liefe ins Leere).
def schalter(wert, route=0, aendern=lambda o: None):
    def setzen(o):
        o["routen"][route]["eingeschraenktes_token"] = wert
        aendern(o)
    return setzen


_KOPF_ZIEL = {"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
              "pfade": [{"methode": "POST", "pfad": "/chat/completions"}]}


def mit_kopf(o):
    o["routen"][0]["netzziele"] = [copy.deepcopy(_KOPF_ZIEL)]


fall("gueltig-eingeschraenktes-token", "Schalter an einer Cloud-Route mit Ziel mit Schlüssel",
     f2(schalter(True, aendern=mit_kopf)))
fall("gueltig-eingeschraenktes-token-aus", "Schalter false (die Vorgabe) an einer Cloud-Route mit Ziel mit Schlüssel",
     f2(schalter(False, aendern=mit_kopf)))
fall("gueltig-eingeschraenktes-token-aus-lokal", "Schalter false an der lokalen Route: gültig, er wirkt nirgends",
     f2(schalter(False, route=1)))
_ERSTSTART_SCHALTER = copy.deepcopy(ERSTSTARTDATEI)
_ERSTSTART_SCHALTER["routen"][0]["eingeschraenktes_token"] = True
fall("gueltig-erststart-eingeschraenktes-token", "Erststart-Profil mit gesetztem Schalter an der Route copilot",
     _ERSTSTART_SCHALTER)
fall("eingeschraenktes-token-fassung-1", "Schalter in einer Datei der Fassung 1",
     mit(lambda o: o["routen"][0].update(eingeschraenktes_token=True)), "einst.schluessel_unbekannt")
fall("eingeschraenktes-token-text", "Schalter als Text", f2(schalter("ja", aendern=mit_kopf)), "einst.typ_falsch")
fall("eingeschraenktes-token-null", "Schalter null", f2(schalter(None, aendern=mit_kopf)), "einst.typ_falsch")
fall("eingeschraenktes-token-zahl", "Schalter als Zahl 1", f2(schalter(1, aendern=mit_kopf)), "einst.typ_falsch")
fall("eingeschraenktes-token-lokal", "Schalter true an der lokalen Route (ohne Netzziel)",
     f2(schalter(True, route=1)), "einst.token_ohne_schluessel")
fall("eingeschraenktes-token-ohne-schluessel", "Schalter true an einer Cloud-Route, deren Ziele keinen Schlüssel tragen",
     f2(schalter(True, aendern=lambda o: o["routen"][0].update(netzziele=[
         {"host": "telemetry.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": False,
          "pfade": [{"methode": "POST", "pfad": "/telemetry"}]}]))), "einst.token_ohne_schluessel")
fall("eingeschraenktes-token-ohne-netzziele", "Schalter true an einer Cloud-Route ohne Netzziel",
     f2(schalter(True, aendern=lambda o: o["routen"][0].update(netzziele=[]))), "einst.token_ohne_schluessel")
fall("eingeschraenktes-token-wert-vor-schalter", "Werkzeugwert (Stufe 34) geht vor dem Schalter (Stufe 37)",
     f2(schalter(True, route=1, aendern=lambda o: o.update(harness=cop(["yolo"])))), "einst.werkzeug_wert")
fall("eingeschraenktes-token-typ-vor-schalter", "falscher Typ des Schalters an der lokalen Route ist Stufe 6, nicht 37",
     f2(schalter("ja", route=1)), "einst.typ_falsch")


# Block effort (Nachtrag 26.11.2026, Fassung 2): Grenzen der Admins je Harness, Klasse und Modell. Das Crate liest den Block;
# die Fälle stehen in faelle/ und erwartet/, gültige Fälle mit der Liste `efforts` (README, Abschnitt „Block effort“).
# Werte je Harness, wörtlich aus der Hilfe der Programme (gemessen 26.11.2026: claude 2.1.332 --effort,
# copilot 1.0.116 --reasoning-effort, pi 1.6.1 --thinking).
EFFORTS = {
    "claude": ("low", "medium", "high", "xhigh", "max"),
    "copilot": ("none", "minimal", "low", "medium", "high", "xhigh", "max"),
    "pi": ("off", "minimal", "low", "medium", "high", "xhigh", "max"),
    # opencode 1.19.5: Wert von options.reasoningEffort am Modell, im Anfragekörper als reasoning_effort (gemessen 27.11.2026)
    "opencode": ("none", "minimal", "low", "medium", "high", "xhigh", "max"),
}
KENNUNG_E = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
MODELL_E = re.compile(r"^[A-Za-z0-9._:/@+-]{1,128}$")


def effort_code(datei):
    """Referenz der Stufen für den Block effort; die übrige Datei ist in diesen Fällen gültig."""
    if datei.get("fassung") != 2 and "effort" in datei:
        return "einst.schluessel_unbekannt"
    if "effort" not in datei:
        return None
    b = datei["effort"]
    stufen = []  # (stufe, code)
    if not isinstance(b, dict):
        return "einst.typ_falsch"
    if set(b) - {"grenzen"}:
        stufen.append((3, "einst.schluessel_unbekannt"))
    if "grenzen" not in b:
        stufen.append((5, "einst.schluessel_fehlt"))
        return min(stufen)[1]
    g = b["grenzen"]
    if not isinstance(g, list):
        stufen.append((6, "einst.typ_falsch"))
        return min(stufen)[1]
    if len(g) > 64:
        stufen.append((8, "einst.anzahl"))
    gesehen = set()
    klassen = datei["klassen"]
    for e in g:
        if not isinstance(e, dict):
            stufen.append((6, "einst.typ_falsch"))
            continue
        if set(e) - {"harness", "klasse", "modell", "erlaubt"}:
            stufen.append((3, "einst.schluessel_unbekannt"))
        if not {"harness", "erlaubt"} <= set(e):
            stufen.append((5, "einst.schluessel_fehlt"))
            continue
        if not isinstance(e["harness"], str) or ("klasse" in e and not isinstance(e["klasse"], str)) \
                or ("modell" in e and not isinstance(e["modell"], str)) or not isinstance(e["erlaubt"], list) \
                or any(not isinstance(x, str) for x in e["erlaubt"]):
            stufen.append((6, "einst.typ_falsch"))
            continue
        if not 1 <= len(e["erlaubt"]) <= 7:
            stufen.append((8, "einst.anzahl"))
        if not KENNUNG_E.match(e["harness"]) or ("klasse" in e and not KENNUNG_E.match(e["klasse"])):
            stufen.append((9, "einst.kennung_form"))
        if len(set(e["erlaubt"])) != len(e["erlaubt"]):
            stufen.append((10, "einst.kennung_doppelt"))
        schl = (e["harness"], e.get("klasse"), e.get("modell"))
        if schl in gesehen:
            stufen.append((10, "einst.kennung_doppelt"))
        gesehen.add(schl)
        if "klasse" in e and e["klasse"] not in klassen:
            stufen.append((12, "einst.klasse_unbekannt"))
        if "modell" in e and not MODELL_E.match(e["modell"]):
            stufen.append((23, "einst.modell_form"))
        if e["harness"] not in EFFORTS:
            stufen.append((35, "einst.effort_harness"))
        elif any(x not in EFFORTS[e["harness"]] for x in e["erlaubt"]):
            stufen.append((36, "einst.effort_wert"))
    # Stufe 34 (harness.copilot.werkzeuge_ohne_rueckfrage) steht vor dem Effort; nur für den Fall der Reihenfolge.
    werkzeuge = datei.get("harness", {}).get("copilot", {}).get("werkzeuge_ohne_rueckfrage", [])
    if any(w not in ("shell", "write", "such-mcp") for w in werkzeuge):
        stufen.append((34, "einst.werkzeug_wert"))
    return min(stufen)[1] if stufen else None


def erlaubte_efforts(datei, harness, route, modell):
    """Die Efforts, die ein Start mit diesem Harness, dieser Route und diesem Modell wählen darf (Reihenfolge der Hilfe)."""
    klasse = next(r["klasse"] for r in datei["routen"] if r["schluessel"] == route)
    passend = [e for e in datei.get("effort", {}).get("grenzen", []) if e["harness"] == harness
               and e.get("klasse", klasse) == klasse and e.get("modell", modell) == modell]
    if not passend:
        return []
    return [w for w in EFFORTS[harness] if all(w in e["erlaubt"] for e in passend)]


def fall_effort(name, beschreibung, datei, soll, efforts=None):
    """Ein Fall zum Block effort; soll ist der ausgeschriebene Code, die Referenz muss ihn treffen. `efforts` nennt
    für gültige Fälle die erlaubten Efforts je (Harness, Route, Modell); sie stehen auch in der Erwartung."""
    ist = effort_code(datei)
    if ist != soll:
        raise SystemExit(f"FEHLER {name}: Referenz {ist!r}, Tabelle {soll!r}")
    if efforts:
        for (h, r, m), liste in efforts:
            if erlaubte_efforts(datei, h, r, m) != liste:
                raise SystemExit(f"FEHLER {name}: erlaubte_efforts({h}, {r}, {m}) = {erlaubte_efforts(datei, h, r, m)!r}, Tabelle {liste!r}")
    erwartet = {"gueltig": True} if soll is None else {"gueltig": False, "code": soll}
    if efforts:
        erwartet["efforts"] = [{"harness": h, "route": r, "modell": m, "erlaubt": liste} for (h, r, m), liste in efforts]
    FAELLE.append((name, {"beschreibung": beschreibung, "text": text(datei)}, erwartet))


def eff(grenzen, **mehr):
    block = {"grenzen": grenzen}
    block.update(mehr)
    return f2(lambda o: o.update(effort=block))


GRENZEN = [{"harness": "copilot", "erlaubt": ["low", "medium", "high"]},
           {"harness": "copilot", "klasse": "intern", "modell": "gpt-6-luna", "erlaubt": ["low", "medium"]},
           {"harness": "pi", "klasse": "kundendaten", "erlaubt": ["off", "low"]}]
fall_effort("gueltig-effort", "Grenzen für copilot allgemein, für copilot mit gpt-6-luna in intern enger, für pi in kundendaten",
             eff(GRENZEN), None,
             [(("copilot", "cloud-beispiel", "gpt-6-luna"), ["low", "medium"]),
            (("copilot", "cloud-beispiel", "auto"), ["low", "medium", "high"]),
            (("pi", "lokal-beispiel", "qwen3:8b"), ["off", "low"]),
            (("claude", "cloud-beispiel", "claude-sonnet-5-5"), [])])
fall_effort("gueltig-erststart",
             "Erststart-Profil: am Modell-Host nur GET /models und POST /chat/completions, Modell fest gpt-6-luna, "
             "Effort für copilot mit gpt-6-luna nur none und low", ERSTSTARTDATEI, None,
             [(("copilot", "copilot", "gpt-6-luna"), ["none", "low"]),
              (("copilot", "copilot", "auto"), []),
              (("pi", "lokal", "qwen3:8b"), [])])
fall_effort("gueltig-effort-leer", "leere Liste: gültig, kein Harness bekommt einen Effort", eff([]), None,
             [(("copilot", "cloud-beispiel", "auto"), [])])
fall_effort("gueltig-ohne-effort", "Fassung 2 ohne Block: gültig, kein Effort wählbar (fail closed)", f2(), None,
             [(("pi", "lokal-beispiel", "qwen3:8b"), [])])
fall_effort("effort-in-fassung-1", "Block effort in einer Datei der Fassung 1",
             mit(lambda o: o.update(effort={"grenzen": []})), "einst.schluessel_unbekannt")
fall_effort("effort-schluessel-unbekannt", "unbekannter Schlüssel im Block", eff([], vorgabe="low"), "einst.schluessel_unbekannt")
fall_effort("effort-eintrag-unbekannt", "unbekannter Schlüssel in einer Grenze",
             eff([{"harness": "pi", "erlaubt": ["low"], "route": "lokal-beispiel"}]), "einst.schluessel_unbekannt")
fall_effort("effort-grenzen-fehlt", "Block ohne grenzen", f2(lambda o: o.update(effort={})), "einst.schluessel_fehlt")
fall_effort("effort-erlaubt-fehlt", "Grenze ohne erlaubt", eff([{"harness": "pi"}]), "einst.schluessel_fehlt")
fall_effort("effort-typ", "erlaubt als Text", eff([{"harness": "pi", "erlaubt": "low"}]), "einst.typ_falsch")
fall_effort("effort-anzahl-leer", "erlaubt leer", eff([{"harness": "pi", "erlaubt": []}]), "einst.anzahl")
fall_effort("effort-anzahl-65", "65 Grenzen",
             eff([{"harness": "pi", "modell": f"m{i}", "erlaubt": ["low"]} for i in range(65)]), "einst.anzahl")
fall_effort("effort-doppelt-wert", "derselbe Wert zweimal", eff([{"harness": "pi", "erlaubt": ["low", "low"]}]),
             "einst.kennung_doppelt")
fall_effort("effort-doppelt-eintrag", "zwei Grenzen für denselben Harness, dieselbe Klasse und dasselbe Modell",
             eff([{"harness": "pi", "erlaubt": ["low"]}, {"harness": "pi", "erlaubt": ["high"]}]), "einst.kennung_doppelt")
fall_effort("effort-klasse-unbekannt", "Klasse, die nicht in klassen steht",
             eff([{"harness": "pi", "klasse": "geheim", "erlaubt": ["low"]}]), "einst.klasse_unbekannt")
fall_effort("effort-modell-form", "Modell mit Leerzeichen", eff([{"harness": "copilot", "modell": "gpt 6", "erlaubt": ["low"]}]),
             "einst.modell_form")
fall_effort("effort-harness-unbekannt", "Harness, der nicht in der Harness-Tabelle von Vertrag 1 steht",
             eff([{"harness": "codex", "erlaubt": ["low"]}]), "einst.effort_harness")
fall_effort("effort-wert-fremd", "none gibt es bei copilot, nicht bei pi", eff([{"harness": "pi", "erlaubt": ["none"]}]),
             "einst.effort_wert")
fall_effort("gueltig-effort-opencode", "opencode (Entscheid des Betreibers, 27.11.2026) in kundendaten mit none und high",
             eff([{"harness": "opencode", "klasse": "kundendaten", "erlaubt": ["none", "high"]}]), None,
             [(("opencode", "lokal-beispiel", "qwen3:8b"), ["none", "high"]),
              (("pi", "lokal-beispiel", "qwen3:8b"), [])])
fall_effort("effort-wert-fremd-opencode", "off gibt es bei pi, nicht bei opencode", eff([{"harness": "opencode", "erlaubt": ["off"]}]),
             "einst.effort_wert")
fall_effort("effort-wert-gross", "Groß geschrieben", eff([{"harness": "claude", "erlaubt": ["High"]}]), "einst.effort_wert")
fall_effort("effort-wert-ultra", "ein Wert, den kein Harness kennt", eff([{"harness": "copilot", "erlaubt": ["low", "ultra"]}]),
             "einst.effort_wert")
fall_effort("effort-harness-vor-wert", "unbekannter Harness geht vor dem Wert (Stufe 35 vor 36)",
             eff([{"harness": "pi", "erlaubt": ["ultra"]}, {"harness": "codex", "erlaubt": ["low"]}]), "einst.effort_harness")
fall_effort("effort-nach-werkzeug", "Werkzeugwert in harness (Stufe 34) geht vor dem Effort (Stufe 36)",
             f2(lambda o: o.update(harness={"copilot": {"werkzeuge_ohne_rueckfrage": ["yolo"]}},
                                   effort={"grenzen": [{"harness": "pi", "erlaubt": ["ultra"]}]})), "einst.werkzeug_wert")


def main():
    for teil in ("faelle", "erwartet"):
        ordner = HIER / teil
        ordner.mkdir(exist_ok=True)
        for alt in ordner.glob("*.json"):
            alt.unlink()
    for name, eintrag, erwartet in FAELLE:
        (HIER / "faelle" / f"{name}.json").write_text(text(eintrag), encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(text(erwartet), encoding="utf-8")
    for teil in ("faelle-ausstehend", "erwartet-ausstehend"):
        # bis 27.11.2026 standen die Fälle zum Block effort hier; das Crate liest sie seitdem aus faelle/
        ordner = HIER / teil
        if ordner.is_dir():
            for alt in ordner.glob("*.json"):
                alt.unlink()
            ordner.rmdir()
    (HIER / "vorfuehrung.json").write_text(text(VORFUEHRDATEI_MIT_WERKZEUGEN), encoding="utf-8")
    (HIER / "erststart.json").write_text(text(ERSTSTARTDATEI), encoding="utf-8")
    print(f"{len(FAELLE)} Fälle geschrieben")


if __name__ == "__main__":
    main()
