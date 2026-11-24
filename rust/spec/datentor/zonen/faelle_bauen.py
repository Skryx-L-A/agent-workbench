"""Builds the visible cases of the part contract `datentor zonen` (zonen/VERTRAG.md).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json (the input on stdin of `datentor zonen`) and sichtbar/erwartet/<name>.json (exit code
and expected output). Every expectation is written out by hand in the tables below. The script checks each one against a
small reference of the rules in zonen/VERTRAG.md, section 5, and each case against the validity rules of section 3; a
mismatch stops the script before anything is written. The vorgabe part and the rules with the option off come from the
reference of contract 3 (../faelle_bauen.py), which is loaded, not copied. All data is invented; the zones of the
template appear only as test data.
"""
import importlib.util
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "sichtbar"
SCHLUESSEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
U64 = 2 ** 64

_spec = importlib.util.spec_from_file_location("datentor_vertrag3", HIER.parent / "faelle_bauen.py")
DT3 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(DT3)

MELDUNG = dict(DT3.MELDUNG, **{
    "nutzer-fehlt": "Anfrage ohne Nutzer",
    "route-ohne-zone": "Route {route} hat keine Zone",
    "zone-durch-regel-gesperrt": "Zone der Route {route} ist durch eine Regel gesperrt",
    "zone-gesperrt": "Zone der Route {route} ist gesperrt",
    "grund-fehlt": "Route {route} braucht einen Grund für die nachrangige Zone",
    "klasse-ueber-zone": "Klasse der Einheit liegt über der Zone der Route {route}",
    "zone-ohne-freigabe": "Kundendaten an Route {route} ohne Freigabe für Kunde und Projekt",
})
KLASSENSAETZE = ("alle", "niedrigste", "keine")
BEDINGUNGEN = ("immer", "nachrangig", "freigabe", "gesperrt")

KL3 = DT3.KL3
route = DT3.route
kunde = DT3.kunde
freigaben = DT3.freigaben
FEHLT = DT3.FEHLT
LAUF = object()  # einheit null: the run check without a unit

T = 1800000000  # the deadline of the evaluation exception in the cases (seconds since 1970-01-01 UTC)
KX = {"nutzer": "n-a", "gruppen": ["g-team"], "jetzt": T - 100}

ROUTEN_Z = [route("r-local", "lokal"), route("r-local2", "lokal", "oeffentlich"), route("r-conf", "cloud"),
            route("r-appr", "cloud"), route("r-public", "cloud"), route("r-public-kd", "cloud", "kundendaten"),
            route("r-unsafe", "cloud"), route("r-ohne-zone", "cloud", "intern")]
ALLE_ROUTEN = [r["schluessel"] for r in ROUTEN_Z]
KUNDEN_Z = [kunde("k", erlaubt=ALLE_ROUTEN), kunde("k2", erlaubt=["r-appr", "r-local"]),
            kunde("k-v", erlaubt=["r-appr"], verboten=["r-appr"])]
V_Z = {"klassen": KL3, "routen": ROUTEN_Z, "kunden": KUNDEN_Z}
V_Z_A50 = dict(V_Z, kundendaten_an_cloud=freigaben(("r-conf", "k"), ("r-appr", "k")))


def zone(schluessel, klassen, bedingung, routen=None):
    z = {"schluessel": schluessel, "klassen": klassen, "bedingung": bedingung}
    if routen is not None:
        z["routen"] = routen
    return z


# Test data after the template: five zones in order of precedence, one route each (two in local and public).
ZONEN_VORLAGE = [
    zone("local", "alle", "immer", ["r-local", "r-local2"]),
    zone("conf", "alle", "nachrangig", ["r-conf"]),
    zone("appr", "alle", "freigabe", ["r-appr"]),
    zone("public", "niedrigste", "immer", ["r-public", "r-public-kd"]),
    zone("unsafe", "keine", "gesperrt", ["r-unsafe"]),
]
AUSN_N = {"zone": "unsafe", "nutzer": ["n-a"], "gruppen": [], "gesetzt_von": "admin-1", "zweck": "Evaluierung Modell X",
          "bis": T}
AUSN_G = {"zone": "unsafe", "nutzer": [], "gruppen": ["g-eval"], "gesetzt_von": "admin-2", "zweck": "Evaluierung Gruppe",
          "bis": T + 1000}


def mz(an=True, zonen=None, **rest):
    m = {"an": an, "zonen": ZONEN_VORLAGE if zonen is None else zonen}
    m.update(rest)
    return m


MZ = mz()
MZ_FREI = mz(freigaben=[{"route": "r-appr", "kunde": "k", "projekt": "p1"},
                        {"route": "r-appr", "kunde": "k-v", "projekt": "p1"}])
MZ_AUSN = mz(ausnahmen=[AUSN_N, {"zone": "unsafe", "gruppen": ["g-eval"], "gesetzt_von": "admin-2",
                                 "zweck": "Evaluierung Gruppe", "bis": T + 1000}])
MZ_REGELN = mz(regeln=[
    {"ebene": "nutzer", "wer": "n-a", "zone": "conf", "gesperrt": True},
    {"ebene": "gruppe", "wer": "g-streng", "zone": "public", "klassen": "keine"},
    {"ebene": "gruppe", "wer": "g-team", "zone": "local", "klassen": "alle"},
    {"ebene": "gruppe", "wer": "g-intern", "zone": "local", "klassen": "niedrigste"},
    {"ebene": "nutzer", "wer": "n-a", "zone": "unsafe", "gesperrt": True},
], ausnahmen=[AUSN_N])
MZ_AUS = mz(an=False)


def za(klasse, route_, kunde_=None, projekt=None, kontext=KX):
    """One request of the zone format. LAUF: einheit null; FEHLT leaves a field out; kontext=FEHLT leaves it out."""
    if klasse is LAUF:
        a = {"einheit": None}
    else:
        a = {"einheit": {} if klasse is FEHLT else {"klasse": klasse}}
    for feld, wert in (("route", route_), ("kunde", kunde_), ("projekt", projekt), ("kontext", kontext)):
        if wert is not FEHLT:
            a[feld] = wert
    return a


def kx(**aenderung):
    k = dict(KX)
    for feld, wert in aenderung.items():
        if wert is FEHLT:
            k.pop(feld, None)
        else:
            k[feld] = wert
    return k


GRUND = "local reicht fuer die Aufgabe nicht"
AUS = "-"  # expected protokoll null (option off)


# (name, rules, description, vorgabe, modellzonen,
#  [(request, (reason, effective class, zone in protokoll or AUS) [, {"grund": ..., "ausnahme": ...}])])
FAELLE = [
    ("z1-jede-route", ["A128-1", "Z6", "Z4", "Z5", "Z7"],
     "A128 (1): dieselben drei Einheiten über jede Zone. local bekommt alle, public nur die niedrigste Klasse, unsafe nichts.",
     V_Z, MZ, [
         (za("oeffentlich", "r-local"), ("frei", "oeffentlich", "local")),
         (za("intern", "r-local"), ("frei", "intern", "local")),
         (za("kundendaten", "r-local", "k", "p1"), ("frei", "kundendaten", "local")),
         (za("oeffentlich", "r-public"), ("frei", "oeffentlich", "public")),
         (za("intern", "r-public"), ("klasse-ueber-zone", "intern", "public")),
         (za("kundendaten", "r-public", "k", "p1"), ("klasse-ueber-zone", "kundendaten", "public")),
         (za("oeffentlich", "r-unsafe"), ("zone-gesperrt", "oeffentlich", "unsafe")),
         (za("intern", "r-unsafe"), ("zone-gesperrt", "intern", "unsafe")),
         (za("kundendaten", "r-unsafe", "k", "p1"), ("zone-gesperrt", "kundendaten", "unsafe")),
         (za("oeffentlich", "r-conf", kontext=kx(grund=GRUND)), ("frei", "oeffentlich", "conf"), {"grund": GRUND}),
         (za("intern", "r-conf", kontext=kx(grund=GRUND)), ("frei", "intern", "conf"), {"grund": GRUND}),
         (za("kundendaten", "r-conf", "k", "p1", kontext=kx(grund=GRUND)),
          ("kundendaten-an-cloud", "kundendaten", "conf"), {"grund": GRUND}),
         (za("oeffentlich", "r-appr"), ("frei", "oeffentlich", "appr")),
         (za("intern", "r-appr"), ("frei", "intern", "appr")),
         (za("kundendaten", "r-appr", "k", "p1"), ("zone-ohne-freigabe", "kundendaten", "appr")),
     ]),
    ("z2-nachrangig-grund", ["A128-2", "Z5", "Z8"],
     "A128 (2): conf ohne Grund abgelehnt, mit Grund frei; der Grund steht im Protokoll. Leerraum ist kein Grund.",
     V_Z, MZ, [
         (za("intern", "r-conf"), ("grund-fehlt", "intern", "conf")),
         (za(LAUF, "r-conf"), ("grund-fehlt", None, "conf")),
         (za("intern", "r-conf", kontext=kx(grund=None)), ("grund-fehlt", "intern", "conf")),
         (za("intern", "r-conf", kontext=kx(grund="")), ("grund-fehlt", "intern", "conf")),
         (za("intern", "r-conf", kontext=kx(grund=" \t\n\r")), ("grund-fehlt", "intern", "conf")),
         (za("intern", "r-conf", kontext=kx(grund=GRUND)), ("frei", "intern", "conf"), {"grund": GRUND}),
         (za(LAUF, "r-conf", kontext=kx(grund=GRUND)), ("frei", None, "conf"), {"grund": GRUND}),
         (za("intern", "r-conf", kontext=kx(grund=" x ")), ("frei", "intern", "conf"), {"grund": " x "}),
         (za("intern", "r-local", kontext=kx(grund=GRUND)), ("frei", "intern", "local")),
         (za("geheim", "r-conf", kontext=kx(grund=GRUND)), ("einheit-ohne-klasse", None, "conf"), {"grund": GRUND}),
     ]),
    ("z3-freigabe-vorher", ["A128-3", "Z7"],
     "A128 (3) vor der Freigabe: über appr keine Kundendaten, auch nicht mit der Freigabe aus A50 ohne Projekt; intern geht.",
     V_Z_A50, MZ, [
         (za("kundendaten", "r-appr", "k", "p1"), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("intern", "r-appr", "k", "p1"), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("intern", "r-appr"), ("frei", "intern", "appr")),
         (za(LAUF, "r-appr", "k", "p1"), ("frei", None, "appr")),
         (za("kundendaten", "r-conf", "k", "p1", kontext=kx(grund=GRUND)), ("frei", "kundendaten", "conf"),
          {"grund": GRUND}),
     ]),
    ("z3-freigabe-nachher", ["A128-3", "Z7", "D9", "D11"],
     "A128 (3) nach der Freigabe für k und p1: k mit p1 frei, mit p2 oder ohne Projekt nicht; k2 nicht; verboten geht vor.",
     V_Z, MZ_FREI, [
         (za("kundendaten", "r-appr", "k", "p1"), ("frei", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k", "p2"), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k", None), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k", FEHLT), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k", "P1"), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k2", "p1"), ("zone-ohne-freigabe", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", "k-v", "p1"), ("route-fuer-kunde-verboten", "kundendaten", "appr")),
         (za("kundendaten", "r-appr", None, "p1"), ("kunde-fehlt", "kundendaten", "appr")),
         (za("kundendaten", "r-conf", "k", "p1", kontext=kx(grund=GRUND)),
          ("kundendaten-an-cloud", "kundendaten", "conf"), {"grund": GRUND}),
     ]),
    ("z4-ohne-ausnahme", ["A128-4", "Z4"], "A128 (4): unsafe ohne Ausnahme ist gesperrt, auch der Lauf ohne Einheit.",
     V_Z, MZ, [
         (za(LAUF, "r-unsafe"), ("zone-gesperrt", None, "unsafe")),
         (za("oeffentlich", "r-unsafe"), ("zone-gesperrt", "oeffentlich", "unsafe")),
     ]),
    ("z4-ausnahme", ["A128-4", "Z4", "Z6", "Z8"],
     "A128 (4): Ausnahme für n-a bis T. n-a läuft vor T mit 0 Einheiten; n-m, n-a ab T und ohne Zeit gesperrt; Gruppe g-eval.",
     V_Z, MZ_AUSN, [
         (za(LAUF, "r-unsafe"), ("frei", None, "unsafe"), {"ausnahme": AUSN_N}),
         (za("oeffentlich", "r-unsafe"), ("klasse-ueber-zone", "oeffentlich", "unsafe"), {"ausnahme": AUSN_N}),
         (za("kundendaten", "r-unsafe", "k", "p1"), ("klasse-ueber-zone", "kundendaten", "unsafe"), {"ausnahme": AUSN_N}),
         (za(LAUF, "r-unsafe", kontext=kx(jetzt=T - 1)), ("frei", None, "unsafe"), {"ausnahme": AUSN_N}),
         (za(LAUF, "r-unsafe", kontext=kx(nutzer="n-m")), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(jetzt=T)), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(jetzt=T + 1)), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(jetzt=None)), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(jetzt=FEHLT)), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(nutzer="N-A")), ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(nutzer="n-m", gruppen=["g-x", "g-eval"], jetzt=T + 999)),
          ("frei", None, "unsafe"), {"ausnahme": AUSN_G}),
         (za(LAUF, "r-unsafe", kontext=kx(nutzer="n-m", gruppen=["g-eval"], jetzt=T + 1000)),
          ("zone-gesperrt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext=kx(gruppen=["g-eval"], jetzt=T - 1)), ("frei", None, "unsafe"),
          {"ausnahme": AUSN_N}),
         (za(LAUF, "r-local"), ("frei", None, "local")),
     ]),
    ("z4-ausnahme-ohne-nutzer", ["A128-4", "Z1", "Z8"],
     "Nachtrag 20.11., 2: Ohne Nutzer gilt Z1 (fail closed); passt die Ausnahme über die Gruppe, nennt das Protokoll sie trotzdem.",
     V_Z, MZ_AUSN, [
         (za(LAUF, "r-unsafe", kontext={"gruppen": ["g-eval"], "jetzt": T}), ("nutzer-fehlt", None, "unsafe"),
          {"ausnahme": AUSN_G}),
         (za(LAUF, "r-unsafe", kontext={"nutzer": "", "gruppen": ["g-eval"], "jetzt": T}), ("nutzer-fehlt", None, "unsafe"),
          {"ausnahme": AUSN_G}),
         (za("oeffentlich", "r-unsafe", kontext={"nutzer": None, "gruppen": ["g-eval"], "jetzt": T}),
          ("nutzer-fehlt", "oeffentlich", "unsafe"), {"ausnahme": AUSN_G}),
         (za(LAUF, "r-unsafe", kontext={"nutzer": None, "gruppen": ["g-eval"], "jetzt": T + 1000}),
          ("nutzer-fehlt", None, "unsafe")),
         (za(LAUF, "r-unsafe", kontext={"nutzer": None, "gruppen": [], "jetzt": T}), ("nutzer-fehlt", None, "unsafe")),
     ]),
    ("z5-regeln-verschaerfen", ["A128-5", "Z3", "Z6"],
     "A128 (5): Die Nutzerregel sperrt conf für n-a, nicht für n-m. Gruppenregeln senken die Klassen; gleich ist keine Lockerung.",
     V_Z, MZ_REGELN, [
         (za("intern", "r-conf", kontext=kx(grund=GRUND)), ("zone-durch-regel-gesperrt", "intern", "conf"),
          {"grund": GRUND}),
         (za(LAUF, "r-conf", kontext=kx(grund=GRUND)), ("zone-durch-regel-gesperrt", None, "conf"), {"grund": GRUND}),
         (za("intern", "r-conf", kontext=kx(nutzer="n-m", grund=GRUND)), ("frei", "intern", "conf"), {"grund": GRUND}),
         (za("oeffentlich", "r-public", kontext=kx(gruppen=["g-streng"])), ("klasse-ueber-zone", "oeffentlich", "public")),
         (za(LAUF, "r-public", kontext=kx(gruppen=["g-streng"])), ("frei", None, "public")),
         (za("oeffentlich", "r-public"), ("frei", "oeffentlich", "public")),
         (za("kundendaten", "r-local", "k", "p1"), ("frei", "kundendaten", "local")),
         (za("intern", "r-local", kontext=kx(nutzer="n-m", gruppen=["g-team", "g-intern"])),
          ("klasse-ueber-zone", "intern", "local")),
         (za("intern", "r-local", kontext=kx(nutzer="n-m", gruppen=[])), ("frei", "intern", "local")),
         (za(LAUF, "r-unsafe"), ("zone-durch-regel-gesperrt", None, "unsafe"), {"ausnahme": AUSN_N}),
         (za(LAUF, "r-unsafe", kontext=kx(nutzer="n-m")), ("zone-gesperrt", None, "unsafe")),
     ]),
    ("z6-eigene-klasse", ["A128-6", "D6", "Z6"],
     "A128 (6): Es gilt der strengere Wert von Zone und eigener Klasse der Route, in beide Richtungen.",
     V_Z, MZ, [
         (za("oeffentlich", "r-local2"), ("frei", "oeffentlich", "local")),
         (za("intern", "r-local2"), ("klasse-ueber-route", "intern", "local")),
         (za("kundendaten", "r-local2", "k", "p1"), ("klasse-ueber-route", "kundendaten", "local")),
         (za("intern", "r-public-kd"), ("klasse-ueber-zone", "intern", "public")),
         (za("oeffentlich", "r-public-kd"), ("frei", "oeffentlich", "public")),
         (za("intern", "r-local"), ("frei", "intern", "local")),
     ]),
    ("z7-route-ohne-zone", ["A127", "Z2", "Z1"],
     "A127: Option an. r-local läuft; r-ohne-zone hat eine Klasse, aber keine Zone, und ist gesperrt; ebenso eine unbekannte Route.",
     V_Z, MZ, [
         (za("intern", "r-local"), ("frei", "intern", "local")),
         (za("intern", "r-ohne-zone"), ("route-ohne-zone", "intern", None)),
         (za(LAUF, "r-ohne-zone"), ("route-ohne-zone", None, None)),
         (za("oeffentlich", "r-x"), ("route-ohne-zone", "oeffentlich", None)),
         (za("intern", "R-LOCAL"), ("route-ohne-zone", "intern", None)),
         (za("intern", "r-local", kontext=FEHLT), ("nutzer-fehlt", "intern", "local")),
     ]),
    ("z7-gegenprobe-aus", ["A127", "Z0"],
     "A127, Gegenprobe: Option aus. Dieselben Läufe gehen nach Vertrag 3; die Zonen und der Kontext zählen nicht.",
     V_Z, MZ_AUS, [
         (za("intern", "r-ohne-zone"), ("frei", "intern", AUS)),
         (za(LAUF, "r-ohne-zone"), ("frei", None, AUS)),
         (za("intern", "r-local"), ("route-ohne-klasse", "intern", AUS)),
         (za(LAUF, "r-local"), ("route-ohne-klasse", None, AUS)),
         (za("oeffentlich", "r-unsafe"), ("route-ohne-klasse", "oeffentlich", AUS)),
         (za("intern", "r-local2", kontext=FEHLT), ("klasse-ueber-route", "intern", AUS)),
         (za("kundendaten", "r-public-kd", "k", "p1"), ("kundendaten-an-cloud", "kundendaten", AUS)),
         (za(LAUF, None), ("route-fehlt", None, AUS)),
         (za(LAUF, "r-x"), ("route-ohne-klasse", None, AUS)),
         (za("intern", "r-x"), ("route-ohne-klasse", "intern", AUS)),
     ]),
    ("z8-reihenfolge", ["Z9", "Z1"], "Treffen mehrere Gründe zu, gilt der erste in der Reihenfolge der Tabelle.",
     V_Z, MZ_REGELN, [
         (za("geheim", "", kontext=FEHLT), ("route-fehlt", None, None)),
         (za("geheim", "r-x", kontext=kx(nutzer="")), ("nutzer-fehlt", None, None)),
         (za("geheim", "r-x"), ("route-ohne-zone", None, None)),
         (za("geheim", "r-unsafe"), ("einheit-ohne-klasse", None, "unsafe"), {"ausnahme": AUSN_N}),
         (za("kundendaten", "r-conf", "k"), ("zone-durch-regel-gesperrt", "kundendaten", "conf")),
         (za("kundendaten", "r-unsafe", "k", kontext=kx(nutzer="n-m")), ("zone-gesperrt", "kundendaten", "unsafe")),
         (za("kundendaten", "r-conf", "k-x", kontext=kx(nutzer="n-m")), ("grund-fehlt", "kundendaten", "conf")),
         (za("kundendaten", "r-public-kd", "k-x"), ("klasse-ueber-zone", "kundendaten", "public")),
         (za("kundendaten", "r-local2", "k-x"), ("klasse-ueber-route", "kundendaten", "local")),
         (za("kundendaten", "r-appr", "k-x"), ("kunde-ohne-eintrag", "kundendaten", "appr")),
         (za(FEHLT, "r-local"), ("kunde-fehlt", "kundendaten", "local")),
         (za("intern", "r-local", ""), ("kunde-ohne-eintrag", "kundendaten", "local")),
     ]),
    ("z8-vier-klassen", ["Z6", "D5"], "Das Tor kennt keinen Klassennamen: niedrigste ist die erste Klasse, alle die letzte.",
     {"klassen": ["s0", "s1", "s2", "s3"], "routen": [route("a", "lokal"), route("b", "lokal")],
      "kunden": [kunde("k", erlaubt=["a", "b"])]},
     mz(zonen=[zone("z-alle", "alle", "immer", ["a"]), zone("z-unten", "niedrigste", "immer", ["b"])]), [
         (za("s3", "a", "k"), ("frei", "s3", "z-alle")),
         (za("s0", "b"), ("frei", "s0", "z-unten")),
         (za("s1", "b"), ("klasse-ueber-zone", "s1", "z-unten")),
         (za("s0", "b", "k"), ("klasse-ueber-zone", "s3", "z-unten")),
     ]),
    ("z8-zone-ohne-routen", ["Z2"], "Eine Zone ohne Feld routen ist gültig und gibt keiner Route eine Zone.",
     V_Z, mz(zonen=[zone("leer", "alle", "immer"), zone("local", "alle", "immer", ["r-local"])]), [
         (za("intern", "r-local"), ("frei", "intern", "local")),
         (za("intern", "r-conf"), ("route-ohne-zone", "intern", None)),
     ]),
]


def ungueltig(name, beschreibung, aendern, v=V_Z, m=MZ_AUSN, anfragen=None):
    fall = json.loads(json.dumps({"vorgabe": v, "modellzonen": m,
                                  "anfragen": anfragen or [za("intern", "r-local")]}))
    aendern(fall)
    return (name, ["Z10"], beschreibung, fall)


setze = DT3.setze
loesche = DT3.loesche


def anfrage(**felder):
    """Sets fields of the first request; FEHLT removes one."""
    def f(fall):
        a = fall["anfragen"][0]
        for k, w in felder.items():
            if w is FEHLT:
                a.pop(k, None)
            else:
                a[k] = w
    return f


ZR = ["modellzonen", "zonen"]
AR = ["modellzonen", "ausnahmen"]
RR = ["modellzonen", "regeln"]
FR = ["modellzonen", "freigaben"]
VR = {"route": "r-appr", "kunde": "k", "projekt": "p1"}
REGEL = {"ebene": "nutzer", "wer": "n-a", "zone": "conf", "gesperrt": True}

# (name, rules, description, case as object, or bytes for a raw input)
UNGUELTIG = [
    ungueltig("u-ohne-modellzonen", "Das Feld modellzonen fehlt.", loesche(["modellzonen"])),
    ungueltig("u-modellzonen-liste", "modellzonen ist kein Objekt.", setze(["modellzonen"], [])),
    ungueltig("u-ohne-an", "Das Pflichtfeld an fehlt; die Vorgabe aus setzt der Lader von Vertrag 2 ein.",
              loesche(["modellzonen", "an"])),
    ungueltig("u-an-zahl", "an ist kein Wahrheitswert.", setze(["modellzonen", "an"], 0)),
    ungueltig("u-an-text", "an ist kein Wahrheitswert.", setze(["modellzonen", "an"], "aus")),
    ungueltig("u-ohne-zonen", "Das Pflichtfeld zonen fehlt.", loesche(ZR)),
    ungueltig("u-modellzonen-feld", "Ein unbekanntes Feld in modellzonen.", setze(["modellzonen", "farbe"], "rot")),
    ungueltig("u-aus-und-kaputt", "Auch mit an false macht eine fehlerhafte Zone den Fall ungültig.",
              lambda f: (setze(["modellzonen", "an"], False)(f), setze(ZR + [0, "bedingung"], "manchmal")(f))),
    ungueltig("u-zone-form", "Ein Zonenschlüssel hat nicht die Schlüsselform.", setze(ZR + [0, "schluessel"], "LOCAL")),
    ungueltig("u-zone-doppelt", "Ein Zonenschlüssel steht doppelt.", setze(ZR + [1, "schluessel"], "local")),
    ungueltig("u-zone-feld", "Ein unbekanntes Feld an einer Zone (Anzeigedaten gehören nicht zur Sicht des Tors).",
              setze(ZR + [0, "praefix"], "LOCAL")),
    ungueltig("u-zone-ohne-klassen", "Das Pflichtfeld klassen einer Zone fehlt.", loesche(ZR + [0, "klassen"])),
    ungueltig("u-zone-ohne-bedingung", "Das Pflichtfeld bedingung einer Zone fehlt.", loesche(ZR + [0, "bedingung"])),
    ungueltig("u-zone-klassen-name", "klassen nennt einen Klassennamen statt alle, niedrigste oder keine.",
              setze(ZR + [3, "klassen"], "oeffentlich")),
    ungueltig("u-zone-bedingung", "bedingung ist kein bekannter Wert.", setze(ZR + [0, "bedingung"], "manchmal")),
    ungueltig("u-zone-route-unbekannt", "Eine Zone nennt eine Route, die es nicht gibt.",
              setze(ZR + [0, "routen"], ["r-local", "r-x"])),
    ungueltig("u-route-zwei-zonen", "Eine Route steht in zwei Zonen.", setze(ZR + [3, "routen"], ["r-public", "r-local"])),
    ungueltig("u-route-doppelt-in-zone", "Eine Route steht zweimal in derselben Zone.",
              setze(ZR + [1, "routen"], ["r-conf", "r-conf"])),
    ungueltig("u-routen-text", "routen einer Zone ist keine Liste.", setze(ZR + [1, "routen"], "r-conf")),
    ungueltig("u-nachrangig-zuerst", "Die erste Zone hat die Bedingung nachrangig; es gibt keine vorrangige Zone.",
              setze(ZR + [0, "bedingung"], "nachrangig")),
    ungueltig("u-freigabe-route-unbekannt", "Eine Projektfreigabe nennt eine unbekannte Route.",
              setze(FR, [dict(VR, route="r-x")])),
    ungueltig("u-freigabe-zone-falsch", "Eine Projektfreigabe nennt eine Route, deren Zone nicht die Bedingung freigabe hat.",
              setze(FR, [dict(VR, route="r-conf")])),
    ungueltig("u-freigabe-ohne-zone", "Eine Projektfreigabe nennt eine Route ohne Zone.",
              setze(FR, [dict(VR, route="r-ohne-zone")])),
    ungueltig("u-freigabe-kunde-unbekannt", "Eine Projektfreigabe nennt einen unbekannten Kunden.",
              setze(FR, [dict(VR, kunde="k-x")])),
    ungueltig("u-freigabe-kunde-liste", "Nachtrag 20.11., 2: Der Kunde einer Projektfreigabe ist eine Liste statt Text.",
              setze(FR, [dict(VR, kunde=["k"])])),
    ungueltig("u-freigabe-projekt-liste", "Nachtrag 20.11., 2: Das Projekt einer Projektfreigabe ist eine Liste statt Text.",
              setze(FR, [dict(VR, projekt=["p1"])])),
    ungueltig("u-freigabe-route-liste", "Nachtrag 20.11., 2: Die Route einer Projektfreigabe ist eine Liste statt Text.",
              setze(FR, [dict(VR, route=["r-appr"])])),
    ungueltig("u-freigabe-projekt-form", "Das Projekt einer Freigabe hat nicht die Schlüsselform.",
              setze(FR, [dict(VR, projekt="P1")])),
    ungueltig("u-freigabe-ohne-projekt", "Einer Projektfreigabe fehlt das Projekt.",
              setze(FR, [{"route": "r-appr", "kunde": "k"}])),
    ungueltig("u-freigabe-feld", "Ein unbekanntes Feld an einer Projektfreigabe.", setze(FR, [dict(VR, bis=1)])),
    ungueltig("u-ausnahme-zone-unbekannt", "Eine Ausnahme nennt eine unbekannte Zone.", setze(AR + [0, "zone"], "z-x")),
    ungueltig("u-ausnahme-zone-nicht-gesperrt", "Eine Ausnahme nennt eine Zone, die nicht gesperrt ist.",
              setze(AR + [0, "zone"], "public")),
    ungueltig("u-ausnahme-fuer-niemand", "Eine Ausnahme nennt weder Nutzer noch Gruppen.",
              lambda f: (setze(AR + [0, "nutzer"], [])(f), loesche(AR + [0, "gruppen"])(f))),
    ungueltig("u-ausnahme-ohne-frist", "Einer Ausnahme fehlt die Frist bis.", loesche(AR + [0, "bis"])),
    ungueltig("u-ausnahme-frist-text", "Die Frist ist keine ganze Zahl.", setze(AR + [0, "bis"], "2027-01-01T00:00:00Z")),
    ungueltig("u-ausnahme-frist-bruch", "Die Frist ist keine ganze Zahl.", setze(AR + [0, "bis"], 1.5)),
    ungueltig("u-ausnahme-frist-negativ", "Die Frist ist negativ.", setze(AR + [0, "bis"], -1)),
    ungueltig("u-ausnahme-frist-wahr", "Die Frist ist ein Wahrheitswert.", setze(AR + [0, "bis"], True)),
    ungueltig("u-ausnahme-frist-zu-gross", "Die Frist liegt über 2^64 - 1.", setze(AR + [0, "bis"], U64)),
    ungueltig("u-ausnahme-ohne-zweck", "Einer Ausnahme fehlt der Zweck.", loesche(AR + [0, "zweck"])),
    ungueltig("u-ausnahme-zweck-leer", "Der Zweck einer Ausnahme ist nur Leerraum.", setze(AR + [0, "zweck"], " \t")),
    ungueltig("u-ausnahme-ohne-admin", "Einer Ausnahme fehlt, wer sie gesetzt hat.", loesche(AR + [0, "gesetzt_von"])),
    ungueltig("u-ausnahme-admin-form", "gesetzt_von hat nicht die Schlüsselform.", setze(AR + [0, "gesetzt_von"], "Admin")),
    ungueltig("u-ausnahme-nutzer-form", "Ein Nutzer der Ausnahme hat nicht die Schlüsselform.",
              setze(AR + [0, "nutzer"], ["n a"])),
    ungueltig("u-ausnahme-feld", "Ein unbekanntes Feld an einer Ausnahme.", setze(AR + [0, "klassen"], "alle")),
    ungueltig("u-regel-lockert-gruppe", "A128 (5): Eine Gruppenregel öffnet public für alle Klassen.",
              setze(RR, [{"ebene": "gruppe", "wer": "g-team", "zone": "public", "klassen": "alle"}])),
    ungueltig("u-regel-lockert-nutzer", "Eine Nutzerregel öffnet unsafe für die niedrigste Klasse.",
              setze(RR, [{"ebene": "nutzer", "wer": "n-a", "zone": "unsafe", "klassen": "niedrigste"}])),
    ungueltig("u-regel-entsperrt", "Eine Regel mit gesperrt false hebt eine Sperre auf; nur true ist erlaubt.",
              setze(RR, [dict(REGEL, zone="unsafe", gesperrt=False)])),
    ungueltig("u-regel-bedingung", "Eine Regel ändert die Bedingung einer Zone; das Feld gibt es nicht.",
              setze(RR, [dict(REGEL, bedingung="immer")])),
    ungueltig("u-regel-leer", "Eine Regel ohne klassen und ohne gesperrt.",
              setze(RR, [{"ebene": "nutzer", "wer": "n-a", "zone": "conf"}])),
    ungueltig("u-regel-ebene", "Die Ebene einer Regel ist weder gruppe noch nutzer.", setze(RR, [dict(REGEL, ebene="firma")])),
    ungueltig("u-regel-zone-unbekannt", "Eine Regel nennt eine unbekannte Zone.", setze(RR, [dict(REGEL, zone="z-x")])),
    ungueltig("u-regel-wer-form", "wer hat nicht die Schlüsselform.", setze(RR, [dict(REGEL, wer="N-A")])),
    ungueltig("u-anfrage-feld", "Eine Anfrage trägt ein unbekanntes Feld.", anfrage(person="p")),
    ungueltig("u-anfrage-ohne-einheit", "Einer Anfrage fehlt einheit; null ist der Lauf ohne Einheit, Fehlen ist ungültig.",
              anfrage(einheit=FEHLT)),
    ungueltig("u-einheit-text", "einheit ist weder Objekt noch null.", anfrage(einheit="intern")),
    ungueltig("u-projekt-zahl", "projekt ist weder Text noch null.", anfrage(projekt=1)),
    ungueltig("u-kontext-liste", "kontext ist kein Objekt.", anfrage(kontext=[])),
    ungueltig("u-kontext-feld", "kontext trägt ein unbekanntes Feld.", anfrage(kontext=kx(rolle="admin"))),
    ungueltig("u-nutzer-zahl", "nutzer ist weder Text noch null.", anfrage(kontext=kx(nutzer=1))),
    ungueltig("u-gruppen-text", "gruppen ist keine Liste.", anfrage(kontext=kx(gruppen="g-team"))),
    ungueltig("u-gruppen-null", "gruppen ist null; die leere Liste heißt keine Gruppe.", anfrage(kontext=kx(gruppen=None))),
    ungueltig("u-gruppe-zahl", "Eine Gruppe ist kein Text.", anfrage(kontext=kx(gruppen=[1]))),
    ungueltig("u-jetzt-text", "jetzt ist keine ganze Zahl.", anfrage(kontext=kx(jetzt="1800000000"))),
    ungueltig("u-jetzt-bruch", "jetzt ist keine ganze Zahl.", anfrage(kontext=kx(jetzt=1.0))),
    ungueltig("u-jetzt-negativ", "jetzt ist negativ.", anfrage(kontext=kx(jetzt=-1))),
    ungueltig("u-jetzt-zu-gross", "jetzt liegt über 2^64 - 1.", anfrage(kontext=kx(jetzt=U64))),
    ungueltig("u-grund-zahl", "grund ist weder Text noch null.", anfrage(kontext=kx(grund=1))),
    ungueltig("u-vorgabe-ungueltig", "Die Vorgabe ist nach Vertrag 3 ungültig.", setze(["vorgabe", "routen", 0, "art"], "x")),
    ungueltig("u-feld-im-fall", "Ein unbekanntes Feld im Fall.", setze(["zusatz"], 1)),
    ("u-doppel-zone", ["Z10", "A139"], "Ein doppelter Schlüssel in einer Zone; der zweite Wert wäre lockerer.",
     b'{"vorgabe": {"klassen": ["a", "b"], "routen": [{"schluessel": "r", "art": "lokal"}], "kunden": []}, '
     b'"modellzonen": {"an": true, "zonen": [{"schluessel": "z", "klassen": "niedrigste", "klassen": "alle", '
     b'"bedingung": "immer", "routen": ["r"]}]}, "anfragen": [{"einheit": {"klasse": "b"}, "route": "r", '
     b'"kontext": {"nutzer": "n"}}]}'),
    ("u-doppel-an", ["Z10", "A139"], "Ein doppelter Schlüssel an in Escape-Schreibweise; der zweite Wert schaltet aus.",
     b'{"vorgabe": {"klassen": ["a"], "routen": [{"schluessel": "r", "art": "lokal", "klasse": "a"}], "kunden": []}, '
     b'"modellzonen": {"an": true, "\\u0061n": false, "zonen": []}, "anfragen": [{"einheit": {"klasse": "a"}, '
     b'"route": "r", "kontext": {"nutzer": "n"}}]}'),
    ("u-doppel-kontext", ["Z10", "A139"], "Ein doppelter Schlüssel drei Ebenen tief im Kontext einer Anfrage.",
     b'{"vorgabe": {"klassen": ["a"], "routen": [{"schluessel": "r", "art": "lokal"}], "kunden": []}, '
     b'"modellzonen": {"an": true, "zonen": [{"schluessel": "z", "klassen": "alle", "bedingung": "immer", '
     b'"routen": ["r"]}]}, "anfragen": [{"einheit": null, "route": "r", '
     b'"kontext": {"nutzer": "n-a", "nutzer": "n-b"}}]}'),
    ("u-kein-json", ["Z10"], "Die Eingabe ist kein JSON.", b'{"modellzonen": '),
]


def lade(roh):
    """Decodes like the gate reads numbers (Nachtrag 20.11., 2): a number with a minus sign, a decimal point or an
    exponent is not an integer in digit form; it is read as a float, which no integer field accepts."""
    return json.loads(roh, parse_int=lambda s: float(s) if s.startswith("-") else int(s))


# Nachtrag 20.11., 2: number form of jetzt and bis. (name, description, text in the base case, text sent)
ZAHLFORM = [
    ("u-jetzt-minus-null", "jetzt ist -0; nur Ziffern ohne Vorzeichen sind gültig.", '"jetzt": 1799999900', '"jetzt": -0'),
    ("u-jetzt-exponent", "jetzt ist 1e9 mit Exponent.", '"jetzt": 1799999900', '"jetzt": 1e9'),
    ("u-jetzt-plus", "jetzt ist +1; das ist schon kein JSON.", '"jetzt": 1799999900', '"jetzt": +1'),
    ("u-ausnahme-frist-minus-null", "bis ist -0.", '"bis": 1800000000', '"bis": -0'),
    ("u-ausnahme-frist-exponent", "bis ist 1E9 mit Exponent.", '"bis": 1800000000', '"bis": 1E9'),
    ("u-ausnahme-frist-punkt-null", "bis ist 1800000000.0 mit Dezimalpunkt.", '"bis": 1800000000', '"bis": 1800000000.0'),
    ("u-ausnahme-frist-plus", "bis ist +1; das ist schon kein JSON.", '"bis": 1800000000', '"bis": +1'),
]


def zahlform_faelle():
    text = json.dumps(ungueltig("basis", "", lambda f: None)[3], ensure_ascii=False)
    assert gueltiger_fall(lade(text)), "Zwilling der Zahlform-Faelle ungueltig"
    faelle = []
    for name, beschreibung, alt, neu in ZAHLFORM:
        assert text.count(alt) == 1, (name, text.count(alt))
        roh = text.replace(alt, neu).encode() + b"\n"
        try:
            gueltig = gueltiger_fall(lade(roh))
        except json.JSONDecodeError:
            gueltig = False
        assert not gueltig, f"{name}: Fall waere gueltig"
        faelle.append((name, ["Z10"], "Nachtrag 20.11., 2: " + beschreibung, roh))
    return faelle


# ---------------------------------------------------------------- reference (only to check the tables above)

def ist_schl(x):
    return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None


def ist_zahl(x):
    return isinstance(x, int) and not isinstance(x, bool) and 0 <= x < U64


def hat_text(x):
    """A text with at least one character that is not ASCII whitespace (space, tab, LF, CR)."""
    return isinstance(x, str) and x.strip(" \t\n\r") != ""


def stufe(v, satz):
    """Highest position a class set admits; -1 for keine."""
    return {"alle": len(v["klassen"]) - 1, "niedrigste": 0, "keine": -1}[satz]


def gueltig_zonen(v, m):
    """True if modellzonen is valid by zonen/VERTRAG.md, section 3 (the vorgabe is already valid)."""
    if not isinstance(m, dict) or not set(m) <= {"an", "zonen", "freigaben", "ausnahmen", "regeln"}:
        return False
    if not {"an", "zonen"} <= set(m) or not isinstance(m["an"], bool) or not isinstance(m["zonen"], list):
        return False
    routen = {r["schluessel"]: r for r in v["routen"]}
    kunden = {k["schluessel"] for k in v["kunden"]}
    zonen, belegt = {}, set()
    for i, z in enumerate(m["zonen"]):
        if not isinstance(z, dict) or not set(z) <= {"schluessel", "klassen", "bedingung", "routen"}:
            return False
        if not {"schluessel", "klassen", "bedingung"} <= set(z) or not ist_schl(z["schluessel"]):
            return False
        if z["schluessel"] in zonen or z["klassen"] not in KLASSENSAETZE or z["bedingung"] not in BEDINGUNGEN:
            return False
        if z["bedingung"] == "nachrangig" and i == 0:
            return False
        rs = z.get("routen", [])
        if not isinstance(rs, list):
            return False
        for r in rs:
            if not isinstance(r, str) or r not in routen or r in belegt:
                return False
            belegt.add(r)
        zonen[z["schluessel"]] = z
    zone_der_route = {r: z for z in zonen.values() for r in z.get("routen", [])}
    fr = m.get("freigaben", [])
    if not isinstance(fr, list):
        return False
    for f in fr:
        if not isinstance(f, dict) or set(f) != {"route", "kunde", "projekt"}:
            return False
        z = zone_der_route.get(f["route"]) if isinstance(f["route"], str) else None
        if z is None or z["bedingung"] != "freigabe" or not isinstance(f["kunde"], str) or f["kunde"] not in kunden or not ist_schl(f["projekt"]):
            return False
    au = m.get("ausnahmen", [])
    if not isinstance(au, list):
        return False
    for a in au:
        if not isinstance(a, dict) or not set(a) <= {"zone", "nutzer", "gruppen", "gesetzt_von", "zweck", "bis"}:
            return False
        if not {"zone", "gesetzt_von", "zweck", "bis"} <= set(a):
            return False
        z = zonen.get(a["zone"]) if isinstance(a["zone"], str) else None
        if z is None or z["bedingung"] != "gesperrt":
            return False
        wer = []
        for feld in ("nutzer", "gruppen"):
            liste = a.get(feld, [])
            if not isinstance(liste, list) or not all(ist_schl(x) for x in liste):
                return False
            wer += liste
        if not wer or not ist_schl(a["gesetzt_von"]) or not hat_text(a["zweck"]) or not ist_zahl(a["bis"]):
            return False
    rg = m.get("regeln", [])
    if not isinstance(rg, list):
        return False
    for r in rg:
        if not isinstance(r, dict) or not set(r) <= {"ebene", "wer", "zone", "klassen", "gesperrt"}:
            return False
        if not {"ebene", "wer", "zone"} <= set(r) or not ({"klassen", "gesperrt"} & set(r)):
            return False
        z = zonen.get(r["zone"]) if isinstance(r["zone"], str) else None
        if r["ebene"] not in ("gruppe", "nutzer") or not ist_schl(r["wer"]) or z is None:
            return False
        if "gesperrt" in r and r["gesperrt"] is not True:
            return False
        if "klassen" in r and (r["klassen"] not in KLASSENSAETZE or stufe(v, r["klassen"]) > stufe(v, z["klassen"])):
            return False
    return True


def gueltige_anfrage(a):
    if not isinstance(a, dict) or not set(a) <= {"einheit", "route", "kunde", "projekt", "kontext"} or "einheit" not in a:
        return False
    e = a["einheit"]
    if e is not None and (not isinstance(e, dict) or not set(e) <= {"klasse"}):
        return False
    if not all(isinstance(a.get(f), (str, type(None))) for f in ("route", "kunde", "projekt")):
        return False
    k = a.get("kontext", {})
    if not isinstance(k, dict) or not set(k) <= {"nutzer", "gruppen", "jetzt", "grund"}:
        return False
    if not isinstance(k.get("nutzer"), (str, type(None))) or not isinstance(k.get("grund"), (str, type(None))):
        return False
    g = k.get("gruppen", [])
    if not isinstance(g, list) or not all(isinstance(x, str) for x in g):
        return False
    return k.get("jetzt") is None or ist_zahl(k["jetzt"])


def gueltiger_fall(fall):
    if not isinstance(fall, dict) or set(fall) - {"name", "regeln", "beschreibung", "vorgabe", "modellzonen", "anfragen"}:
        return False
    if not {"vorgabe", "modellzonen", "anfragen"} <= set(fall) or not DT3.gueltig(fall["vorgabe"]):
        return False
    if not gueltig_zonen(fall["vorgabe"], fall["modellzonen"]) or not isinstance(fall["anfragen"], list):
        return False
    return all(gueltige_anfrage(a) for a in fall["anfragen"]) and DT3.tiefe(fall) <= 64


def ausnahme_voll(a):
    return {"zone": a["zone"], "nutzer": a.get("nutzer", []), "gruppen": a.get("gruppen", []),
            "gesetzt_von": a["gesetzt_von"], "zweck": a["zweck"], "bis": a["bis"]}


def referenz(v, m, a):
    """(reason, effective class, protokoll) by zonen/VERTRAG.md, section 5."""
    e, r_, k_, p_ = a["einheit"], a.get("route"), a.get("kunde"), a.get("projekt")
    if not m["an"]:
        if e is not None:
            g, kl = DT3.referenz(v, {"einheit": e, "route": r_, "kunde": k_})
            return g, kl, None
        r = {x["schluessel"]: x for x in v["routen"]}.get(r_) if r_ else None
        if not r_:
            return "route-fehlt", None, None
        if r is None or r.get("klasse") is None:
            return "route-ohne-klasse", None, None
        return "frei", None, None
    kx_ = a.get("kontext", {})
    nutzer, gruppen, jetzt, grund = kx_.get("nutzer"), kx_.get("gruppen", []), kx_.get("jetzt"), kx_.get("grund")
    kl = v["klassen"]
    top = kl[-1]
    routen = {x["schluessel"]: x for x in v["routen"]}
    kunden = {x["schluessel"]: x for x in v["kunden"]}
    frei_a50 = {(f["route"], f["kunde"]) for f in v.get("kundendaten_an_cloud", {}).get("freigaben", [])}
    frei_p = {(f["route"], f["kunde"], f["projekt"]) for f in m.get("freigaben", [])}
    zone_der_route = {r: z for z in m["zonen"] for r in z.get("routen", [])}
    # D5
    if e is None:
        eigene = wirksam = None
    else:
        roh = e.get("klasse")
        eigene = v.get("vorgabestufe", top) if roh is None else (roh if isinstance(roh, str) and roh in kl else None)
        wirksam = None if eigene is None else (top if k_ is not None else eigene)
    z = zone_der_route.get(r_) if r_ else None
    # protokoll
    ausn = None
    # Nachtrag 20.11., 2: an exception may hold through a group without a user; the answer is still nutzer-fehlt (Z1)
    if z is not None and z["bedingung"] == "gesperrt" and jetzt is not None:
        for x in m.get("ausnahmen", []):
            if x["zone"] == z["schluessel"] and jetzt < x["bis"] and (
                    nutzer in x.get("nutzer", []) or any(g in x.get("gruppen", []) for g in gruppen)):
                ausn = ausnahme_voll(x)
                break
    prot = {"zone": z["schluessel"] if z else None,
            "grund": grund if z is not None and z["bedingung"] == "nachrangig" and hat_text(grund) else None,
            "ausnahme": ausn}

    def ergebnis(g):
        return g, wirksam, prot

    if not r_:
        return ergebnis("route-fehlt")
    if not nutzer:
        return ergebnis("nutzer-fehlt")
    if z is None:
        return ergebnis("route-ohne-zone")
    if e is not None and eigene is None:
        return ergebnis("einheit-ohne-klasse")
    regeln = [x for x in m.get("regeln", []) if x["zone"] == z["schluessel"] and (
        (x["ebene"] == "nutzer" and x["wer"] == nutzer) or (x["ebene"] == "gruppe" and x["wer"] in gruppen))]
    if any(x.get("gesperrt") for x in regeln):
        return ergebnis("zone-durch-regel-gesperrt")
    if z["bedingung"] == "gesperrt" and ausn is None:
        return ergebnis("zone-gesperrt")
    if z["bedingung"] == "nachrangig" and not hat_text(grund):
        return ergebnis("grund-fehlt")
    if e is None:
        return ergebnis("frei")
    grenze = min([stufe(v, z["klassen"])] + [stufe(v, x["klassen"]) for x in regeln if "klassen" in x])
    if kl.index(wirksam) > grenze:
        return ergebnis("klasse-ueber-zone")
    r = routen[r_]
    if r.get("klasse") is not None and kl.index(wirksam) > kl.index(r["klasse"]):
        return ergebnis("klasse-ueber-route")
    if k_ is not None or wirksam == top:
        if k_ is None:
            return ergebnis("kunde-fehlt")
        kd = kunden.get(k_)
        if kd is None:
            return ergebnis("kunde-ohne-eintrag")
        if r_ in kd.get("verboten", []):
            return ergebnis("route-fuer-kunde-verboten")
        if r_ not in kd.get("erlaubt", []):
            return ergebnis("route-fuer-kunde-nicht-erlaubt")
        if z["bedingung"] == "freigabe":
            if (r_, k_, p_) not in frei_p:
                return ergebnis("zone-ohne-freigabe")
        elif r["art"] == "cloud" and (r_, k_) not in frei_a50:
            return ergebnis("kundendaten-an-cloud")
    return ergebnis("frei")


def antwort(grund, klasse, route_, prot):
    text = MELDUNG[grund]
    if "{route}" in text:
        text = text.replace("{route}", route_)
    return {"darf": grund == "frei", "grund": grund, "klasse": klasse, "meldung": text, "protokoll": prot}


def erwartet_prot(erw, extra):
    if erw[2] == AUS:
        assert not extra
        return None
    return {"zone": erw[2], "grund": extra.get("grund"), "ausnahme": extra.get("ausnahme")}


def aus_vertrag3():
    """Counter-check of A127: every valid case of contract 3 with the option off, with its expectations from there."""
    faelle = []
    for name, regeln, beschreibung, v, paare in DT3.FAELLE:
        if name in DT3.TIEFE:
            continue
        faelle.append((f"z0-aus-{name}", ["A127", "Z0"] + regeln,
                       f"Option aus: der Fall {name} aus Vertrag 3 mit derselben Erwartung. {beschreibung}",
                       v, MZ_AUS if v is V_Z else mz(an=False, zonen=[]),
                       [(a, (g, kl, AUS)) for a, (g, kl) in paare]))
    return faelle


def main():
    dateien = {}
    namen = set()
    alle = FAELLE + aus_vertrag3()
    for name, regeln, beschreibung, v, m, paare in alle:
        assert name not in namen, name
        namen.add(name)
        fall = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v, "modellzonen": m,
                "anfragen": [p[0] for p in paare]}
        assert gueltiger_fall(json.loads(json.dumps(fall))), f"{name}: Fall ungueltig"
        antworten = []
        for a, erw, *extra in paare:
            extra = extra[0] if extra else {}
            prot = erwartet_prot(erw, extra)
            ref = referenz(v, m, json.loads(json.dumps(a)))
            assert ref == (erw[0], erw[1], prot), f"{name}: {a} erwartet {(erw[0], erw[1], prot)}, Referenz {ref}"
            antworten.append(antwort(erw[0], erw[1], a.get("route"), prot))
        dateien[name] = (json.dumps(fall, ensure_ascii=False, indent=1).encode() + b"\n",
                         {"exit": 0, "ausgabe": {"antworten": antworten}})
    for name, regeln, beschreibung, fall in UNGUELTIG + zahlform_faelle():
        assert name not in namen, name
        namen.add(name)
        if isinstance(fall, bytes):
            roh = fall
        else:
            assert not gueltiger_fall(fall), f"{name}: Fall waere gueltig"
            roh = json.dumps(fall, ensure_ascii=False).encode() + b"\n"
        dateien[name] = (roh, {"exit": 2, "regeln": regeln, "beschreibung": beschreibung})
    # Each invalid case differs from a valid one in one point: the base case without the change must be valid.
    assert gueltiger_fall(json.loads(json.dumps(ungueltig("basis", "", lambda f: None)[3])))
    if ZIEL.exists():
        shutil.rmtree(ZIEL)
    (ZIEL / "faelle").mkdir(parents=True)
    (ZIEL / "erwartet").mkdir(parents=True)
    for name, (roh, erw) in sorted(dateien.items()):
        (ZIEL / "faelle" / f"{name}.json").write_bytes(roh)
        (ZIEL / "erwartet" / f"{name}.json").write_text(json.dumps(erw, ensure_ascii=False, indent=1) + "\n",
                                                       encoding="utf-8")
    gueltige = sum(1 for _, (_, e) in dateien.items() if e["exit"] == 0)
    anfragen = sum(len(p) for *_, p in alle)
    print(f"{len(dateien)} Faelle ({gueltige} gueltig mit {anfragen} Anfragen, {len(dateien) - gueltige} ungueltig)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
