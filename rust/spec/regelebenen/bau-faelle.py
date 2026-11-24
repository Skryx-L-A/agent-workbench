#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags Regelebenen (faelle/, erwartet/).

Aufruf: python3 -I rust/spec/regelebenen/bau-faelle.py
Jede Erwartung rechnet eine kleine Referenz der Abschnitte 5 bis 7 des README. Jeder Fall trägt zusätzlich
ausgeschriebene Kernwerte; weicht die Referenz davon ab, bricht das Skript mit Exit 1 ab und schreibt nichts.
Formfehler (Stufen 3 bis 12) rechnet die Referenz nicht; ihr Code steht ausgeschrieben im Fall (`form`).
"""
import copy
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
PSEUDONYM = re.compile(r"^ps_[0-9a-f]{32}$")  # Personen nur als Pseudonym (A12, B09)
EBENEN = ("firma", "gruppe", "nutzer")

# --- Referenz ---------------------------------------------------------------------------------------------


class Abgelehnt(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


def kanonisch_hash(obj):
    # RFC 8785 fällt für diese Daten (ASCII-Schlüssel, Ganzzahlen, Texte ohne Escape) mit sortiertem kompaktem JSON zusammen.
    roh = json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(roh).hexdigest()


class Zustand:
    def __init__(self, firma):
        e = firma["einstellung"]
        self.klassen = list(e["klassen"])
        self.pos = {k: i for i, k in enumerate(self.klassen)}
        self.vorgabestufe = e.get("vorgabestufe", self.klassen[-1])
        self.routen = {r["schluessel"]: {"art": r["art"], "klasse": r["klasse"], "gesperrt": False} for r in e["routen"]}
        self.reihe_routen = [r["schluessel"] for r in e["routen"]]
        self.kunden = {k["schluessel"]: {"erlaubt": set(k.get("erlaubt", [])), "verboten": set(k.get("verboten", []))}
                       for k in e["kunden"]}
        self.reihe_kunden = [k["schluessel"] for k in e["kunden"]]
        self.freigaben = {(f["route"], f["kunde"]) for f in e["kundendaten_an_cloud"]["freigaben"]}
        s = firma.get("senkung", {"aus": True, "bestaetigung": False, "arten": []})
        self.senkung = {"aus": s["aus"], "bestaetigung": s["bestaetigung"], "arten": set(s["arten"])}
        self.protokoll = firma.get("protokoll", {"aufbewahrung_tage": 365})
        self.routen_status = firma.get("routen_status", [])
        self.herkunft = {}
        self.hinweise = []
        for st in self.routen_status:
            if st["status"] in ("gesperrt", "evaluierung"):
                self.sperren(st["route"], "firma")
        self.aufraeumen()

    def sperren(self, r, ebene):
        self.routen[r]["gesperrt"] = True
        self.herkunft[("routen", r, "gesperrt")] = ebene

    def gesperrt(self, r):
        return self.routen[r]["gesperrt"]

    def aufraeumen(self):
        weg = {r for r, v in self.routen.items() if v["gesperrt"]}
        for k in self.kunden.values():
            k["erlaubt"] -= weg
            k["verboten"] -= weg
        self.freigaben = {p for p in self.freigaben if p[0] not in weg}

    def listen(self):
        bild = {("kundendaten_an_cloud", "freigaben"): frozenset(self.freigaben),
                ("senkung", "arten"): frozenset(self.senkung["arten"])}
        for k, v in self.kunden.items():
            bild[("kunden", k, "erlaubt")] = frozenset(v["erlaubt"])
            bild[("kunden", k, "verboten")] = frozenset(v["verboten"])
        return bild

    # Bezüge einer Änderung (Stufen 17 bis 19), über den ganzen Regelsatz
    def bezuege(self, regeln):
        routen_refs, kunden_refs, klassen_refs = [], [], []
        for e in regeln.get("routen", []):
            routen_refs.append(e["route"])
            if "klasse" in e:
                klassen_refs.append(e["klasse"])
        for e in regeln.get("kunden", []):
            kunden_refs.append(e["kunde"])
            routen_refs += e.get("erlaubt", []) + e.get("verboten", [])
        for p in regeln.get("freigaben_entfernt", []):
            routen_refs.append(p["route"])
            kunden_refs.append(p["kunde"])
        if any(r not in self.routen for r in routen_refs):
            raise Abgelehnt("ebenen.route_unbekannt")
        if any(k not in self.kunden for k in kunden_refs):
            raise Abgelehnt("ebenen.kunde_unbekannt")
        if any(k not in self.pos for k in klassen_refs):
            raise Abgelehnt("ebenen.klasse_unbekannt")

    def anwenden(self, regeln, ebene, strikt):
        if strikt:
            self.bezuege(regeln)
        vorher = self.listen()

        def hinweis(code, pfad):
            self.hinweise.append({"code": code, "ebene": ebene, "pfad": list(pfad)})

        def lockerung(pfad):
            if strikt:
                raise Abgelehnt("ebenen.lockerung")
            hinweis("ebenen.lockerung_ignoriert", pfad)

        for e in regeln.get("routen", []):
            r = e["route"]
            if r not in self.routen:
                hinweis("ebenen.route_unbekannt_ignoriert", ["routen", r])
                continue
            if self.gesperrt(r):
                if e.get("gesperrt") is False:
                    lockerung(["routen", r, "gesperrt"])
                continue
            if "klasse" in e:
                k = e["klasse"]
                if k not in self.pos:
                    hinweis("ebenen.klasse_unbekannt_gesperrt", ["routen", r, "klasse"])
                    self.sperren(r, ebene)
                    continue
                alt = self.routen[r]["klasse"]
                if self.pos[k] > self.pos[alt]:
                    lockerung(["routen", r, "klasse"])
                elif self.pos[k] < self.pos[alt]:
                    self.routen[r]["klasse"] = k
                    self.herkunft[("routen", r, "klasse")] = ebene
            if e.get("gesperrt") is True:
                self.sperren(r, ebene)
        for e in regeln.get("kunden", []):
            k = e["kunde"]
            if k not in self.kunden:
                hinweis("ebenen.kunde_unbekannt_ignoriert", ["kunden", k])
                continue
            kd = self.kunden[k]
            if "erlaubt" in e:
                neu = set()
                for x in e["erlaubt"]:
                    if x not in self.routen:
                        hinweis("ebenen.route_unbekannt_ignoriert", ["kunden", k, "erlaubt", x])
                    elif self.gesperrt(x):
                        pass
                    elif x not in kd["erlaubt"]:
                        lockerung(["kunden", k, "erlaubt", x])
                    else:
                        neu.add(x)
                kd["erlaubt"] = neu
            for x in e.get("verboten", []):
                if x not in self.routen:
                    hinweis("ebenen.route_unbekannt_ignoriert", ["kunden", k, "verboten", x])
                elif not self.gesperrt(x):
                    kd["verboten"].add(x)
        for p in regeln.get("freigaben_entfernt", []):
            if p["route"] not in self.routen:
                hinweis("ebenen.route_unbekannt_ignoriert", ["kundendaten_an_cloud", "freigaben", p["route"]])
            elif p["kunde"] not in self.kunden:
                hinweis("ebenen.kunde_unbekannt_ignoriert", ["kundendaten_an_cloud", "freigaben", p["kunde"]])
            else:
                self.freigaben.discard((p["route"], p["kunde"]))
        s = regeln.get("senkung", {})
        if "arten" in s:
            neu = set()
            for a in s["arten"]:
                if a not in self.senkung["arten"]:
                    lockerung(["senkung", "arten", a])
                else:
                    neu.add(a)
            self.senkung["arten"] = neu
        for feld in ("aus", "bestaetigung"):
            if feld in s:
                if s[feld] is True and not self.senkung[feld]:
                    self.senkung[feld] = True
                    self.herkunft[("senkung", feld)] = ebene
                elif s[feld] is False and self.senkung[feld]:
                    lockerung(["senkung", feld])
        self.aufraeumen()
        nachher = self.listen()
        for pfad, wert in nachher.items():
            if vorher[pfad] != wert:
                self.herkunft[pfad] = ebene

    def wirksam(self):
        routen = [{"schluessel": r, "art": self.routen[r]["art"], "klasse": self.routen[r]["klasse"]}
                  for r in self.reihe_routen if not self.gesperrt(r)]
        if not routen:
            raise Abgelehnt("ebenen.keine_route")
        einst = {
            "fassung": 1, "klassen": self.klassen, "vorgabestufe": self.vorgabestufe, "routen": routen,
            "kunden": [{"schluessel": k, "erlaubt": sorted(self.kunden[k]["erlaubt"]),
                        "verboten": sorted(self.kunden[k]["verboten"])} for k in self.reihe_kunden],
            "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": [
                {"route": r, "kunde": k} for r, k in sorted(self.freigaben)]},
        }
        herkunft = []

        def h(pfad, wert):
            herkunft.append({"pfad": list(pfad), "wert": wert, "ebene": self.herkunft.get(tuple(pfad), "firma")})

        for r in self.reihe_routen:
            if self.gesperrt(r):
                h(("routen", r, "gesperrt"), True)
            else:
                h(("routen", r, "klasse"), self.routen[r]["klasse"])
        for k in self.reihe_kunden:
            h(("kunden", k, "erlaubt"), sorted(self.kunden[k]["erlaubt"]))
            h(("kunden", k, "verboten"), sorted(self.kunden[k]["verboten"]))
        h(("kundendaten_an_cloud", "freigaben"), einst["kundendaten_an_cloud"]["freigaben"])
        h(("senkung", "aus"), self.senkung["aus"])
        h(("senkung", "bestaetigung"), self.senkung["bestaetigung"])
        h(("senkung", "arten"), sorted(self.senkung["arten"]))
        return {
            "einstellung": einst,
            "senkung": {"aus": self.senkung["aus"], "bestaetigung": self.senkung["bestaetigung"],
                        "arten": sorted(self.senkung["arten"])},
            "protokoll": self.protokoll,
            "routen_status": self.routen_status,
            "herkunft": herkunft,
            "hinweise": self.hinweise,
        }


def gruppe_von(firma, nutzer):
    treffer = [g["schluessel"] for g in firma["gruppen"] if nutzer in g["mitglieder"]]
    return treffer


def firma_intern(firma):
    """Stufen 13 und 14 der Firmendatei; die Formstufen davor rechnet die Referenz nicht."""
    bekannt = {r["schluessel"] for r in firma["einstellung"]["routen"]}
    if any(st["route"] not in bekannt for st in firma.get("routen_status", [])):
        raise Abgelehnt("ebenen.route_unbekannt")
    gesehen = set()
    for g in firma["gruppen"]:
        for m in g["mitglieder"]:
            if m in gesehen:
                raise Abgelehnt("ebenen.nutzer_mehrfach")
            gesehen.add(m)


def oben_fuer(stand, nutzer):
    """Firma, Nutzerregel der Firma und Gruppe im Lademodus; Abgelehnt bei einem Fehler der Ladestufen."""
    firma = stand["firma"]
    firma_intern(firma)
    z = Zustand(firma)
    for nr in firma["nutzerregeln"]:
        if nr["nutzer"] == nutzer:
            z.anwenden(nr["regeln"], "firma", strikt=False)
    gruppen = gruppe_von(firma, nutzer)
    if gruppen and gruppen[0] in stand["gruppen"]:
        gd = stand["gruppen"][gruppen[0]]
        if gd.get("ebene") != "gruppe" or gd.get("gruppe") != gruppen[0]:
            raise Abgelehnt("ebenen.ebene_falsch")
        z.anwenden(gd["regeln"], "gruppe", strikt=False)
    return z


def zeige(stand, nutzer):
    try:
        z = oben_fuer(stand, nutzer)
        if nutzer in stand["nutzer"]:
            nd = stand["nutzer"][nutzer]
            if nd.get("ebene") != "nutzer" or nd.get("nutzer") != nutzer:
                raise Abgelehnt("ebenen.ebene_falsch")
            z.anwenden(nd["regeln"], "nutzer", strikt=False)
        return {"wirksam": z.wirksam()}
    except Abgelehnt as a:
        return {"fehler": a.code}


def aendern(stand, wer, datei, form=None):
    ebene = datei.get("ebene") if datei.get("ebene") in EBENEN else None
    ziel = None
    if ebene in ("gruppe", "nutzer"):
        z = datei.get(ebene)
        muster = PSEUDONYM if ebene == "nutzer" else KENNUNG
        ziel = z if isinstance(z, str) and muster.match(z) else None
    if ebene == "firma":
        alt_datei = stand["firma"]
    elif ebene == "gruppe" and ziel:
        alt_datei = stand["gruppen"].get(ziel)
    elif ebene == "nutzer" and ziel:
        alt_datei = stand["nutzer"].get(ziel)
    else:
        alt_datei = None
    code = form
    try:
        if code is None:
            admins = [p for p, rollen in stand["firma"]["rollen"].items() if "admin" in rollen]
            if ebene == "firma":
                firma_intern(datei)
                if wer not in admins:
                    raise Abgelehnt("ebenen.nicht_befugt")
                for nr in datei["nutzerregeln"]:
                    Zustand(datei).anwenden(nr["regeln"], "firma", strikt=True)
            elif ebene == "gruppe":
                if wer not in admins:
                    raise Abgelehnt("ebenen.nicht_befugt")
                if ziel not in [g["schluessel"] for g in stand["firma"]["gruppen"]]:
                    raise Abgelehnt("ebenen.gruppe_unbekannt")
                Zustand(stand["firma"]).anwenden(datei["regeln"], "gruppe", strikt=True)
            else:
                if wer != ziel and wer not in admins:
                    raise Abgelehnt("ebenen.nicht_befugt")
                oben_fuer(stand, ziel).anwenden(datei["regeln"], "nutzer", strikt=True)
    except Abgelehnt as a:
        code = a.code
    # Form aus Vertrag 5, Abschnitt 6.6 (B13); fassung, vorher und zeit setzt die Bibliothek.
    zeile = {"aktionsname": "regel.abgelehnt" if code else "regel.geaendert",
             "alt": kanonisch_hash(alt_datei) if alt_datei is not None else None, "antrag": None, "code": code,
             "ebene": ebene, "ergebnis": "abgelehnt" if code else "geaendert", "handelnd": wer,
             "neu": kanonisch_hash(datei), "pflicht": None, "ziel": ziel if ebene != "firma" else None, "zweiter": None}
    if code is None:
        if ebene == "firma":
            stand["firma"] = copy.deepcopy(datei)
        elif ebene == "gruppe":
            stand["gruppen"][ziel] = copy.deepcopy(datei)
        else:
            stand["nutzer"][ziel] = copy.deepcopy(datei)
    return {"angenommen": code is None, "code": code, "zeile": zeile}


def ausfuehren(fall):
    stand = copy.deepcopy(fall["stand"])
    stand.setdefault("gruppen", {})
    stand.setdefault("nutzer", {})
    ergebnisse = []
    for i, s in enumerate(fall["schritte"]):
        form = fall.get("_form", {}).get(i)
        if "zeige" in s:
            ergebnisse.append({"fehler": form} if form else zeige(stand, s["zeige"]))
        else:
            ergebnisse.append(aendern(stand, s["aendern"]["wer"], s["aendern"]["datei"], form))
    return {"schritte": ergebnisse}


# --- Daten ------------------------------------------------------------------------------------------------

EINST = {
    "fassung": 1,
    "klassen": ["oeffentlich", "intern", "kundendaten"],
    "vorgabestufe": "kundendaten",
    "routen": [
        {"schluessel": "cloud-r", "art": "cloud", "klasse": "intern"},
        {"schluessel": "cloud-s", "art": "cloud", "klasse": "intern"},
        {"schluessel": "lokal-l", "art": "lokal", "klasse": "kundendaten"},
    ],
    "kunden": [
        {"schluessel": "k-beispiel", "erlaubt": ["cloud-r", "lokal-l"], "verboten": []},
        {"schluessel": "k-zwei", "erlaubt": ["lokal-l"], "verboten": []},
    ],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": [{"route": "cloud-r", "kunde": "k-beispiel"}]},
}
FIRMA = {
    "fassung": 1, "ebene": "firma", "einstellung": EINST,
    "rollen": {"ps_00000000000000000000000000000b01": ["admin"], "ps_00000000000000000000000000000b02": ["admin"]},
    "gruppen": [{"schluessel": "g-eins", "mitglieder": ["ps_00000000000000000000000000000c01"]},
                {"schluessel": "g-zwei", "mitglieder": ["ps_00000000000000000000000000000c02"]}],
    "nutzerregeln": [],
    "senkung": {"aus": False, "bestaetigung": False, "arten": ["bedienwissen", "quellenauszug", "regeln", "vorlagen"]},
}


def firma(**felder):
    f = copy.deepcopy(FIRMA)
    f.update(copy.deepcopy(felder))
    return f


def gruppe(g, regeln):
    return {"fassung": 1, "ebene": "gruppe", "gruppe": g, "regeln": regeln}


def nutzer(n, regeln):
    return {"fassung": 1, "ebene": "nutzer", "nutzer": n, "regeln": regeln}


def Z(n):
    return {"zeige": n}


def A(wer, datei):
    return {"aendern": {"wer": wer, "datei": datei}}


# Kernwerte: Prüffunktionen auf das Ergebnis eines Schritts
def klasse(r):
    def f(e):
        for x in e["wirksam"]["einstellung"]["routen"]:
            if x["schluessel"] == r:
                return x["klasse"]
        return "gesperrt"
    return f


def herkunft(*pfad):
    def f(e):
        for h in e["wirksam"]["herkunft"]:
            if h["pfad"] == list(pfad):
                return h["ebene"]
        return None
    return f


def kunde(k, feld):
    def f(e):
        for x in e["wirksam"]["einstellung"]["kunden"]:
            if x["schluessel"] == k:
                return x[feld]
        return None
    return f


def code(e):
    return e.get("code", e.get("fehler"))


def angenommen(e):
    return e["angenommen"]


def hinweise(e):
    return sorted(h["code"] for h in e["wirksam"]["hinweise"])


def senkung(feld):
    return lambda e: e["wirksam"]["senkung"][feld]


def freigaben(e):
    return e["wirksam"]["einstellung"]["kundendaten_an_cloud"]["freigaben"]


FAELLE = []


def fall(name, anforderungen, beschreibung, stand, schritte, kern, form=None):
    FAELLE.append({"name": name, "anforderungen": anforderungen, "beschreibung": beschreibung,
                   "stand": stand, "schritte": schritte, "_kern": kern, "_form": form or {}})


ST = {"firma": FIRMA}

# Anforderungen als Ganzes
U_S_GESPERRT = firma(nutzerregeln=[{"nutzer": "ps_00000000000000000000000000000c01", "regeln": {"routen": [{"route": "cloud-s", "gesperrt": True}]}}])
fall("a26-ausgangslage", ["A26"],
     "Firmenregel cloud-r höchstens intern; u und v in verschiedenen Gruppen; Admin sperrt cloud-s nur für u; u verschärft cloud-r auf oeffentlich, dann versucht u kundendaten",
     ST,
     [A("ps_00000000000000000000000000000b01", U_S_GESPERRT),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}]})),
      Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, angenommen, True), (1, angenommen, True), (2, code, "ebenen.lockerung"),
      (3, klasse("cloud-r"), "oeffentlich"), (3, herkunft("routen", "cloud-r", "klasse"), "nutzer"),
      (3, klasse("cloud-s"), "gesperrt"), (3, herkunft("routen", "cloud-s", "gesperrt"), "firma"),
      (4, klasse("cloud-r"), "intern"), (4, klasse("cloud-s"), "intern")])

fall("a27-admins", ["A27", "A135"],
     "Admin 1 ändert die Gruppe g-eins; ein Mitglied sieht den Stand ohne eigenen Schritt; u versucht dieselbe Änderung; Admin 1 legt einen dritten Admin an, der dann ändert",
     ST,
     [A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"routen": [{"route": "cloud-s", "klasse": "oeffentlich"}]})),
      Z("ps_00000000000000000000000000000c01"),
      A("ps_00000000000000000000000000000c01", gruppe("g-eins", {"routen": [{"route": "cloud-s", "klasse": "oeffentlich"}]})),
      A("ps_00000000000000000000000000000b01", firma(rollen={"ps_00000000000000000000000000000b01": ["admin"], "ps_00000000000000000000000000000b02": ["admin"], "ps_00000000000000000000000000000b03": ["admin"]})),
      A("ps_00000000000000000000000000000b03", gruppe("g-eins", {})),
      Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, klasse("cloud-s"), "oeffentlich"), (2, code, "ebenen.nicht_befugt"),
      (3, angenommen, True), (4, angenommen, True), (5, klasse("cloud-s"), "intern")])

fall("a28-herkunft", ["A28"],
     "cloud-s kommt von der Gruppe, lokal-l vom Nutzer, cloud-r von der Firma; danach sperrt ein Admin cloud-s in der Gruppe",
     {"firma": FIRMA,
      "gruppen": {"g-eins": gruppe("g-eins", {"routen": [{"route": "cloud-s", "klasse": "oeffentlich"}]})},
      "nutzer": {"ps_00000000000000000000000000000c01": nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "lokal-l", "klasse": "intern"}]})}},
     [Z("ps_00000000000000000000000000000c01"),
      A("ps_00000000000000000000000000000b02", gruppe("g-eins", {"routen": [{"route": "cloud-s", "gesperrt": True}]})),
      Z("ps_00000000000000000000000000000c01")],
     [(0, herkunft("routen", "cloud-s", "klasse"), "gruppe"), (0, herkunft("routen", "lokal-l", "klasse"), "nutzer"),
      (0, herkunft("routen", "cloud-r", "klasse"), "firma"), (1, angenommen, True),
      (2, klasse("cloud-s"), "gesperrt"), (2, herkunft("routen", "cloud-s", "gesperrt"), "gruppe")])

fall("a41-senkung", ["A41", "A39"],
     "Firmenliste mit vier Arten; g-eins streicht eine, versucht dann eine fünfte; u schaltet ab; v verlangt Bestätigung",
     ST,
     [A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"senkung": {"arten": ["bedienwissen", "regeln", "vorlagen"]}})),
      A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"senkung": {"arten": ["bedienwissen", "regeln", "vorlagen", "vertraege"]}})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"senkung": {"aus": True}})),
      A("ps_00000000000000000000000000000c02", nutzer("ps_00000000000000000000000000000c02", {"senkung": {"bestaetigung": True}})),
      Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, angenommen, True), (1, code, "ebenen.lockerung"), (2, angenommen, True), (3, angenommen, True),
      (4, senkung("arten"), ["bedienwissen", "regeln", "vorlagen"]), (4, senkung("aus"), True),
      (4, herkunft("senkung", "arten"), "gruppe"), (4, herkunft("senkung", "aus"), "nutzer"),
      (5, senkung("arten"), ["bedienwissen", "quellenauszug", "regeln", "vorlagen"]),
      (5, senkung("bestaetigung"), True), (5, senkung("aus"), False)])

STATUS = [{"route": "cloud-s", "status": "gesperrt", "grund": "Evaluierung laeuft", "seit": "2026-11-06",
           "freigabe_durch": "it-leitung", "zone": "z2"}]
fall("a45-routenstatus", ["A45"],
     "cloud-s ist gesperrt mit Grund, Datum, Freigebendem und Zone; u versucht den Status zu setzen; der Admin gibt frei",
     {"firma": firma(routen_status=STATUS)},
     [Z("ps_00000000000000000000000000000c01"),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen_status": [dict(STATUS[0], status="frei")]})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-s", "gesperrt": False}]})),
      A("ps_00000000000000000000000000000b01", firma(routen_status=[dict(STATUS[0], status="frei", grund="Evaluierung abgeschlossen")])),
      Z("ps_00000000000000000000000000000c01")],
     [(0, klasse("cloud-s"), "gesperrt"), (0, lambda e: e["wirksam"]["routen_status"], STATUS),
      (0, kunde("k-beispiel", "erlaubt"), ["cloud-r", "lokal-l"]),
      (1, code, "ebenen.schluessel_unbekannt"), (2, code, "ebenen.lockerung"), (3, angenommen, True),
      (4, klasse("cloud-s"), "intern")],
     form={1: "ebenen.schluessel_unbekannt"})

fall("a103-protokoll", ["A103", "A102"],
     "ohne Angabe gelten Aufbewahrung 365 Tage; der Admin stellt 0 ein (nichts löschen); eine Gruppe kann das Feld nicht setzen; das Feld einsicht gibt es hier nicht mehr (B08, es gehört der Stelle)",
     ST,
     [Z("ps_00000000000000000000000000000c01"),
      A("ps_00000000000000000000000000000b01", firma(protokoll={"aufbewahrung_tage": 0})),
      Z("ps_00000000000000000000000000000c01"),
      A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"protokoll": {"aufbewahrung_tage": 1}})),
      A("ps_00000000000000000000000000000b01", firma(protokoll={"aufbewahrung_tage": 30, "einsicht": "rolle"}))],
     [(0, lambda e: e["wirksam"]["protokoll"], {"aufbewahrung_tage": 365}), (1, angenommen, True),
      (2, lambda e: e["wirksam"]["protokoll"]["aufbewahrung_tage"], 0), (3, code, "ebenen.schluessel_unbekannt"),
      (4, code, "ebenen.schluessel_unbekannt")],
     form={3: "ebenen.schluessel_unbekannt", 4: "ebenen.schluessel_unbekannt"})

EINE = firma(gruppen=[])
del EINE["senkung"]
fall("eine-ebene", ["A26"],
     "nur die Firma, keine Gruppen, keine Nutzerdatei: die wirksame Einstellung ist die eingebettete nach Vertrag 2 Fassung 1",
     {"firma": EINE}, [Z("ps_00000000000000000000000000000c01")],
     [(0, lambda e: e["wirksam"]["einstellung"], EINST), (0, senkung("aus"), True)])

# Verschärfung je Feldart
fall("verschaerfen-klasse", ["A26"], "der Nutzer senkt die Klasse von lokal-l auf intern", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "lokal-l", "klasse": "intern"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, klasse("lokal-l"), "intern")])
fall("verschaerfen-route-gesperrt", ["A26"],
     "der Nutzer sperrt lokal-l; sie fällt auch aus erlaubt beider Kunden", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "lokal-l", "gesperrt": True}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, klasse("lokal-l"), "gesperrt"), (1, kunde("k-beispiel", "erlaubt"), ["cloud-r"]),
      (1, kunde("k-zwei", "erlaubt"), []), (1, herkunft("kunden", "k-zwei", "erlaubt"), "nutzer")])
fall("verschaerfen-erlaubt-kuerzer", ["A26", "A51"], "die Gruppe kürzt erlaubt von k-beispiel auf lokal-l", ST,
     [A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"kunden": [{"kunde": "k-beispiel", "erlaubt": ["lokal-l"]}]})), Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, angenommen, True), (1, kunde("k-beispiel", "erlaubt"), ["lokal-l"]),
      (1, herkunft("kunden", "k-beispiel", "erlaubt"), "gruppe"), (2, kunde("k-beispiel", "erlaubt"), ["cloud-r", "lokal-l"])])
fall("verschaerfen-verboten-laenger", ["A26", "A51"], "der Nutzer verbietet cloud-s für k-beispiel", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"kunden": [{"kunde": "k-beispiel", "verboten": ["cloud-s"]}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, kunde("k-beispiel", "verboten"), ["cloud-s"]),
      (1, herkunft("kunden", "k-beispiel", "verboten"), "nutzer")])
fall("verschaerfen-freigabe-entfernt", ["A26", "A50"], "die Gruppe entfernt die Freigabe cloud-r für k-beispiel", ST,
     [A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"freigaben_entfernt": [{"route": "cloud-r", "kunde": "k-beispiel"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, freigaben, []), (1, herkunft("kundendaten_an_cloud", "freigaben"), "gruppe")])
fall("verschaerfen-bestaetigung", ["A41"], "der Nutzer verlangt Bestätigung vor jeder Senkung", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"senkung": {"bestaetigung": True}})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, senkung("bestaetigung"), True)])
fall("verschaerfen-gleichstand", ["A28"],
     "der Nutzer setzt cloud-r auf intern, also gleich der Firma: angenommen, Herkunft bleibt firma", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "intern"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, herkunft("routen", "cloud-r", "klasse"), "firma")])
fall("nutzer-ruecknahme", ["A26"],
     "der Nutzer verschärft cloud-r auf oeffentlich und nimmt das zurück; die Grenze ist die Ebene darüber", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {})), Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, angenommen, True), (2, klasse("cloud-r"), "intern")])

# Lockerung je Feldart: abgelehnt, Stand unverändert
fall("lockerung-klasse-hoeher", ["A26"], "der Nutzer hebt cloud-r auf kundendaten: abgelehnt, der Stand bleibt", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, klasse("cloud-r"), "intern"), (1, hinweise, [])])
fall("lockerung-gesperrt-false", ["A26", "A45"], "die Firma sperrt cloud-s; der Nutzer setzt gesperrt false",
     {"firma": firma(routen_status=STATUS)},
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-s", "gesperrt": False}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, klasse("cloud-s"), "gesperrt")])
fall("lockerung-erlaubt-mehr", ["A26", "A51"], "der Nutzer erlaubt cloud-r für k-zwei", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"kunden": [{"kunde": "k-zwei", "erlaubt": ["cloud-r", "lokal-l"]}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, kunde("k-zwei", "erlaubt"), ["lokal-l"])])
fall("lockerung-freigabe-hinzu", ["A50"], "der Nutzer versucht eine Freigabe hinzuzufügen; das Feld gibt es nicht", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"freigaben": [{"route": "cloud-s", "kunde": "k-zwei"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.schluessel_unbekannt"), (1, freigaben, [{"route": "cloud-r", "kunde": "k-beispiel"}])],
     form={0: "ebenen.schluessel_unbekannt"})
fall("lockerung-aus-false", ["A41"], "die Firma schaltet die Senkung aus; die Gruppe setzt aus false",
     {"firma": firma(senkung={"aus": True, "bestaetigung": False, "arten": ["regeln"]})},
     [A("ps_00000000000000000000000000000b01", gruppe("g-eins", {"senkung": {"aus": False}})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, senkung("aus"), True)])
fall("lockerung-gegen-gruppe", ["A26"],
     "die Gruppe senkt cloud-r auf oeffentlich; der Nutzer setzt intern (gleich der Firma, lockerer als die Gruppe)",
     {"firma": FIRMA, "gruppen": {"g-eins": gruppe("g-eins", {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]})}},
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "intern"}]})), Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, klasse("cloud-r"), "oeffentlich")])
fall("lockerung-gegen-nutzerregel-der-firma", ["A26", "A128"],
     "die Firma setzt für u cloud-r auf oeffentlich; u setzt intern: abgelehnt, Herkunft bleibt firma",
     {"firma": firma(nutzerregeln=[{"nutzer": "ps_00000000000000000000000000000c01", "regeln": {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]}}])},
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "intern"}]})), Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, code, "ebenen.lockerung"), (1, klasse("cloud-r"), "oeffentlich"),
      (1, herkunft("routen", "cloud-r", "klasse"), "firma"), (2, klasse("cloud-r"), "intern")])
fall("lockerung-nutzerregel-in-firmendatei", ["A26"],
     "ein Admin reicht eine Nutzerregel ein, die lockerer ist als die Firmengrundregel", ST,
     [A("ps_00000000000000000000000000000b01", firma(nutzerregeln=[{"nutzer": "ps_00000000000000000000000000000c01", "regeln": {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}]}}])),
      Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.lockerung"), (1, klasse("cloud-r"), "intern")])

# Laden: veraltete Angaben ergeben den strengeren Wert und einen Hinweis
fall("laden-veraltet-lockerer", ["A26"],
     "die Nutzerdatei nennt cloud-r kundendaten (nach einer Firmenänderung lockerer als oben): es gilt intern, mit Hinweis",
     {"firma": FIRMA, "nutzer": {"ps_00000000000000000000000000000c01": nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}],
                                                                       "kunden": [{"kunde": "k-zwei", "erlaubt": ["cloud-s", "lokal-l"]}]})}},
     [Z("ps_00000000000000000000000000000c01")],
     [(0, klasse("cloud-r"), "intern"), (0, kunde("k-zwei", "erlaubt"), ["lokal-l"]),
      (0, hinweise, ["ebenen.lockerung_ignoriert", "ebenen.lockerung_ignoriert"])])
fall("laden-klasse-unbekannt-sperrt", ["A26", "A42"],
     "die Nutzerdatei nennt für cloud-r eine Klasse, die es nicht mehr gibt: cloud-r ist für u gesperrt",
     {"firma": FIRMA, "nutzer": {"ps_00000000000000000000000000000c01": nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "geheim"}]})}},
     [Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, klasse("cloud-r"), "gesperrt"), (0, hinweise, ["ebenen.klasse_unbekannt_gesperrt"]),
      (0, freigaben, []), (1, klasse("cloud-r"), "intern")])
fall("laden-bezug-unbekannt", ["A26"],
     "die Gruppendatei nennt eine Route und einen Kunden, die es nicht mehr gibt: wirkungslos, mit Hinweis",
     {"firma": FIRMA, "gruppen": {"g-eins": gruppe("g-eins", {"routen": [{"route": "alt-route", "gesperrt": True}],
                                                             "kunden": [{"kunde": "k-alt", "verboten": ["cloud-r"]}]})}},
     [Z("ps_00000000000000000000000000000c01")],
     [(0, klasse("cloud-r"), "intern"),
      (0, hinweise, ["ebenen.kunde_unbekannt_ignoriert", "ebenen.route_unbekannt_ignoriert"])])

# Fail closed beim Laden
fall("laden-nutzerdatei-ungueltig", ["A26", "A139"],
     "die Nutzerdatei von u hat einen unbekannten Schlüssel: u bekommt keine Einstellung, v ist nicht betroffen",
     {"firma": FIRMA, "nutzer": {"ps_00000000000000000000000000000c01": dict(nutzer("ps_00000000000000000000000000000c01", {}), notiz="x")}},
     [Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, code, "ebenen.schluessel_unbekannt"), (1, klasse("cloud-r"), "intern")],
     form={0: "ebenen.schluessel_unbekannt"})
fall("laden-gruppendatei-falscher-platz", ["A26"],
     "auf dem Platz von g-eins liegt die Datei der Gruppe g-zwei",
     {"firma": FIRMA, "gruppen": {"g-eins": gruppe("g-zwei", {})}},
     [Z("ps_00000000000000000000000000000c01"), Z("ps_00000000000000000000000000000c02")],
     [(0, code, "ebenen.ebene_falsch"), (1, klasse("cloud-r"), "intern")])
fall("laden-einstellung-ungueltig", ["A26"],
     "die eingebettete Einstellung nennt eine unbekannte Klasse: der Code von Vertrag 2 geht unverändert durch",
     {"firma": firma(einstellung=dict(EINST, routen=[{"schluessel": "cloud-r", "art": "cloud", "klasse": "geheim"}],
                                      kunden=[], kundendaten_an_cloud={"vorgabe": "nie", "freigaben": []}))},
     [Z("ps_00000000000000000000000000000c01")],
     [(0, code, "einst.klasse_unbekannt")],
     form={0: "einst.klasse_unbekannt"})
fall("laden-nutzer-mehrfach", ["A26"], "u steht in zwei Gruppen",
     {"firma": firma(gruppen=[{"schluessel": "g-eins", "mitglieder": ["ps_00000000000000000000000000000c01"]},
                              {"schluessel": "g-zwei", "mitglieder": ["ps_00000000000000000000000000000c01", "ps_00000000000000000000000000000c02"]}])},
     [Z("ps_00000000000000000000000000000c02")],
     [(0, code, "ebenen.nutzer_mehrfach")])
fall("laden-keine-route", ["A26", "A42"],
     "u sperrt alle drei Routen: keine Einstellung, der Aufrufer startet nichts",
     {"firma": FIRMA, "nutzer": {"ps_00000000000000000000000000000c01": nutzer("ps_00000000000000000000000000000c01", {"routen": [
         {"route": "cloud-r", "gesperrt": True}, {"route": "cloud-s", "gesperrt": True}, {"route": "lokal-l", "gesperrt": True}]})}},
     [Z("ps_00000000000000000000000000000c01")],
     [(0, code, "ebenen.keine_route")])

# Befugnis und Bezüge der Änderung
fall("befugnis-selbst-admin", ["A27", "A135"],
     "u reicht eine Firmendatei ein, die u als Admin nennt: abgelehnt, die gespeicherte Datei entscheidet", ST,
     [A("ps_00000000000000000000000000000c01", firma(rollen={"ps_00000000000000000000000000000b01": ["admin"], "ps_00000000000000000000000000000b02": ["admin"], "ps_00000000000000000000000000000c01": ["admin"]})),
      A("ps_00000000000000000000000000000c01", gruppe("g-eins", {}))],
     [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.nicht_befugt")])
fall("befugnis-fremde-nutzerdatei", ["A27"], "u schreibt die Nutzerdatei von v", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c02", {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]})), Z("ps_00000000000000000000000000000c02")],
     [(0, code, "ebenen.nicht_befugt"), (1, klasse("cloud-r"), "intern")])
fall("befugnis-admin-nutzerdatei", ["A27"],
     "ein Admin verschärft die Nutzerdatei von u; eine Lockerung lehnt die Prüfung auch beim Admin ab", ST,
     [A("ps_00000000000000000000000000000b02", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "oeffentlich"}]})),
      A("ps_00000000000000000000000000000b02", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}]})),
      Z("ps_00000000000000000000000000000c01")],
     [(0, angenommen, True), (1, code, "ebenen.lockerung"), (2, klasse("cloud-r"), "oeffentlich")])
fall("firma-ohne-admin", ["A27"], "ein Admin reicht eine Firmendatei ohne Admin ein", ST,
     [A("ps_00000000000000000000000000000b01", firma(rollen={"ps_00000000000000000000000000000b01": ["einsicht"]})), A("ps_00000000000000000000000000000b02", gruppe("g-eins", {})),
      A("ps_00000000000000000000000000000b01", firma(rollen={}))],
     [(0, code, "ebenen.anzahl"), (1, angenommen, True), (2, code, "ebenen.anzahl")],
     form={0: "ebenen.anzahl", 2: "ebenen.anzahl"})
fall("rollen-form", ["A27", "A12"],
     "Rollen der Firmendatei: ein Name statt eines Pseudonyms, eine unbekannte Rolle, ein Doppel und das alte Feld admins sind Formfehler",
     ST,
     [A("ps_00000000000000000000000000000b01", firma(rollen={"ps_00000000000000000000000000000b01": ["admin"], "max.beispiel": ["einsicht"]})),
      A("ps_00000000000000000000000000000b01", firma(rollen={"ps_00000000000000000000000000000b01": ["admin", "root"]})),
      A("ps_00000000000000000000000000000b01", firma(rollen={"ps_00000000000000000000000000000b01": ["admin", "admin"]})),
      A("ps_00000000000000000000000000000b01", firma(admins=["ps_00000000000000000000000000000b01"]))],
     [(0, code, "ebenen.kennung_form"), (1, code, "ebenen.wert_ungueltig"), (2, code, "ebenen.kennung_doppelt"),
      (3, code, "ebenen.schluessel_unbekannt")],
     form={0: "ebenen.kennung_form", 1: "ebenen.wert_ungueltig", 2: "ebenen.kennung_doppelt", 3: "ebenen.schluessel_unbekannt"})
fall("gruppe-unbekannt", ["A27"], "ein Admin schreibt eine Gruppe, die die Firma nicht kennt", ST,
     [A("ps_00000000000000000000000000000b01", gruppe("g-drei", {}))],
     [(0, code, "ebenen.gruppe_unbekannt")])
fall("aendern-bezug-unbekannt", ["A26"],
     "der Nutzer nennt eine unbekannte Route, dann einen unbekannten Kunden, dann eine unbekannte Klasse; die Bezüge gehen der Lockerung vor", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "kundendaten"}, {"route": "neu-x", "gesperrt": True}]})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"kunden": [{"kunde": "k-neu", "verboten": ["cloud-r"]}]})),
      A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"routen": [{"route": "cloud-r", "klasse": "geheim"}]}))],
     [(0, code, "ebenen.route_unbekannt"), (1, code, "ebenen.kunde_unbekannt"), (2, code, "ebenen.klasse_unbekannt")])
fall("nutzer-senkung-arten", ["A41"], "der Nutzer versucht, die Arten zu setzen; das darf nur die Gruppe", ST,
     [A("ps_00000000000000000000000000000c01", nutzer("ps_00000000000000000000000000000c01", {"senkung": {"arten": ["regeln"]}}))],
     [(0, code, "ebenen.schluessel_unbekannt")],
     form={0: "ebenen.schluessel_unbekannt"})


def main():
    fehler = []
    ausgabe = []
    for f in FAELLE:
        erg = ausfuehren(f)
        for i, prob, soll in f["_kern"]:
            try:
                ist = prob(erg["schritte"][i])
            except (KeyError, TypeError, IndexError) as e:
                ist = f"<{type(e).__name__}: {e}>"
            if ist != soll:
                fehler.append(f"{f['name']} Schritt {i}: Referenz {ist!r}, Tabelle {soll!r}")
        sichtbar = {k: v for k, v in f.items() if not k.startswith("_")}
        ausgabe.append((f["name"], sichtbar, erg))
    if fehler:
        for z in fehler:
            print("FEHLER", z)
        return 1
    for ordner in ("faelle", "erwartet"):
        shutil.rmtree(HIER / ordner, ignore_errors=True)
        (HIER / ordner).mkdir()
    for name, sichtbar, erg in ausgabe:
        (HIER / "faelle" / f"{name}.json").write_text(json.dumps(sichtbar, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(json.dumps(erg, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(ausgabe)} Fälle geschrieben, Kernwerte stimmen")
    return 0


if __name__ == "__main__":
    sys.exit(main())
