"""Builds the visible cases of the contract `datentor` (rust/spec/VERTRAEGE.md).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json (the input on stdin of the program) and sichtbar/erwartet/<name>.json (exit code and
expected output). Every expectation is written out by hand in the tables below. The script also checks each one against a
small reference of the rules in rust/spec/VERTRAEGE.md, section 4, and each vorgabe against the validity rules of section 2; a
mismatch stops the script before anything is written. All data is invented.
"""
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "sichtbar"
SCHLUESSEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")

MELDUNG = {
    "frei": "frei",
    "route-fehlt": "Anfrage ohne Route",
    "route-ohne-klasse": "Route {route} hat keine Klasse",
    "einheit-ohne-klasse": "Einheit ohne gültige Klasse",
    "klasse-ueber-route": "Klasse der Einheit liegt über Route {route}",
    "kunde-fehlt": "Kundendaten ohne Kunde",
    "kunde-ohne-eintrag": "Kunde ohne Eintrag in der Kundenvorgabe",
    "route-fuer-kunde-verboten": "Route {route} ist für diesen Kunden verboten",
    "route-fuer-kunde-nicht-erlaubt": "Route {route} ist für diesen Kunden nicht erlaubt",
    "kundendaten-an-cloud": "Kundendaten an Cloud-Route {route} ohne Freigabe",
}

KL3 = ["oeffentlich", "intern", "kundendaten"]
NIE = {"vorgabe": "nie", "freigaben": []}


def route(schluessel, art, klasse="-"):
    r = {"schluessel": schluessel, "art": art}
    if klasse != "-":
        r["klasse"] = klasse
    return r


def kunde(schluessel, erlaubt=None, verboten=None):
    k = {"schluessel": schluessel}
    if erlaubt is not None:
        k["erlaubt"] = erlaubt
    if verboten is not None:
        k["verboten"] = verboten
    return k


def freigaben(*paare):
    return {"vorgabe": "nie", "freigaben": [{"route": r, "kunde": k} for r, k in paare]}


# The settings of the demonstration (same as the visible cases of contract 4).
V_VORFUEHRUNG = {
    "klassen": KL3,
    "routen": [route("cloud", "cloud", "intern"), route("lokal", "lokal", "kundendaten")],
    "kunden": [kunde("k-nord", erlaubt=["lokal"])],
    "kundendaten_an_cloud": NIE,
}
V_A42 = {
    "klassen": KL3,
    "routen": [route("cloud", "cloud", "intern"), route("cloud-n", "cloud", None), route("cloud-o", "cloud"),
               route("lokal", "lokal", "kundendaten")],
    "kunden": [kunde("k-nord", erlaubt=["lokal"])],
}
V_STUFE_INTERN = dict(V_VORFUEHRUNG, vorgabestufe="intern")
V_ORDNUNG = {
    "klassen": KL3,
    "routen": [route("r-oeff", "lokal", "oeffentlich"), route("r-intern", "lokal", "intern"),
               route("r-kd", "lokal", "kundendaten")],
    "kunden": [],
}
V_VIER = {"klassen": ["s0", "s1", "s2", "s3"], "routen": [route("r2", "lokal", "s2"), route("r3", "lokal", "s3")],
          "kunden": [kunde("k-a", erlaubt=["r3"])]}
V_EINE = {"klassen": ["nur"], "routen": [route("r", "lokal", "nur")], "kunden": [kunde("k-a", erlaubt=["r"])]}
V_A51 = {
    "klassen": KL3,
    "routen": [route("r1", "lokal", "kundendaten"), route("r2", "lokal", "kundendaten")],
    "kunden": [kunde("k-a", erlaubt=["r1"]), kunde("k-b", erlaubt=["r1"], verboten=["r1"]), kunde("k-ohne-liste")],
}
V_A50_ROUTEN = [route("c1", "cloud", "kundendaten"), route("c2", "cloud", "kundendaten"),
                route("c-intern", "cloud", "intern"), route("lokal", "lokal", "kundendaten")]
V_A50_KUNDEN = [kunde("k-k", erlaubt=["c1", "c2", "c-intern", "lokal"]), kunde("k-l", erlaubt=["c1", "lokal"])]
V_A50 = {"klassen": KL3, "routen": V_A50_ROUTEN, "kunden": V_A50_KUNDEN, "kundendaten_an_cloud": NIE}
V_A50_FREI = {"klassen": KL3, "routen": V_A50_ROUTEN, "kunden": V_A50_KUNDEN,
              "kundendaten_an_cloud": freigaben(("c1", "k-k"), ("c-intern", "k-k"))}
V_A52_ROUTEN = [route("k-k-werkzeug", "cloud", "kundendaten"), route("c1", "cloud", "kundendaten"),
                route("lokal", "lokal", "kundendaten")]
V_A52_KUNDEN = [kunde("k-k", erlaubt=["k-k-werkzeug", "c1", "lokal"])]
V_A52 = {"klassen": KL3, "routen": V_A52_ROUTEN, "kunden": V_A52_KUNDEN}
V_A52_FREI = {"klassen": KL3, "routen": V_A52_ROUTEN, "kunden": V_A52_KUNDEN,
              "kundendaten_an_cloud": freigaben(("k-k-werkzeug", "k-k"))}
V_DOPPEL = {
    "klassen": KL3,
    "routen": [route("c1", "cloud", "kundendaten"), route("c2", "cloud", "kundendaten"), route("lokal", "lokal", "kundendaten")],
    "kunden": [kunde("k", erlaubt=["c1", "c1", "lokal"], verboten=["c2", "c2"])],
    "kundendaten_an_cloud": freigaben(("c1", "k"), ("c1", "k")),
}
V_REIHE = {
    "klassen": KL3,
    "routen": [route("c-intern", "cloud", "intern"), route("c1", "cloud", "kundendaten")],
    "kunden": [kunde("k-v", erlaubt=["c1"], verboten=["c1"]), kunde("k-n", erlaubt=[])],
}

FEHLT = object()


def anf(klasse, route_, kunde_=None):
    """One request. FEHLT leaves the field out."""
    a = {"einheit": {} if klasse is FEHLT else {"klasse": klasse}}
    if route_ is not FEHLT:
        a["route"] = route_
    if kunde_ is not FEHLT:
        a["kunde"] = kunde_
    return a


# (name, rules, description, vorgabe, [(request, (expected reason, expected effective class))])
FAELLE = [
    ("d1-vorfuehrung", ["D1", "D5", "D6", "A34"],
     "Schritt 3 und 5 der Vorführung: intern kommt über die Cloud-Route, die Kundennotiz nur über die lokale Route.",
     V_VORFUEHRUNG, [
         (anf("intern", "cloud"), ("frei", "intern")),
         (anf("kundendaten", "cloud", "k-nord"), ("klasse-ueber-route", "kundendaten")),
         (anf("kundendaten", "lokal", "k-nord"), ("frei", "kundendaten")),
         (anf("oeffentlich", "cloud"), ("frei", "oeffentlich")),
         (anf("intern", "lokal"), ("frei", "intern")),
     ]),
    ("d1-keine-anfrage", ["D1"], "Eine leere Liste von Anfragen ergibt eine leere Liste von Antworten.",
     V_VORFUEHRUNG, []),
    ("d2-route-fehlt", ["D2", "A43", "D12"],
     "Leere, fehlende und null-Route werden abgelehnt, vor jeder anderen Prüfung; die Klasse steht trotzdem in der Antwort.",
     V_VORFUEHRUNG, [
         (anf("intern", ""), ("route-fehlt", "intern")),
         (anf("intern", None), ("route-fehlt", "intern")),
         (anf("intern", FEHLT), ("route-fehlt", "intern")),
         (anf(7, None), ("route-fehlt", None)),
         (anf("intern", "", "k-nord"), ("route-fehlt", "kundendaten")),
     ]),
    ("d3-route-ohne-klasse", ["D3", "A42", "D12"],
     "Unbekannte Route, Route mit Klasse null und Route ohne Feld klasse sind gesperrt; die Gegenprobe mit Klasse ist frei.",
     V_A42, [
         (anf("intern", "cloud-x"), ("route-ohne-klasse", "intern")),
         (anf("intern", "cloud-n"), ("route-ohne-klasse", "intern")),
         (anf("intern", "cloud-o"), ("route-ohne-klasse", "intern")),
         (anf("intern", "cloud"), ("frei", "intern")),
         (anf("geheim", "cloud-x"), ("route-ohne-klasse", None)),
         (anf("kundendaten", "cloud-n", "k-nord"), ("route-ohne-klasse", "kundendaten")),
     ]),
    ("d4-einheit-ohne-klasse", ["D4", "D14"],
     "Eine Klasse, die kein Text ist oder nicht in klassen steht, sperrt; auch mit Kunde. Vergleich byte-genau.",
     V_VORFUEHRUNG, [
         (anf("geheim", "lokal"), ("einheit-ohne-klasse", None)),
         (anf("INTERN", "lokal"), ("einheit-ohne-klasse", None)),
         (anf(" intern", "lokal"), ("einheit-ohne-klasse", None)),
         (anf("", "lokal"), ("einheit-ohne-klasse", None)),
         (anf(5, "lokal"), ("einheit-ohne-klasse", None)),
         (anf(True, "lokal"), ("einheit-ohne-klasse", None)),
         (anf(["intern"], "lokal"), ("einheit-ohne-klasse", None)),
         (anf({"name": "intern"}, "lokal"), ("einheit-ohne-klasse", None)),
         (anf("geheim", "lokal", "k-nord"), ("einheit-ohne-klasse", None)),
     ]),
    ("d5-vorgabestufe-fehlt", ["D5", "A35", "D7"],
     "Ohne Feld vorgabestufe bekommt eine Einheit ohne Klasse die höchste Klasse und kommt auf keiner Route an.",
     V_VORFUEHRUNG, [
         (anf(None, "cloud"), ("klasse-ueber-route", "kundendaten")),
         (anf(FEHLT, "lokal"), ("kunde-fehlt", "kundendaten")),
         (anf(None, "lokal", "k-nord"), ("frei", "kundendaten")),
     ]),
    ("d5-vorgabestufe-intern", ["D5", "A35"],
     "Der Admin setzt die Vorgabestufe auf intern; dieselbe Einheit ohne Klasse kommt über die Cloud-Route an.",
     V_STUFE_INTERN, [
         (anf(None, "cloud"), ("frei", "intern")),
         (anf(FEHLT, "cloud"), ("frei", "intern")),
         (anf(None, "cloud", "k-nord"), ("klasse-ueber-route", "kundendaten")),
     ]),
    ("d5-kunde-hebt", ["D5"],
     "Eine Einheit mit Kunde gilt als höchste Klasse; intern mit Kunde k-nord ergibt kundendaten. Ein leerer Text ist ein Kunde.",
     V_VORFUEHRUNG, [
         (anf("intern", "lokal", "k-nord"), ("frei", "kundendaten")),
         (anf("intern", "cloud", "k-nord"), ("klasse-ueber-route", "kundendaten")),
         (anf("oeffentlich", "lokal", "k-nord"), ("frei", "kundendaten")),
         (anf("intern", "lokal", None), ("frei", "intern")),
         (anf("intern", "cloud", ""), ("klasse-ueber-route", "kundendaten")),
         (anf("intern", "lokal", ""), ("kunde-ohne-eintrag", "kundendaten")),
     ]),
    ("d6-klassenordnung", ["D6", "D7", "A34"],
     "Die Klasse der Route ist die höchste, die sie sehen darf.",
     V_ORDNUNG, [
         (anf("oeffentlich", "r-oeff"), ("frei", "oeffentlich")),
         (anf("intern", "r-oeff"), ("klasse-ueber-route", "intern")),
         (anf("kundendaten", "r-oeff"), ("klasse-ueber-route", "kundendaten")),
         (anf("intern", "r-intern"), ("frei", "intern")),
         (anf("oeffentlich", "r-kd"), ("frei", "oeffentlich")),
         (anf("kundendaten", "r-kd"), ("kunde-fehlt", "kundendaten")),
     ]),
    ("d6-vier-klassen", ["D6", "D5"],
     "Das Tor vergleicht nur Positionen; die Namen der Klassen bedeuten ihm nichts.",
     V_VIER, [
         (anf("s1", "r2"), ("frei", "s1")),
         (anf("s2", "r2"), ("frei", "s2")),
         (anf("s3", "r2", "k-a"), ("klasse-ueber-route", "s3")),
         (anf("s0", "r3", "k-a"), ("frei", "s3")),
         (anf("s3", "r3"), ("kunde-fehlt", "s3")),
     ]),
    ("d6-eine-klasse", ["D6", "D7"], "Mit nur einer Klasse ist jede Einheit Kundendaten.",
     V_EINE, [
         (anf("nur", "r"), ("kunde-fehlt", "nur")),
         (anf("nur", "r", "k-a"), ("frei", "nur")),
     ]),
    ("d8-d10-kundenvorgabe", ["D8", "D9", "D10", "A51", "D14"],
     "Kunde ohne Eintrag bekommt keine Route; verboten geht vor erlaubt; ohne Liste nichts erlaubt.",
     V_A51, [
         (anf("kundendaten", "r1", "k-a"), ("frei", "kundendaten")),
         (anf("kundendaten", "r1", "k-b"), ("route-fuer-kunde-verboten", "kundendaten")),
         (anf("kundendaten", "r1", "k-c"), ("kunde-ohne-eintrag", "kundendaten")),
         (anf("kundendaten", "r2", "k-a"), ("route-fuer-kunde-nicht-erlaubt", "kundendaten")),
         (anf("kundendaten", "r1", "k-ohne-liste"), ("route-fuer-kunde-nicht-erlaubt", "kundendaten")),
         (anf("kundendaten", "r1", ""), ("kunde-ohne-eintrag", "kundendaten")),
         (anf("kundendaten", "r1", "K-A"), ("kunde-ohne-eintrag", "kundendaten")),
         (anf("intern", "r1"), ("frei", "intern")),
     ]),
    ("d11-cloud-nie", ["D11", "A50"],
     "Vorgabe nie: Kundendaten gehen nicht an eine Cloud-Route, auch nicht mit höchster Klasse und Eintrag beim Kunden.",
     V_A50, [
         (anf("kundendaten", "c1", "k-k"), ("kundendaten-an-cloud", "kundendaten")),
         (anf("kundendaten", "lokal", "k-k"), ("frei", "kundendaten")),
         (anf("intern", "c1"), ("frei", "intern")),
     ]),
    ("d11-cloud-freigabe", ["D11", "A50"],
     "Freigabe für genau (c1, k-k): k-k über c1 frei; k-l über c1 und k-k über c2 nicht; die Freigabe hebt die Klasse nicht.",
     V_A50_FREI, [
         (anf("kundendaten", "c1", "k-k"), ("frei", "kundendaten")),
         (anf("kundendaten", "c1", "k-l"), ("kundendaten-an-cloud", "kundendaten")),
         (anf("kundendaten", "c2", "k-k"), ("kundendaten-an-cloud", "kundendaten")),
         (anf("kundendaten", "c-intern", "k-k"), ("klasse-ueber-route", "kundendaten")),
         (anf("intern", "c1", "k-k"), ("frei", "kundendaten")),
     ]),
    ("d11-kundenwerkzeug", ["D11", "A52"],
     "Das Cloud-Werkzeug des Kunden ist eine eigene Route; ohne Freigabe gehen keine Kundendaten dorthin.",
     V_A52, [
         (anf("kundendaten", "k-k-werkzeug", "k-k"), ("kundendaten-an-cloud", "kundendaten")),
         (anf("kundendaten", "lokal", "k-k"), ("frei", "kundendaten")),
     ]),
    ("d11-kundenwerkzeug-freigabe", ["D11", "A52"],
     "Freigabe nur für diese Route und k-k; eine andere Cloud-Route bleibt ohne Kundendaten.",
     V_A52_FREI, [
         (anf("kundendaten", "k-k-werkzeug", "k-k"), ("frei", "kundendaten")),
         (anf("kundendaten", "c1", "k-k"), ("kundendaten-an-cloud", "kundendaten")),
     ]),
    ("d12-reihenfolge", ["D12"],
     "Treffen mehrere Gründe zu, gilt der erste in der Reihenfolge der Tabelle.",
     V_REIHE, [
         (anf("kundendaten", "c-intern", "k-x"), ("klasse-ueber-route", "kundendaten")),
         (anf("kundendaten", "c1", "k-x"), ("kunde-ohne-eintrag", "kundendaten")),
         (anf("kundendaten", "c1", "k-v"), ("route-fuer-kunde-verboten", "kundendaten")),
         (anf("kundendaten", "c1", "k-n"), ("route-fuer-kunde-nicht-erlaubt", "kundendaten")),
         (anf("geheim", "c-x", "k-x"), ("route-ohne-klasse", None)),
         (anf("geheim", "c1", "k-x"), ("einheit-ohne-klasse", None)),
     ]),
    ("d14-byte-genau", ["D14", "D3"],
     "Route und Kunde werden byte-genau verglichen; die Meldung nennt die Route so, wie sie übergeben wurde.",
     V_VORFUEHRUNG, [
         (anf("intern", "Cloud"), ("route-ohne-klasse", "intern")),
         (anf("intern", "cloud "), ("route-ohne-klasse", "intern")),
         (anf("intern", "clöud"), ("route-ohne-klasse", "intern")),
         (anf("kundendaten", "lokal", "k-nörd"), ("kunde-ohne-eintrag", "kundendaten")),
         (anf("kundendaten", "lokal", "k-nord "), ("kunde-ohne-eintrag", "kundendaten")),
     ]),
    ("d13-doppel-in-listen", ["D9", "D10", "D11", "D13"],
     "Doppelte Routen in erlaubt und verboten und doppelte Freigaben sind gültig und ändern nichts (Nachtrag 20.11.).",
     V_DOPPEL, [
         (anf("kundendaten", "c1", "k"), ("frei", "kundendaten")),
         (anf("kundendaten", "c2", "k"), ("route-fuer-kunde-verboten", "kundendaten")),
         (anf("kundendaten", "lokal", "k"), ("frei", "kundendaten")),
     ]),
    ("d13-tiefe-64", ["D13"], "Verschachtelung genau 64; das oberste Objekt ist Ebene 1 (Nachtrag 20.11.).",
     V_VORFUEHRUNG, [
         (anf("intern", "cloud"), ("frei", "intern")),
     ]),
]
# Faelle, deren nicht ausgewertetes Feld beschreibung eine Verschachtelung traegt (Name -> Tiefe des ganzen Falls).
TIEFE = {"d13-tiefe-64": 64}


def ungueltig_vorgabe(name, beschreibung, aendern):
    v = json.loads(json.dumps(V_VORFUEHRUNG))
    aendern(v)
    return (name, ["D13"], beschreibung, {"vorgabe": v, "anfragen": [anf("intern", "cloud")]})


def setze(pfad, wert):
    def f(v):
        ziel = v
        for teil in pfad[:-1]:
            ziel = ziel[teil]
        ziel[pfad[-1]] = wert
    return f


def loesche(pfad):
    def f(v):
        ziel = v
        for teil in pfad[:-1]:
            ziel = ziel[teil]
        del ziel[pfad[-1]]
    return f


def tief(n):
    x = []
    for _ in range(n):
        x = [x]
    return x


# (name, rules, description, case as object, or bytes for a raw input)
UNGUELTIG = [
    ("u-kein-json", ["D13"], "Die Eingabe ist kein JSON.", b'{"vorgabe": '),
    ("u-liste", ["D13"], "Die Eingabe ist kein Objekt.", b"[]"),
    ("u-ohne-vorgabe", ["D13"], "Das Feld vorgabe fehlt.", {"anfragen": []}),
    ("u-ohne-anfragen", ["D13"], "Das Feld anfragen fehlt.", {"vorgabe": V_VORFUEHRUNG}),
    ("u-anfragen-objekt", ["D13"], "anfragen ist keine Liste.", {"vorgabe": V_VORFUEHRUNG, "anfragen": {}}),
    ("u-feld-im-fall", ["D13"], "Ein unbekanntes Feld im Fall.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [], "zusatz": 1}),
    ("u-anfrage-ohne-einheit", ["D13"], "Eine Anfrage ohne einheit.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [{"route": "cloud"}]}),
    ("u-einheit-text", ["D13"], "einheit ist kein Objekt.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [{"einheit": "intern", "route": "cloud"}]}),
    ("u-einheit-feld", ["D13"], "Die Einheit trägt ein Feld außer klasse.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [{"einheit": {"klasse": "intern", "titel": "x"}, "route": "cloud"}]}),
    ("u-anfrage-feld", ["D13"], "Die Anfrage trägt ein Feld außer einheit, route und kunde.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [dict(anf("intern", "cloud"), person="p")]}),
    ("u-route-zahl", ["D13"], "route ist weder Text noch null.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [anf("intern", 5)]}),
    ("u-kunde-zahl", ["D13"], "kunde ist weder Text noch null.",
     {"vorgabe": V_VORFUEHRUNG, "anfragen": [anf("intern", "lokal", 5)]}),
    ("u-zu-tief", ["D13"], "Verschachtelung über 64 Ebenen (im nicht ausgewerteten Feld beschreibung).",
     {"beschreibung": tief(70), "vorgabe": V_VORFUEHRUNG, "anfragen": []}),
    ("u-tiefe-65", ["D13"], "Verschachtelung 65; das oberste Objekt ist Ebene 1 (Nachtrag 20.11.).",
     {"beschreibung": tief(63), "vorgabe": V_VORFUEHRUNG, "anfragen": []}),
    ("u-doppel-fall", ["D13"], "Ein doppelter Schlüssel im Fall (Nachtrag 20.11., fail closed).",
     b'{"vorgabe": {"klassen": ["a"], "routen": [], "kunden": []}, "anfragen": [], "anfragen": []}'),
    ("u-doppel-route", ["D13"], "Ein doppelter Schlüssel in einer Route; der letzte Wert wäre lockerer (Nachtrag 20.11.).",
     b'{"vorgabe": {"klassen": ["intern", "kundendaten"], "routen": [{"schluessel": "cloud", "art": "cloud", '
     b'"klasse": "intern", "klasse": "kundendaten"}], "kunden": []}, "anfragen": [{"einheit": {"klasse": "kundendaten"}, '
     b'"route": "cloud"}]}'),
    ("u-doppel-einheit", ["D13"], "Ein doppelter Schlüssel in der Einheit (Nachtrag 20.11.).",
     b'{"vorgabe": {"klassen": ["intern", "kundendaten"], "routen": [{"schluessel": "lokal", "art": "lokal", '
     b'"klasse": "kundendaten"}], "kunden": []}, "anfragen": [{"einheit": {"klasse": "kundendaten", "klasse": "intern"}, '
     b'"route": "lokal"}]}'),
    ("u-doppel-tief", ["D13"], "Ein doppelter Schlüssel tief im nicht ausgewerteten Feld beschreibung (Nachtrag 20.11.).",
     b'{"beschreibung": [[{"x": 1, "x": 2}]], "vorgabe": {"klassen": ["a"], "routen": [], "kunden": []}, "anfragen": []}'),
    ungueltig_vorgabe("u-vorgabestufe-null", "vorgabestufe ist null (Nachtrag 20.11.).", setze(["vorgabestufe"], None)),
    ungueltig_vorgabe("u-klassen-leer", "klassen ist leer.", setze(["klassen"], [])),
    ungueltig_vorgabe("u-klassen-doppelt", "Eine Klasse steht doppelt in klassen.",
                      setze(["klassen"], ["oeffentlich", "intern", "intern", "kundendaten"])),
    ungueltig_vorgabe("u-klasse-form", "Ein Klassenschlüssel hat nicht die Schlüsselform.",
                      setze(["klassen"], ["oeffentlich", "Intern", "kundendaten"])),
    ungueltig_vorgabe("u-ohne-routen", "Das Pflichtfeld routen fehlt.", loesche(["routen"])),
    ungueltig_vorgabe("u-ohne-kunden", "Das Pflichtfeld kunden fehlt.", loesche(["kunden"])),
    ungueltig_vorgabe("u-vorgabe-feld", "Ein unbekanntes Feld in der Vorgabe.", setze(["zonen"], [])),
    ungueltig_vorgabe("u-route-doppelt", "Ein Routenschlüssel steht doppelt.",
                      setze(["routen", 1, "schluessel"], "cloud")),
    ungueltig_vorgabe("u-route-form", "Ein Routenschlüssel hat nicht die Schlüsselform.",
                      setze(["routen", 0, "schluessel"], "Cloud")),
    ungueltig_vorgabe("u-route-art", "art ist weder cloud noch lokal.", setze(["routen", 0, "art"], "hybrid")),
    ungueltig_vorgabe("u-route-ohne-art", "Das Pflichtfeld art fehlt.", loesche(["routen", 0, "art"])),
    ungueltig_vorgabe("u-route-klasse-unbekannt", "Die Klasse einer Route steht nicht in klassen.",
                      setze(["routen", 0, "klasse"], "geheim")),
    ungueltig_vorgabe("u-route-klasse-zahl", "Die Klasse einer Route ist weder Text noch null.",
                      setze(["routen", 0, "klasse"], 1)),
    ungueltig_vorgabe("u-route-feld", "Ein unbekanntes Feld an einer Route.", setze(["routen", 0, "zone"], "z1")),
    ungueltig_vorgabe("u-kunde-form", "Ein Kundenschlüssel hat nicht die Schlüsselform.",
                      setze(["kunden", 0, "schluessel"], "k nord")),
    ungueltig_vorgabe("u-kunde-doppelt", "Ein Kundenschlüssel steht doppelt.",
                      setze(["kunden"], [kunde("k-nord", erlaubt=["lokal"]), kunde("k-nord")])),
    ungueltig_vorgabe("u-erlaubt-unbekannt", "erlaubt nennt eine Route, die es nicht gibt.",
                      setze(["kunden", 0, "erlaubt"], ["lokal", "lokal-x"])),
    ungueltig_vorgabe("u-verboten-unbekannt", "verboten nennt eine Route, die es nicht gibt.",
                      setze(["kunden", 0, "verboten"], ["cloud-x"])),
    ungueltig_vorgabe("u-cloud-immer", "vorgabe in kundendaten_an_cloud ist nicht nie.",
                      setze(["kundendaten_an_cloud", "vorgabe"], "immer")),
    ungueltig_vorgabe("u-freigabe-lokal", "Eine Freigabe nennt eine Route der Art lokal.",
                      setze(["kundendaten_an_cloud", "freigaben"], [{"route": "lokal", "kunde": "k-nord"}])),
    ungueltig_vorgabe("u-freigabe-route-unbekannt", "Eine Freigabe nennt eine unbekannte Route.",
                      setze(["kundendaten_an_cloud", "freigaben"], [{"route": "cloud-x", "kunde": "k-nord"}])),
    ungueltig_vorgabe("u-freigabe-route-liste", "Nachtrag 20.11., 3: Die Route einer Freigabe ist eine Liste statt Text.",
                      setze(["kundendaten_an_cloud", "freigaben"], [{"route": ["cloud"], "kunde": "k-nord"}])),
    ungueltig_vorgabe("u-freigabe-kunde-unbekannt", "Eine Freigabe nennt einen unbekannten Kunden.",
                      setze(["kundendaten_an_cloud", "freigaben"], [{"route": "cloud", "kunde": "k-sued"}])),
    ungueltig_vorgabe("u-vorgabestufe-unbekannt", "vorgabestufe steht nicht in klassen.",
                      setze(["vorgabestufe"], "geheim")),
]


# ---------------------------------------------------------------- reference (only to check the tables above)

def gueltig(v):
    """True if the vorgabe is valid by rust/spec/VERTRAEGE.md, section 2."""
    def ist_schl(x):
        return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None

    def einmalig(xs):
        return len(xs) == len(set(xs))

    if not isinstance(v, dict) or not set(v) <= {"klassen", "vorgabestufe", "routen", "kunden", "kundendaten_an_cloud"}:
        return False
    if not {"klassen", "routen", "kunden"} <= set(v):
        return False
    kl = v["klassen"]
    if not isinstance(kl, list) or not kl or not all(ist_schl(k) for k in kl) or not einmalig(kl):
        return False
    if "vorgabestufe" in v and v["vorgabestufe"] not in kl:
        return False
    routen = v["routen"]
    if not isinstance(routen, list):
        return False
    for r in routen:
        if not isinstance(r, dict) or not set(r) <= {"schluessel", "art", "klasse"} or not {"schluessel", "art"} <= set(r):
            return False
        if not ist_schl(r["schluessel"]) or r["art"] not in ("cloud", "lokal"):
            return False
        if r.get("klasse") is not None and r["klasse"] not in kl:
            return False
    rn = [r["schluessel"] for r in routen]
    if not einmalig(rn):
        return False
    art = {r["schluessel"]: r["art"] for r in routen}
    kunden = v["kunden"]
    if not isinstance(kunden, list):
        return False
    for k in kunden:
        if not isinstance(k, dict) or not set(k) <= {"schluessel", "erlaubt", "verboten"} or "schluessel" not in k:
            return False
        if not ist_schl(k["schluessel"]):
            return False
        for feld in ("erlaubt", "verboten"):
            liste = k.get(feld, [])
            if not isinstance(liste, list) or not all(isinstance(x, str) and x in art for x in liste):
                return False
    kn = [k["schluessel"] for k in kunden]
    if not einmalig(kn):
        return False
    kc = v.get("kundendaten_an_cloud", {"vorgabe": "nie"})
    if not isinstance(kc, dict) or not set(kc) <= {"vorgabe", "freigaben"} or kc.get("vorgabe") != "nie":
        return False
    fr = kc.get("freigaben", [])
    if not isinstance(fr, list):
        return False
    for f in fr:
        if not isinstance(f, dict) or set(f) != {"route", "kunde"}:
            return False
        # Nachtrag 20.11., 3: a route that is not a text is invalid (a list is not hashable and must not raise)
        if not isinstance(f["route"], str) or art.get(f["route"]) != "cloud" or f["kunde"] not in kn:
            return False
    return True


def referenz(v, a):
    """(reason, effective class) by rust/spec/VERTRAEGE.md, section 4."""
    kl = v["klassen"]
    top = kl[-1]
    stufe = v.get("vorgabestufe", top)
    routen = {r["schluessel"]: r for r in v["routen"]}
    kunden = {k["schluessel"]: k for k in v["kunden"]}
    frei = {(f["route"], f["kunde"]) for f in v.get("kundendaten_an_cloud", {}).get("freigaben", [])}
    roh = a["einheit"].get("klasse")
    r_ = a.get("route")
    k_ = a.get("kunde")
    if roh is None:
        eigene = stufe
    elif isinstance(roh, str) and roh in kl:
        eigene = roh
    else:
        eigene = None
    wirksam = None if eigene is None else (top if k_ is not None else eigene)
    if not r_:
        return "route-fehlt", wirksam
    r = routen.get(r_)
    if r is None or r.get("klasse") is None:
        return "route-ohne-klasse", wirksam
    if eigene is None:
        return "einheit-ohne-klasse", wirksam
    if kl.index(wirksam) > kl.index(r["klasse"]):
        return "klasse-ueber-route", wirksam
    if k_ is not None or wirksam == top:
        if k_ is None:
            return "kunde-fehlt", wirksam
        k = kunden.get(k_)
        if k is None:
            return "kunde-ohne-eintrag", wirksam
        if r_ in k.get("verboten", []):
            return "route-fuer-kunde-verboten", wirksam
        if r_ not in k.get("erlaubt", []):
            return "route-fuer-kunde-nicht-erlaubt", wirksam
        if r["art"] == "cloud" and (r_, k_) not in frei:
            return "kundendaten-an-cloud", wirksam
    return "frei", wirksam


def antwort(grund, klasse, route_):
    text = MELDUNG[grund]
    if "{route}" in text:
        text = text.replace("{route}", route_)
    return {"darf": grund == "frei", "grund": grund, "klasse": klasse, "meldung": text}


def tiefe(x):
    if isinstance(x, dict):
        return 1 + max((tiefe(y) for y in x.values()), default=0)
    if isinstance(x, list):
        return 1 + max((tiefe(y) for y in x), default=0)
    return 0


def main():
    dateien = {}
    namen = set()
    for name, regeln, beschreibung, v, paare in FAELLE:
        assert name not in namen, name
        namen.add(name)
        assert gueltig(v), f"{name}: Vorgabe ungueltig"
        anfragen, antworten = [], []
        for a, (grund, klasse) in paare:
            ref = referenz(v, a)
            assert ref == (grund, klasse), f"{name}: {a} erwartet {(grund, klasse)}, Referenz {ref}"
            anfragen.append(a)
            antworten.append(antwort(grund, klasse, a.get("route")))
        fall = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v, "anfragen": anfragen}
        if name in TIEFE:
            fall["beschreibung"] = tief(TIEFE[name] - 2)
            assert tiefe(fall) == TIEFE[name], (name, tiefe(fall))
        dateien[name] = (json.dumps(fall, ensure_ascii=False, indent=1).encode() + b"\n",
                         {"exit": 0, "ausgabe": {"antworten": antworten}})
    # Nachtrag 20.11., 3: each new invalid case has a valid twin that differs only in the changed value
    zwilling = json.loads(json.dumps(V_VORFUEHRUNG))
    zwilling["kundendaten_an_cloud"] = {"vorgabe": "nie", "freigaben": [{"route": "cloud", "kunde": "k-nord"}]}
    assert gueltig(zwilling), "Zwilling von u-freigabe-route-liste ungueltig"
    for name, regeln, beschreibung, fall in UNGUELTIG:
        assert name not in namen, name
        namen.add(name)
        if isinstance(fall, bytes):
            roh = fall
        else:
            v = fall.get("vorgabe")
            erwartet_ungueltig = (v is None or not gueltig(v) or set(fall) - {"name", "regeln", "beschreibung", "vorgabe", "anfragen"}
                                  or not isinstance(fall.get("anfragen"), list) or tiefe(fall) > 64
                                  or any(not isinstance(a, dict) or set(a) - {"einheit", "route", "kunde"}
                                         or not isinstance(a.get("einheit"), dict) or set(a["einheit"]) - {"klasse"}
                                         or not isinstance(a.get("route"), (str, type(None)))
                                         or not isinstance(a.get("kunde"), (str, type(None)))
                                         for a in fall["anfragen"]))
            assert erwartet_ungueltig, f"{name}: Fall waere gueltig"
            roh = json.dumps(fall, ensure_ascii=False).encode() + b"\n"
        dateien[name] = (roh, {"exit": 2, "regeln": regeln, "beschreibung": beschreibung})
    if ZIEL.exists():
        shutil.rmtree(ZIEL)
    (ZIEL / "faelle").mkdir(parents=True)
    (ZIEL / "erwartet").mkdir(parents=True)
    for name, (roh, erw) in sorted(dateien.items()):
        (ZIEL / "faelle" / f"{name}.json").write_bytes(roh)
        (ZIEL / "erwartet" / f"{name}.json").write_text(json.dumps(erw, ensure_ascii=False, indent=1) + "\n",
                                                       encoding="utf-8")
    gueltige = sum(1 for _, (_, e) in dateien.items() if e["exit"] == 0)
    anfragen = sum(len(p) for *_, p in FAELLE)
    print(f"{len(dateien)} Faelle ({gueltige} gueltig mit {anfragen} Anfragen, {len(dateien) - gueltige} ungueltig)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
