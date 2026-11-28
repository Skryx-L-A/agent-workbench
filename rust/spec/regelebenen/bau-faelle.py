#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags Regelebenen (faelle/, erwartet/).
Dazu die Fälle zu setzen und sicht (faelle-setzen/) und zum Schreibweg der Firmenvorgabe (faelle-schreiben/).

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
            # Fassung 2 (README 15.2, Entscheid des Betreibers, 27.11.2026): Trägt der Stand admins, ist das die einzige
            # Quelle. Der Dienst leitet daraus die Rolle admin in rollen ab; eine Datei ändert die Admins nie.
            if "admins" in stand:
                admins = list(stand["admins"])
            else:
                admins = [p for p, rollen in stand["firma"]["rollen"].items() if "admin" in rollen]
            if ebene == "firma":
                firma_intern(datei)
                if wer not in admins:
                    raise Abgelehnt("ebenen.nicht_befugt")
                if "admins" in stand and sorted(p for p, r in datei["rollen"].items() if "admin" in r) != sorted(admins):
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

fall("a45-freigabe-person", ["A45", "A12"],
     "freigabe_durch nennt eine Rolle der Firma, nie eine Person: ein Pseudonym dort ist ein Formfehler",
     ST,
     [A("ps_00000000000000000000000000000b01", firma(routen_status=[dict(STATUS[0], freigabe_durch="ps_00000000000000000000000000000b01")])),
      A("ps_00000000000000000000000000000b01", firma(routen_status=STATUS))],
     [(0, code, "ebenen.kennung_form"), (1, angenommen, True)],
     form={0: "ebenen.kennung_form"})

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
# Fassung 2: eine Quelle der Admins (README 15.2; Entscheid des Betreibers, 27.11.2026). Der Stand trägt admins aus der
# Kontenliste der Firmenvorgabe; die Rolle admin in rollen folgt ihr, eine eingereichte Datei ändert sie nie.
_B = ["ps_00000000000000000000000000000b01", "ps_00000000000000000000000000000b02"]
fall("admins-eine-quelle", ["A27", "E1", "B09"],
     "der Stand nennt die Admins aus der Kontenliste; Admin 1 reicht eine Firmendatei mit einem dritten Admin und eine ohne Admin 2 ein: "
     "beide abgelehnt; mit denselben Admins und einer Rolle einsicht angenommen; der dritte ändert danach keine Gruppe",
     {"firma": FIRMA, "admins": _B},
     [A("ps_00000000000000000000000000000b01", firma(rollen={_B[0]: ["admin"], _B[1]: ["admin"], "ps_00000000000000000000000000000b03": ["admin"]})),
      A("ps_00000000000000000000000000000b01", firma(rollen={_B[0]: ["admin"], _B[1]: ["einsicht"]})),
      A("ps_00000000000000000000000000000b01", firma(rollen={_B[0]: ["admin", "einsicht"], _B[1]: ["admin"],
                                                              "ps_00000000000000000000000000000c02": ["einsicht"]})),
      A("ps_00000000000000000000000000000b03", gruppe("g-eins", {})),
      A("ps_00000000000000000000000000000b02", gruppe("g-eins", {}))],
     [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.nicht_befugt"), (2, angenommen, True),
      (3, code, "ebenen.nicht_befugt"), (4, angenommen, True)])
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


# --- Fassung 2: Einstellungen im Lauf (Nachtrag 26.11.2026, README Abschnitt 15) --------------------------
# Referenz von setzen() und sicht(). Die Fälle stehen in faelle-setzen/ und erwartet-setzen/.

MODELL = re.compile(r"^[A-Za-z0-9._:/@+-]{1,128}$")
GRUPPE_EBENE = re.compile(r"^gruppe ([a-z0-9][a-z0-9._-]{0,63})$")
HARNESSES = ("claude", "copilot", "pi")
# Werte je Harness, wörtlich aus der Hilfe der Programme (Vertrag 2, Abschnitt „Block effort“).
EFFORTS = {
    "claude": ("low", "medium", "high", "xhigh", "max"),
    "copilot": ("none", "minimal", "low", "medium", "high", "xhigh", "max"),
    "pi": ("off", "minimal", "low", "medium", "high", "xhigh", "max"),
}
ALLE_EFFORTS = tuple(sorted({w for ws in EFFORTS.values() for w in ws}))
ALLE = ("firma", "gruppe", "nutzer")

# Katalog des Pilots: Bereich, Ebenen, Typ, Vorgabe, Richtung
KATALOG = {
    "aussehen.thema": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("wahl", ("system", "hell", "dunkel")),
                       "vorgabe": "system", "richtung": "frei"},
    "aussehen.farbsatz": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("wahl", ("grau", "blauschwarz")),
                          "vorgabe": "grau", "richtung": "frei"},
    "aussehen.terminal": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("wahl", ("dunkel", "hell", "thema")),
                          "vorgabe": "dunkel", "richtung": "frei"},
    "terminal.schriftgroesse": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("ganzzahl", 9, 32),
                                "vorgabe": 13, "richtung": "frei"},
    "sitzung.harness": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("wahl", HARNESSES),
                        "vorgabe": None, "richtung": "frei"},
    "sitzung.modell": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("modell",),
                       "vorgabe": None, "richtung": "frei"},
    "sitzung.effort": {"bereich": "nutzer", "ebenen": ALLE, "typ": ("wahl", ALLE_EFFORTS),
                       "vorgabe": None, "richtung": "frei"},
    "harnesses": {"bereich": "admin", "ebenen": ALLE, "typ": ("klassenliste", "harness"),
                  "vorgabe": {}, "richtung": "schnitt"},
    "modelle": {"bereich": "admin", "ebenen": ALLE, "typ": ("klassenliste", "modell"),
                "vorgabe": {}, "richtung": "schnitt"},
    "effort.grenzen": {"bereich": "admin", "ebenen": ("firma",), "typ": ("grenzen",),
                       "vorgabe": [], "richtung": "nur_firma"},
    "optionen.copilot_ohne_rueckfrage": {"bereich": "admin", "ebenen": ALLE, "typ": ("schalter",),
                                         "vorgabe": False, "richtung": "aus_strenger"},
    # Optionen nach A30 für den Pilot (Entscheid des Betreibers, 27.11.2026), alle mit Vorgabe aus.
    "optionen.herabstufen": {"bereich": "admin", "ebenen": ALLE, "typ": ("schalter",),
                             "vorgabe": False, "richtung": "aus_strenger"},
    "optionen.lokale_ca": {"bereich": "admin", "ebenen": ("firma",), "typ": ("schalter",),
                           "vorgabe": False, "richtung": "nur_firma"},
    "optionen.copilot_telemetrie": {"bereich": "admin", "ebenen": ALLE, "typ": ("schalter",),
                                    "vorgabe": False, "richtung": "aus_strenger"},
    "optionen.copilot_eingeschraenktes_token": {"bereich": "admin", "ebenen": ("firma",), "typ": ("schalter",),
                                                "vorgabe": False, "richtung": "nur_firma"},
    "optionen.herstellerzugang": {"bereich": "admin", "ebenen": (), "typ": ("schalter",),
                                  "vorgabe": False, "richtung": "fest"},
    # Leseklasse je Person oder Gruppe (Projektentscheid, 27.11.2026). Die Wirkung (P2, P4) rechnet nur datentor.
    "leseklassen": {"bereich": "admin", "ebenen": ("firma",), "typ": ("leseklassen",),
                    "vorgabe": [], "richtung": "nur_firma"},
    # Fristen der Freigabe, die die Stelle heute aus Vertrag 2 (Block mcp) auswertet (Entscheid des Betreibers, 27.11.2026;
    # README Abschnitt 15.1.2). Grenzen und Vorgaben wie Vertrag 2; die Einloesefrist darf ein Admin bis 24 h erhoehen.
    "aufsicht.freigabefrist_ms": {"bereich": "admin", "ebenen": ("firma",), "typ": ("ganzzahl", 1, 86400000),
                                  "vorgabe": 60000, "richtung": "nur_firma"},
    "aufsicht.einloesefrist_ms": {"bereich": "admin", "ebenen": ("firma",), "typ": ("ganzzahl", 1, 86400000),
                                  "vorgabe": 300000, "richtung": "nur_firma"},
}
FRISTEN = [s for s in KATALOG if s.startswith("aufsicht.")]
OPTIONEN = [s for s in KATALOG if s.startswith("optionen.")]
LESE_MAX = 4096
PSEUDONYM = re.compile(r"^ps_[0-9a-f]{32}$")
SCHLUESSEL = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
GRUND_TEXT = {"nur_admin": "Vorgabe der Firma", "nur_strenger": "Nur strenger als die Vorgabe", "fest": "Im Pilot fest aus"}


def ist_ganzzahl(w):
    return isinstance(w, int) and not isinstance(w, bool)


def wert_gueltig(eintrag, wert, klassen, gruppen=None):
    art = eintrag["typ"][0]
    if art == "wahl":
        return isinstance(wert, str) and wert in eintrag["typ"][1]
    if art == "ganzzahl":
        return ist_ganzzahl(wert) and eintrag["typ"][1] <= wert <= eintrag["typ"][2]
    if art == "modell":
        return isinstance(wert, str) and bool(MODELL.match(wert))
    if art == "schalter":
        return isinstance(wert, bool)
    if art == "klassenliste":
        if not isinstance(wert, dict) or len(wert) > len(klassen):
            return False
        for k, liste in wert.items():
            if k not in klassen or not isinstance(liste, list) or len(liste) > 16 or len(set(map(str, liste))) != len(liste):
                return False
            for x in liste:
                if not isinstance(x, str):
                    return False
                if eintrag["typ"][1] == "harness" and x not in HARNESSES:
                    return False
                if eintrag["typ"][1] == "modell" and not MODELL.match(x):
                    return False
        return True
    if art == "grenzen":
        if not isinstance(wert, list) or len(wert) > 64:
            return False
        gesehen = set()
        for g in wert:
            if not isinstance(g, dict) or not {"harness", "erlaubt"} <= set(g) or not set(g) <= {"harness", "klasse", "modell", "erlaubt"}:
                return False
            h = g["harness"]
            if h not in HARNESSES:
                return False
            if "klasse" in g and g["klasse"] not in klassen:
                return False
            if "modell" in g and not (isinstance(g["modell"], str) and MODELL.match(g["modell"])):
                return False
            e = g["erlaubt"]
            if not isinstance(e, list) or not 1 <= len(e) <= 7 or len(set(map(str, e))) != len(e):
                return False
            if any(not isinstance(x, str) or x not in EFFORTS[h] for x in e):
                return False
            schl = (h, g.get("klasse"), g.get("modell"))
            if schl in gesehen:
                return False
            gesehen.add(schl)
        return True
    if art == "leseklassen":
        # Form wie datentor/person Abschnitt 3; bei ebene nutzer steht hier schon das Pseudonym (Abschnitt 15.2).
        if not isinstance(wert, list) or len(wert) > LESE_MAX:
            return False
        paare = set()
        for x in wert:
            if not isinstance(x, dict) or set(x) != {"ebene", "wer", "klasse"}:
                return False
            if not all(isinstance(x[k], str) for k in x):
                return False
            if x["ebene"] == "gruppe":
                if not SCHLUESSEL.match(x["wer"]) or x["wer"] not in (gruppen or {}):
                    return False
            elif x["ebene"] == "nutzer":
                if not PSEUDONYM.match(x["wer"]):
                    return False
            else:
                return False
            if x["klasse"] not in klassen:
                return False
            if (x["ebene"], x["wer"]) in paare:
                return False
            paare.add((x["ebene"], x["wer"]))
        return True
    raise AssertionError(art)


def gruppe_des(stand, wer):
    for g, mitglieder in stand["gruppen"].items():
        if wer in mitglieder:
            return g
    return None


def ebenen_fuer(stand, ziel_art, ziel):
    """Die Ebenen vom Platz firma bis zum Platz der Änderung, je (Herkunft, Werte)."""
    w = stand["werte"]
    kette = [("firma", w["firma"])]
    if ziel_art == "gruppe":
        kette.append((f"gruppe {ziel}", w["gruppen"].get(ziel, {})))
    elif ziel_art == "nutzer":
        g = gruppe_des(stand, ziel)
        if g is not None:
            kette.append((f"gruppe {g}", w["gruppen"].get(g, {})))
        kette.append(("nutzer", w["nutzer"].get(ziel, {})))
    return kette


def zusammen(schluessel, kette, klassen):
    """Wirksamer Wert und Herkunft über eine Kette von Ebenen (oben zuerst)."""
    e = KATALOG[schluessel]
    wert, herkunft = copy.deepcopy(e["vorgabe"]), "firma"
    for name, werte in kette:
        if schluessel not in werte:
            continue
        neu = werte[schluessel]
        if e["richtung"] == "fest":
            continue
        if e["richtung"] == "frei":
            wert, herkunft = neu, name
        elif e["richtung"] == "nur_firma":
            if name == "firma":
                wert, herkunft = neu, name
        elif e["richtung"] == "aus_strenger":
            vorher = wert
            wert = wert and neu if name != "firma" else neu
            if wert != vorher:
                herkunft = name
        elif e["richtung"] == "schnitt":
            vorher = copy.deepcopy(wert)
            if name == "firma":
                wert = copy.deepcopy(neu)
            else:
                for k, liste in neu.items():
                    if k in wert:
                        wert[k] = [x for x in wert[k] if x in liste]
                    else:
                        wert[k] = list(liste)
            if wert != vorher:
                herkunft = name
    if e["richtung"] == "schnitt":
        wert = {k: wert[k] for k in klassen if k in wert}
    return wert, herkunft


def ist_lockerung(schluessel, oben, neu):
    r = KATALOG[schluessel]["richtung"]
    if r == "aus_strenger":
        return neu is True and oben is False
    if r == "schnitt":
        return any(k in oben and any(x not in oben[k] for x in liste) for k, liste in neu.items())
    return False


def zeile_setzen(wer, ebene, ziel, schluessel, alt, neu, code):
    # Form aus Vertrag 5, Abschnitt 6.6, gefüllt nach README Abschnitt 15.6.
    def h(wert):
        return kanonisch_hash({"schluessel": schluessel, "wert": wert})
    return {"aktionsname": "regel.abgelehnt" if code else "regel.geaendert",
            "alt": h(alt[1]) if alt[0] else None, "antrag": None, "code": code, "ebene": ebene,
            "ergebnis": "abgelehnt" if code else "geaendert", "handelnd": wer,
            "neu": h(neu), "pflicht": None, "ziel": ziel, "zweiter": None}


def setzen(stand, wer, bereich, ebene, schluessel, wert):
    ziel_art, ziel, platz = None, None, None
    if ebene == "firma":
        ziel_art, platz = "firma", stand["werte"]["firma"]
    elif ebene == "nutzer":
        ziel_art, ziel = "nutzer", wer
        platz = stand["werte"]["nutzer"].get(wer)
    elif isinstance(ebene, str) and GRUPPE_EBENE.match(ebene):
        ziel_art, ziel = "gruppe", GRUPPE_EBENE.match(ebene).group(1)
        platz = stand["werte"]["gruppen"].get(ziel)
    alt = (True, platz[schluessel]) if isinstance(platz, dict) and isinstance(schluessel, str) and schluessel in platz else (False, None)
    code = None
    try:
        if not isinstance(schluessel, str) or schluessel not in KATALOG:
            raise Abgelehnt("ebenen.einstellung_unbekannt")
        e = KATALOG[schluessel]
        if bereich != e["bereich"]:
            raise Abgelehnt("ebenen.bereich_falsch")
        if ziel_art is None:
            raise Abgelehnt("ebenen.ebene_unbekannt")
        if ziel_art == "gruppe" and ziel not in stand["gruppen"]:
            raise Abgelehnt("ebenen.gruppe_unbekannt")
        if ziel_art not in e["ebenen"]:
            raise Abgelehnt("ebenen.ebene_gesperrt")
        if ziel_art in ("firma", "gruppe") and wer not in stand["admins"]:
            raise Abgelehnt("ebenen.nicht_befugt")
        # leseklassen kennt kein null, auch nicht als Rücknahme; „niemand liest“ ist die leere Liste (Abschnitt 15.11).
        if (wert is not None or e["typ"][0] == "leseklassen") and not wert_gueltig(e, wert, stand["klassen"], stand["gruppen"]):
            raise Abgelehnt("ebenen.wert_ungueltig")
        if wert is not None and ziel_art != "firma":
            kette = ebenen_fuer(stand, ziel_art, ziel)[:-1]
            oben, _ = zusammen(schluessel, kette, stand["klassen"])
            if ist_lockerung(schluessel, oben, wert):
                raise Abgelehnt("ebenen.lockerung")
        if wert is not None and schluessel in ("sitzung.harness", "sitzung.modell"):
            liste = "harnesses" if schluessel == "sitzung.harness" else "modelle"
            kette = ebenen_fuer(stand, ziel_art, ziel)
            grenze, _ = zusammen(liste, kette, stand["klassen"])
            if not any(k not in grenze or wert in grenze[k] for k in stand["klassen"]):
                raise Abgelehnt("ebenen.ausser_grenze")
    except Abgelehnt as a:
        code = a.code
    zeile = zeile_setzen(wer, ziel_art, ziel, schluessel, alt, wert, code)
    if code is None:
        if ziel_art == "firma":
            ort = stand["werte"]["firma"]
        elif ziel_art == "gruppe":
            ort = stand["werte"]["gruppen"].setdefault(ziel, {})
        else:
            ort = stand["werte"]["nutzer"].setdefault(ziel, {})
        if wert is None:
            ort.pop(schluessel, None)
        else:
            ort[schluessel] = copy.deepcopy(wert)
    return {"angenommen": code is None, "code": code, "zeile": zeile}


def sicht(stand, wer):
    admin = wer in stand["admins"]
    kette = ebenen_fuer(stand, "nutzer", wer)
    aus = {}
    for s, e in KATALOG.items():
        wert, herkunft = zusammen(s, kette, stand["klassen"])
        if e["richtung"] == "fest":
            aenderbar, grund = False, "fest"
        elif admin or (e["bereich"] == "nutzer" and "nutzer" in e["ebenen"]):
            aenderbar, grund = True, None
        elif "nutzer" not in e["ebenen"]:
            aenderbar, grund = False, "nur_admin"
        elif e["richtung"] == "aus_strenger" and wert is False:
            aenderbar, grund = False, "nur_admin"
        else:
            aenderbar, grund = True, "nur_strenger"
        if e["typ"][0] == "leseklassen" and not admin:
            # Nur die Einträge, die den Betrachter nennen: sein eigener und der seiner Gruppe. Keine Rangfolge (datentor P4).
            g = gruppe_des(stand, wer)
            wert = [x for x in wert if (x["ebene"], x["wer"]) in (("nutzer", wer), ("gruppe", g))]
        aus[s] = {"wert": wert, "herkunft": herkunft, "aenderbar": aenderbar, "grund": grund}
    return {"einstellungen": aus}


def wirksame_fristen(katalog, server):
    """Wirksame Frist je MCP-Server (README Abschnitt 15.1.2, Entscheid des Betreibers, 27.11.2026): der kleinere Wert aus
    Katalog und Firmendatei; kürzer ist strenger. Ein Server ohne eigenen Wert (None) bekommt den Katalogwert. Bei gleichem
    Wert gilt der Wert des Servers (herkunft firmendatei): Der Katalog lockert ihn nicht und verschärft ihn nicht."""
    aus = []
    for s in server:
        eigen = s["wert"]
        if eigen is not None and eigen <= katalog:
            aus.append({"server": s["server"], "wert": eigen, "herkunft": "firmendatei"})
        else:
            aus.append({"server": s["server"], "wert": katalog, "herkunft": "katalog"})
    return aus


def ausfuehren_setzen(fall):
    stand = copy.deepcopy(fall["stand"])
    erg = []
    for s in fall["schritte"]:
        if "sicht" in s:
            erg.append(sicht(stand, s["sicht"]["wer"]))
        elif "fristen" in s:
            a = s["fristen"]
            katalog = sicht(stand, a["wer"])["einstellungen"][a["schluessel"]]["wert"]
            erg.append({"fristen": wirksame_fristen(katalog, a["server"])})
        else:
            a = s["setzen"]
            erg.append(setzen(stand, a["wer"], a["bereich"], a["ebene"], a["schluessel"], a["wert"]))
    return {"schritte": erg}


ADM, ADM2, U, V = ("ps_00000000000000000000000000000b01", "ps_00000000000000000000000000000b02",
                   "ps_00000000000000000000000000000c01", "ps_00000000000000000000000000000c02")


def stand_lauf(firma=None, gruppen=None, nutzer=None):
    # admins: vom Dienst aus der Kontenliste der Firmenvorgabe abgeleitet (Abschnitt 15.2); nur Pseudonyme.
    return {"klassen": ["oeffentlich", "intern", "kundendaten"], "admins": [ADM, ADM2],
            "gruppen": {"g-eins": [U], "g-zwei": [V]},
            "werte": {"firma": firma or {}, "gruppen": gruppen or {}, "nutzer": nutzer or {}}}


def S(wer, bereich, ebene, schluessel, wert):
    return {"setzen": {"wer": wer, "bereich": bereich, "ebene": ebene, "schluessel": schluessel, "wert": wert}}


def SI(wer):
    return {"sicht": {"wer": wer}}


def FR(wer, schluessel, *server):
    return {"fristen": {"wer": wer, "schluessel": schluessel,
                        "server": [{"server": n, "wert": w} for n, w in server]}}


def frist_von(server, teil):
    return lambda e: next(x[teil] for x in e["fristen"] if x["server"] == server)


def feld(schluessel, teil):
    return lambda e: e["einstellungen"][schluessel][teil]


FAELLE_SETZEN = []


def fall_s(name, anforderungen, beschreibung, stand, schritte, kern):
    FAELLE_SETZEN.append({"name": name, "anforderungen": anforderungen, "beschreibung": beschreibung,
                          "stand": stand, "schritte": schritte, "_kern": kern})


GRENZEN = [{"harness": "copilot", "erlaubt": ["low", "medium", "high"]},
           {"harness": "copilot", "klasse": "kundendaten", "erlaubt": ["low"]},
           {"harness": "pi", "modell": "qwen3:8b", "erlaubt": ["off", "low", "medium"]}]

fall_s("setzen-nutzer-angenommen", ["A26", "E3"],
       "der Nutzer wählt den Farbsatz blauschwarz und die Schriftgröße 15, dann nimmt er die Schriftgröße zurück (wert null)",
       stand_lauf(),
       [S(U, "nutzer", "nutzer", "aussehen.farbsatz", "blauschwarz"), S(U, "nutzer", "nutzer", "terminal.schriftgroesse", 15),
        S(U, "nutzer", "nutzer", "terminal.schriftgroesse", None), SI(U), SI(V)],
       [(0, angenommen, True), (1, angenommen, True), (2, angenommen, True),
        (3, feld("aussehen.farbsatz", "wert"), "blauschwarz"), (3, feld("aussehen.farbsatz", "herkunft"), "nutzer"),
        (3, feld("terminal.schriftgroesse", "wert"), 13), (3, feld("terminal.schriftgroesse", "herkunft"), "firma"),
        (4, feld("aussehen.farbsatz", "wert"), "grau")])
fall_s("setzen-admin-angenommen", ["A27", "A30", "E1", "E2"],
       "ein Admin setzt die Grenzen des Efforts und schaltet die Option an; ein Nutzer sieht beides mit Herkunft firma",
       stand_lauf(),
       [S(ADM, "admin", "firma", "effort.grenzen", GRENZEN),
        S(ADM, "admin", "firma", "optionen.copilot_ohne_rueckfrage", True), SI(U), SI(ADM)],
       [(0, angenommen, True), (1, angenommen, True),
        (2, feld("effort.grenzen", "wert"), GRENZEN), (2, feld("effort.grenzen", "herkunft"), "firma"),
        (2, feld("effort.grenzen", "aenderbar"), False), (2, feld("effort.grenzen", "grund"), "nur_admin"),
        (2, feld("optionen.copilot_ohne_rueckfrage", "wert"), True),
        (2, feld("optionen.copilot_ohne_rueckfrage", "grund"), "nur_strenger"),
        (3, feld("effort.grenzen", "aenderbar"), True), (3, feld("effort.grenzen", "grund"), None)])
fall_s("herkunft-und-aenderbar", ["A28", "E3"],
       "Thema von der Firma, Schriftgröße von der Gruppe, Farbsatz vom Nutzer; harnesses von der Gruppe verschärft; die Option ist aus",
       stand_lauf(firma={"aussehen.thema": "hell", "harnesses": {"intern": ["copilot", "pi"]}},
                  gruppen={"g-eins": {"terminal.schriftgroesse": 15, "harnesses": {"intern": ["copilot"]}}},
                  nutzer={U: {"aussehen.farbsatz": "blauschwarz"}}),
       [SI(U), SI(V), SI(ADM)],
       [(0, feld("aussehen.thema", "herkunft"), "firma"), (0, feld("aussehen.thema", "wert"), "hell"),
        (0, feld("terminal.schriftgroesse", "herkunft"), "gruppe g-eins"), (0, feld("terminal.schriftgroesse", "wert"), 15),
        (0, feld("aussehen.farbsatz", "herkunft"), "nutzer"),
        (0, feld("harnesses", "wert"), {"intern": ["copilot"]}), (0, feld("harnesses", "herkunft"), "gruppe g-eins"),
        (0, feld("harnesses", "aenderbar"), True), (0, feld("harnesses", "grund"), "nur_strenger"),
        (0, feld("aussehen.thema", "aenderbar"), True), (0, feld("aussehen.thema", "grund"), None),
        (0, feld("optionen.copilot_ohne_rueckfrage", "aenderbar"), False),
        (0, feld("optionen.copilot_ohne_rueckfrage", "grund"), "nur_admin"),
        (0, feld("effort.grenzen", "aenderbar"), False),
        (1, feld("terminal.schriftgroesse", "herkunft"), "firma"), (1, feld("harnesses", "wert"), {"intern": ["copilot", "pi"]}),
        (2, feld("optionen.copilot_ohne_rueckfrage", "aenderbar"), True)])
fall_s("of9-nutzer-lockert", ["A26", "OF9"],
       "die Firma erlaubt in intern nur copilot und lässt die Option aus; der Nutzer nimmt claude dazu und schaltet die Option an; danach verschärft er auf leer",
       stand_lauf(firma={"harnesses": {"intern": ["copilot"]}}),
       [S(U, "admin", "nutzer", "harnesses", {"intern": ["copilot", "claude"]}),
        S(U, "admin", "nutzer", "optionen.copilot_ohne_rueckfrage", True),
        S(U, "admin", "nutzer", "harnesses", {"intern": []}), SI(U)],
       [(0, code, "ebenen.lockerung"), (1, code, "ebenen.lockerung"), (2, angenommen, True),
        (3, feld("harnesses", "wert"), {"intern": []}), (3, feld("harnesses", "herkunft"), "nutzer"),
        (3, feld("optionen.copilot_ohne_rueckfrage", "wert"), False)])
fall_s("of9-nichtadmin-schreibt-adminwert", ["A27", "E1", "OF9"],
       "ein Nutzer schreibt die Grenzen des Efforts und die Liste der Harnesses auf der Ebene der Firma und seiner Gruppe; der Stand bleibt",
       stand_lauf(),
       [S(U, "admin", "firma", "harnesses", {"intern": ["claude"]}),
        S(U, "admin", "gruppe g-eins", "optionen.copilot_ohne_rueckfrage", False),
        S(U, "nutzer", "firma", "aussehen.thema", "dunkel"), SI(U)],
       [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.nicht_befugt"), (2, code, "ebenen.nicht_befugt"),
        (3, feld("harnesses", "wert"), {}), (3, feld("aussehen.thema", "wert"), "system")])
fall_s("kein-admin-durch-angabe", ["A27", "E1", "B09"],
       "ein Nutzer versucht, sich über einen Schlüssel admins oder rollen zum Admin zu machen; danach bleibt die Firmenebene gesperrt",
       stand_lauf(),
       [S(U, "admin", "firma", "admins", [U]), S(U, "nutzer", "nutzer", "rollen", {U: ["admin"]}),
        S(U, "admin", "firma", "effort.grenzen", GRENZEN)],
       [(0, code, "ebenen.einstellung_unbekannt"), (1, code, "ebenen.einstellung_unbekannt"),
        (2, code, "ebenen.nicht_befugt")])
fall_s("setzen-bereich-falsch", ["OF9"],
       "das Thema gehört zum Bereich nutzer, die Option zum Bereich admin; ein falscher oder unbekannter Bereich wird abgelehnt",
       stand_lauf(),
       [S(U, "admin", "nutzer", "aussehen.thema", "hell"), S(ADM, "nutzer", "firma", "optionen.copilot_ohne_rueckfrage", True),
        S(U, "system", "nutzer", "aussehen.thema", "hell")],
       [(0, code, "ebenen.bereich_falsch"), (1, code, "ebenen.bereich_falsch"), (2, code, "ebenen.bereich_falsch")])
fall_s("setzen-ebene-unbekannt", ["A26"],
       "Ebenen außerhalb von firma, nutzer und gruppe <schluessel>",
       stand_lauf(),
       [S(ADM, "nutzer", "abteilung", "aussehen.thema", "hell"), S(ADM, "nutzer", "gruppe", "aussehen.thema", "hell"),
        S(ADM, "nutzer", "gruppe G-Eins", "aussehen.thema", "hell")],
       [(0, code, "ebenen.ebene_unbekannt"), (1, code, "ebenen.ebene_unbekannt"), (2, code, "ebenen.ebene_unbekannt")])
fall_s("setzen-gruppe-unbekannt", ["A27"], "ein Admin schreibt in eine Gruppe, die die Firma nicht kennt",
       stand_lauf(), [S(ADM, "nutzer", "gruppe g-drei", "aussehen.thema", "hell")],
       [(0, code, "ebenen.gruppe_unbekannt")])
fall_s("setzen-ebene-gesperrt", ["E2"],
       "die Grenzen des Efforts setzt nur die Firma: auf der Gruppe und beim Nutzer abgelehnt, auch für einen Admin",
       stand_lauf(),
       [S(ADM, "admin", "gruppe g-eins", "effort.grenzen", GRENZEN), S(U, "admin", "nutzer", "effort.grenzen", GRENZEN)],
       [(0, code, "ebenen.ebene_gesperrt"), (1, code, "ebenen.ebene_gesperrt")])
fall_s("setzen-wert-ungueltig", ["OF3", "OF4", "E2"],
       "Schriftgröße 40, Thema blau, ein Effort ultra in den Grenzen, ein Effort none für pi, eine unbekannte Klasse, die Option als Text",
       stand_lauf(),
       [S(U, "nutzer", "nutzer", "terminal.schriftgroesse", 40), S(U, "nutzer", "nutzer", "aussehen.thema", "blau"),
        S(ADM, "admin", "firma", "effort.grenzen", [{"harness": "copilot", "erlaubt": ["ultra"]}]),
        S(ADM, "admin", "firma", "effort.grenzen", [{"harness": "pi", "erlaubt": ["none"]}]),
        S(ADM, "admin", "firma", "harnesses", {"geheim": ["pi"]}),
        S(ADM, "admin", "firma", "optionen.copilot_ohne_rueckfrage", "an"),
        S(U, "nutzer", "nutzer", "terminal.schriftgroesse", True)],
       [(i, code, "ebenen.wert_ungueltig") for i in range(7)])
fall_s("setzen-ausser-grenze", ["E2", "OF9"],
       "die Firma erlaubt in jeder Klasse nur copilot und pi; der Nutzer wählt claude als Vorgabe, dann pi; ein Modell außerhalb der Liste",
       stand_lauf(firma={"harnesses": {"oeffentlich": ["copilot", "pi"], "intern": ["copilot", "pi"], "kundendaten": ["pi"]},
                         "modelle": {"oeffentlich": ["gpt-6-luna"], "intern": ["gpt-6-luna"], "kundendaten": ["qwen3:8b"]}}),
       [S(U, "nutzer", "nutzer", "sitzung.harness", "claude"), S(U, "nutzer", "nutzer", "sitzung.harness", "pi"),
        S(U, "nutzer", "nutzer", "sitzung.modell", "gpt-5.6-sol"), S(U, "nutzer", "nutzer", "sitzung.modell", "qwen3:8b"),
        S(U, "nutzer", "nutzer", "sitzung.effort", "medium"), SI(U)],
       [(0, code, "ebenen.ausser_grenze"), (1, angenommen, True), (2, code, "ebenen.ausser_grenze"), (3, angenommen, True),
        (4, angenommen, True), (5, feld("sitzung.harness", "wert"), "pi"), (5, feld("sitzung.effort", "wert"), "medium")])
fall_s("setzen-einstellung-unbekannt", ["OF9"], "ein Schlüssel, den der Katalog nicht kennt",
       stand_lauf(), [S(U, "nutzer", "nutzer", "aussehen.schrift", "gross"), S(U, "nutzer", "nutzer", 7, "x")],
       [(0, code, "ebenen.einstellung_unbekannt"), (1, code, "ebenen.einstellung_unbekannt")])
fall_s("setzen-gruppe-verschaerft", ["A26", "A27"],
       "ein Admin verschärft die Gruppe g-eins gegen die Firma und versucht dann eine Lockerung; Gleichstand ist keine Lockerung",
       stand_lauf(firma={"modelle": {"intern": ["gpt-6-luna", "gpt-5.6-sol"]}, "optionen.copilot_ohne_rueckfrage": True}),
       [S(ADM, "admin", "gruppe g-eins", "modelle", {"intern": ["gpt-6-luna"]}),
        S(ADM, "admin", "gruppe g-eins", "modelle", {"intern": ["gpt-6-luna", "claude-sonnet-5.5"]}),
        S(ADM, "admin", "gruppe g-eins", "optionen.copilot_ohne_rueckfrage", True),
        S(U, "admin", "nutzer", "optionen.copilot_ohne_rueckfrage", False), SI(U), SI(V)],
       [(0, angenommen, True), (1, code, "ebenen.lockerung"), (2, angenommen, True), (3, angenommen, True),
        (4, feld("modelle", "wert"), {"intern": ["gpt-6-luna"]}), (4, feld("modelle", "herkunft"), "gruppe g-eins"),
        (4, feld("optionen.copilot_ohne_rueckfrage", "wert"), False),
        (4, feld("optionen.copilot_ohne_rueckfrage", "herkunft"), "nutzer"),
        (5, feld("optionen.copilot_ohne_rueckfrage", "herkunft"), "firma"),
        (5, feld("modelle", "wert"), {"intern": ["gpt-6-luna", "gpt-5.6-sol"]})])


# Optionen nach A30 (Entscheid des Betreibers, 27.11.2026): je Option ein Fall „Nutzer setzt: abgelehnt“ und ein Fall
# „Admin setzt: angenommen“; der Herstellerzugang ist fest aus und nicht umstellbar, auch für Admins.
for _o in OPTIONEN:
    if KATALOG[_o]["richtung"] == "fest":
        continue
    _kurz = _o.split(".", 1)[1].replace("_", "-")
    _nutzer = "nutzer" in KATALOG[_o]["ebenen"]
    fall_s(f"option-{_kurz}-nutzer-abgelehnt", ["A30", "A31", "A26"],
           f"ein Nutzer schaltet {_o} auf der Firmenebene an (nicht befugt) und auf seiner Ebene an "
           + ("(Lockerung)" if _nutzer else "(Ebene gesperrt)") + "; der Wert bleibt aus",
           stand_lauf(),
           [S(U, "admin", "firma", _o, True), S(U, "admin", "nutzer", _o, True), SI(U)],
           [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.lockerung" if _nutzer else "ebenen.ebene_gesperrt"),
            (2, feld(_o, "wert"), False), (2, feld(_o, "herkunft"), "firma"),
            (2, feld(_o, "aenderbar"), False), (2, feld(_o, "grund"), "nur_admin")])
    fall_s(f"option-{_kurz}-admin-angenommen", ["A30", "A31", "A27"],
           f"ein Admin schaltet {_o} für die Firma an; der Nutzer sieht an mit Herkunft firma"
           + ("; er schaltet für sich wieder aus (strenger)" if _nutzer else "; ändern darf er ihn nicht"),
           stand_lauf(),
           [S(ADM, "admin", "firma", _o, True), SI(U), SI(ADM)]
           + ([S(U, "admin", "nutzer", _o, False), SI(U)] if _nutzer else [S(U, "admin", "nutzer", _o, False)]),
           [(0, angenommen, True), (1, feld(_o, "wert"), True), (1, feld(_o, "herkunft"), "firma"),
            (1, feld(_o, "aenderbar"), _nutzer), (1, feld(_o, "grund"), "nur_strenger" if _nutzer else "nur_admin"),
            (2, feld(_o, "aenderbar"), True), (2, feld(_o, "grund"), None)]
           + ([(3, angenommen, True), (4, feld(_o, "wert"), False), (4, feld(_o, "herkunft"), "nutzer")] if _nutzer
              else [(3, code, "ebenen.ebene_gesperrt")]))

# Fristen der Freigabe (Entscheid des Betreibers, 27.11.2026; README Abschnitt 15.1.2): je Frist ein Fall „Nutzer setzt:
# abgelehnt“ und ein Fall „Admin setzt: angenommen“ mit den Grenzen 1 und 86400000 (24 h).
for _f in FRISTEN:
    _kurz = _f.split(".", 1)[1].replace("_", "-")
    _vorgabe = KATALOG[_f]["vorgabe"]
    fall_s(f"frist-{_kurz}-nutzer-abgelehnt", ["A26", "A27"],
           f"ein Nutzer setzt {_f} auf der Firmenebene (nicht befugt) und auf seiner Ebene (Ebene gesperrt); die Vorgabe bleibt",
           stand_lauf(),
           [S(U, "admin", "firma", _f, 120000), S(U, "admin", "nutzer", _f, 120000), S(U, "nutzer", "nutzer", _f, 120000), SI(U)],
           [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.ebene_gesperrt"), (2, code, "ebenen.bereich_falsch"),
            (3, feld(_f, "wert"), _vorgabe), (3, feld(_f, "herkunft"), "firma"),
            (3, feld(_f, "aenderbar"), False), (3, feld(_f, "grund"), "nur_admin")])
    fall_s(f"frist-{_kurz}-admin-angenommen", ["A26", "A27"],
           f"ein Admin setzt {_f} für die Firma auf 24 h, die obere Grenze; 0 und mehr als 24 h sind ungültig, eine Gruppe "
           "darf die Frist nicht tragen; der Nutzer sieht den Wert mit Herkunft firma und darf ihn nicht ändern",
           stand_lauf(),
           [S(ADM, "admin", "firma", _f, 86400000), SI(U), SI(ADM), S(ADM, "admin", "firma", _f, 0),
            S(ADM, "admin", "firma", _f, 86400001), S(ADM, "admin", "gruppe g-eins", _f, 1), S(ADM, "admin", "firma", _f, None),
            SI(U)],
           [(0, angenommen, True), (1, feld(_f, "wert"), 86400000), (1, feld(_f, "herkunft"), "firma"),
            (1, feld(_f, "aenderbar"), False), (1, feld(_f, "grund"), "nur_admin"),
            (2, feld(_f, "aenderbar"), True), (2, feld(_f, "grund"), None),
            (3, code, "ebenen.wert_ungueltig"), (4, code, "ebenen.wert_ungueltig"), (5, code, "ebenen.ebene_gesperrt"),
            (6, angenommen, True), (7, feld(_f, "wert"), _vorgabe)])

# Wirksame Frist je MCP-Server (Entscheid des Betreibers, 27.11.2026; README Abschnitt 15.1.2): der kleinere Wert aus Katalog
# und Firmendatei. Der erste Fall ist genau der Fall des Entscheids: Katalog 120 s, Server x 30 s, wirksam 30 s mit Herkunft Server x.
_F, _E = "aufsicht.freigabefrist_ms", "aufsicht.einloesefrist_ms"
fall_s("frist-wirksam-server-strenger", ["A26", "A27"],
       "der Katalog sagt 120 s, die Firmendatei sagt für den Server x 30 s; wirksam sind 30 s mit Herkunft Firmendatei, Server x",
       stand_lauf(firma={_F: 120000}),
       [FR(U, _F, ("x", 30000)), FR(ADM, _F, ("x", 30000))],
       [(0, frist_von("x", "wert"), 30000), (0, frist_von("x", "herkunft"), "firmendatei"),
        (1, frist_von("x", "wert"), 30000), (1, frist_von("x", "herkunft"), "firmendatei")])
fall_s("frist-wirksam-je-server", ["A26", "A27"],
       "je Server der kleinere Wert: strenger als der Katalog gilt der Server, gleich gilt der Server, lockerer oder ohne eigenen Wert "
       "gilt der Katalog; ein Admin senkt den Katalog unter den Server, dann gilt der Katalog; zurückgenommen gilt die Vorgabe",
       stand_lauf(),
       [FR(U, _E, ("tickets", 60000), ("wiki", None), ("ablage", 3600000), ("kalender", 300000)),
        S(ADM, "admin", "firma", _E, 30000),
        FR(U, _E, ("tickets", 60000), ("wiki", None)),
        S(ADM, "admin", "firma", _E, None),
        FR(U, _F, ("tickets", 90000), ("wiki", 1))],
       [(0, frist_von("tickets", "wert"), 60000), (0, frist_von("tickets", "herkunft"), "firmendatei"),
        (0, frist_von("wiki", "wert"), 300000), (0, frist_von("wiki", "herkunft"), "katalog"),
        (0, frist_von("ablage", "wert"), 300000), (0, frist_von("ablage", "herkunft"), "katalog"),
        (0, frist_von("kalender", "wert"), 300000), (0, frist_von("kalender", "herkunft"), "firmendatei"),
        (1, angenommen, True),
        (2, frist_von("tickets", "wert"), 30000), (2, frist_von("tickets", "herkunft"), "katalog"),
        (2, frist_von("wiki", "wert"), 30000), (2, frist_von("wiki", "herkunft"), "katalog"),
        (3, angenommen, True),
        (4, frist_von("tickets", "wert"), 60000), (4, frist_von("tickets", "herkunft"), "katalog"),
        (4, frist_von("wiki", "wert"), 1), (4, frist_von("wiki", "herkunft"), "firmendatei")])

_H = "optionen.herstellerzugang"
fall_s("option-herstellerzugang-fest", ["A11", "A30", "A31"],
       "der Herstellerzugang ist im Pilot fest aus: ein Admin setzt ihn auf Firma oder Gruppe an oder aus, ein Nutzer auf seiner Ebene; "
       "alles abgelehnt, auch ein Wert, den der Stand schon trägt, wirkt nicht",
       stand_lauf(firma={_H: True}),
       [S(ADM, "admin", "firma", _H, True), S(ADM, "admin", "firma", _H, False), S(ADM, "admin", "gruppe g-eins", _H, False),
        S(U, "admin", "nutzer", _H, True), S(ADM, "admin", "firma", _H, None), SI(ADM), SI(U)],
       [(i, code, "ebenen.ebene_gesperrt") for i in range(5)]
       + [(5, feld(_H, "wert"), False), (5, feld(_H, "herkunft"), "firma"), (5, feld(_H, "aenderbar"), False),
          (5, feld(_H, "grund"), "fest"), (6, feld(_H, "aenderbar"), False), (6, feld(_H, "grund"), "fest")])

# Admin-Feld leseklassen (Projektentscheid, 27.11.2026; README Abschnitt 15.11). Form wie rust/spec/datentor/person/VERTRAG.md
# Abschnitt 3. Die Wirkfälle (F1, F2, P2) stehen in rust/spec/datentor/person/sichtbar/ und werden hier nicht gedoppelt.
_L = "leseklassen"
W = "ps_00000000000000000000000000000c03"


def LE(ebene, wer, klasse):
    return {"ebene": ebene, "wer": wer, "klasse": klasse}


LISTE = [LE("gruppe", "g-eins", "intern"), LE("nutzer", U, "oeffentlich"), LE("gruppe", "g-zwei", "kundendaten")]

fall_s("leseklassen-admin-setzt", ["A05", "A26", "A27"],
       "ein Admin setzt die Leseklassen für zwei Gruppen und einen Nutzer auf der Firmenebene; die Admin-Sicht zeigt die ganze Liste",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, LISTE), SI(ADM)],
       [(0, angenommen, True), (1, feld(_L, "wert"), LISTE), (1, feld(_L, "herkunft"), "firma"),
        (1, feld(_L, "aenderbar"), True), (1, feld(_L, "grund"), None)])
fall_s("leseklassen-nutzer-setzt", ["A05", "A27"],
       "ein Nutzer setzt die Leseklassen auf der Firmenebene (nicht befugt), für seine Gruppe und für sich (Ebene gesperrt); "
       "die Liste bleibt leer, also liest er nichts",
       stand_lauf(),
       [S(U, "admin", "firma", _L, [LE("nutzer", U, "kundendaten")]),
        S(U, "admin", "gruppe g-eins", _L, [LE("nutzer", U, "kundendaten")]),
        S(U, "admin", "nutzer", _L, [LE("nutzer", U, "kundendaten")]), SI(U)],
       [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.ebene_gesperrt"), (2, code, "ebenen.ebene_gesperrt"),
        (3, feld(_L, "wert"), []), (3, feld(_L, "aenderbar"), False), (3, feld(_L, "grund"), "nur_admin")])
fall_s("leseklassen-klasse-unbekannt", ["A05", "D14"],
       "eine Klasse, die nicht byte-genau in klassen steht: geheim, Intern und intern mit Leerzeichen",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, [LE("gruppe", "g-eins", "geheim")]),
        S(ADM, "admin", "firma", _L, [LE("gruppe", "g-eins", "Intern")]),
        S(ADM, "admin", "firma", _L, [LE("nutzer", U, "intern ")]), SI(ADM)],
       [(0, code, "ebenen.wert_ungueltig"), (1, code, "ebenen.wert_ungueltig"), (2, code, "ebenen.wert_ungueltig"),
        (3, feld(_L, "wert"), [])])
fall_s("leseklassen-paar-doppelt", ["A05", "A139"],
       "dasselbe Paar aus ebene und wer zweimal, mit anderer und mit gleicher Klasse; gleiches wer auf verschiedener Ebene ist kein Doppel",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, [LE("nutzer", U, "intern"), LE("nutzer", U, "oeffentlich")]),
        S(ADM, "admin", "firma", _L, [LE("gruppe", "g-eins", "intern"), LE("gruppe", "g-eins", "intern")]),
        S(ADM, "admin", "firma", _L, [LE("gruppe", "g-eins", "intern"), LE("gruppe", "g-zwei", "intern")])],
       [(0, code, "ebenen.wert_ungueltig"), (1, code, "ebenen.wert_ungueltig"), (2, angenommen, True)])
fall_s("leseklassen-feld-unbekannt", ["A05"],
       "ein Eintrag mit dem unbekannten Feld kunden (Frage F4), ein Eintrag ohne klasse, die Liste als Objekt",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, [dict(LE("gruppe", "g-eins", "intern"), kunden=["k-a"])]),
        S(ADM, "admin", "firma", _L, [{"ebene": "gruppe", "wer": "g-eins"}]),
        S(ADM, "admin", "firma", _L, {"leseklassen": [LE("gruppe", "g-eins", "intern")]})],
       [(i, code, "ebenen.wert_ungueltig") for i in range(3)])
fall_s("leseklassen-null", ["A05"],
       "null ist für leseklassen kein Wert und keine Rücknahme, auch nicht als klasse oder wer; der gesetzte Wert bleibt",
       stand_lauf(firma={_L: [LE("gruppe", "g-eins", "intern")]}),
       [S(ADM, "admin", "firma", _L, None), S(ADM, "admin", "firma", _L, [LE("gruppe", "g-eins", None)]),
        S(ADM, "admin", "firma", _L, [LE("nutzer", None, "intern")]), S(ADM, "admin", "firma", _L, [None]), SI(ADM)],
       [(i, code, "ebenen.wert_ungueltig") for i in range(4)]
       + [(4, feld(_L, "wert"), [LE("gruppe", "g-eins", "intern")])])
fall_s("leseklassen-form-ungueltig", ["A05", "A12"],
       "ein Konto statt eines Pseudonyms (der Dienst ordnet vorher zu), eine Gruppe, die die Firma nicht kennt, "
       "und eine Ebene rolle; alles abgelehnt",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, [LE("nutzer", "konto-beispiel", "intern")]),
        S(ADM, "admin", "firma", _L, [LE("gruppe", "g-drei", "intern")]),
        S(ADM, "admin", "firma", _L, [LE("rolle", "g-eins", "intern")])],
       [(i, code, "ebenen.wert_ungueltig") for i in range(3)])
_VIELE = [LE("nutzer", "ps_%032x" % (0x100000 + i), "oeffentlich") for i in range(LESE_MAX + 1)]
fall_s("leseklassen-4097", ["A05"],
       "4096 Einträge sind die Grenze und werden angenommen; 4097 Einträge sind ungültig, und der Stand bleibt bei 4096",
       stand_lauf(),
       [S(ADM, "admin", "firma", _L, _VIELE[:LESE_MAX]), S(ADM, "admin", "firma", _L, _VIELE)],
       [(0, angenommen, True), (1, code, "ebenen.wert_ungueltig")])
fall_s("leseklassen-sicht-nutzer", ["A05", "A12", "E3"],
       "die Sicht zeigt dem Nutzer nur die Einträge, die ihn nennen: seinen eigenen und den seiner Gruppe, unverändert und ohne Rangfolge; "
       "nicht änderbar, Grund nur_admin. Der Admin sieht die ganze Liste",
       stand_lauf(firma={_L: [LE("gruppe", "g-eins", "kundendaten"), LE("nutzer", W, "kundendaten"), LE("nutzer", U, "oeffentlich"),
                              LE("gruppe", "g-zwei", "intern")]}),
       [SI(U), SI(V), SI(W), SI(ADM)],
       [(0, feld(_L, "wert"), [LE("gruppe", "g-eins", "kundendaten"), LE("nutzer", U, "oeffentlich")]),
        (0, feld(_L, "herkunft"), "firma"), (0, feld(_L, "aenderbar"), False), (0, feld(_L, "grund"), "nur_admin"),
        (1, feld(_L, "wert"), [LE("gruppe", "g-zwei", "intern")]),
        (2, feld(_L, "wert"), [LE("nutzer", W, "kundendaten")]),
        (3, feld(_L, "aenderbar"), True), (3, lambda e: len(e["einstellungen"][_L]["wert"]), 4)])
# Fehlende Firmenvorgabe (Entscheid des Betreibers, 27.11.2026; schließt OPEN 30): ohne Datei lädt der Lader keine Admins und
# eine leere leseklassen. Die Regeln der Firma gelten weiter; jede Änderung eines Admins lehnt setzen ab.
_STAND_OHNE_VORGABE = dict(stand_lauf(firma={"effort.grenzen": GRENZEN, _L: []}), admins=[])
fall_s("ohne-firmenvorgabe-keine-admins", ["A05", "E1"],
       "die Firmenvorgabe fehlt: niemand ist Admin, und setzen lehnt jede Änderung der Firmenebene und einer Gruppe mit "
       "ebenen.nicht_befugt ab, auch für ein Konto, das vorher Admin war; die Regeln der Firma gelten weiter, ein Nutzer "
       "setzt seine Nutzerebene, und leseklassen bleibt leer",
       _STAND_OHNE_VORGABE,
       [S(ADM, "admin", "firma", _L, [LE("nutzer", U, "intern")]), S(ADM, "admin", "firma", "effort.grenzen", []),
        S(ADM, "admin", "gruppe g-eins", "optionen.herabstufen", True), S(U, "nutzer", "nutzer", "aussehen.thema", "hell"),
        SI(U), SI(ADM)],
       [(0, code, "ebenen.nicht_befugt"), (1, code, "ebenen.nicht_befugt"), (2, code, "ebenen.nicht_befugt"),
        (3, angenommen, True), (4, feld("effort.grenzen", "wert"), GRENZEN), (4, feld("effort.grenzen", "herkunft"), "firma"),
        (4, feld(_L, "wert"), []), (5, feld(_L, "aenderbar"), False), (5, feld("aussehen.thema", "wert"), "system")])


def bauen_setzen():
    fehler, ausgabe = [], []
    for f in FAELLE_SETZEN:
        erg = ausfuehren_setzen(f)
        for i, prob, soll in f["_kern"]:
            try:
                ist = prob(erg["schritte"][i])
            except (KeyError, TypeError, IndexError) as e:
                ist = f"<{type(e).__name__}: {e}>"
            if ist != soll:
                fehler.append(f"{f['name']} Schritt {i}: Referenz {ist!r}, Tabelle {soll!r}")
        ausgabe.append((f["name"], {k: v for k, v in f.items() if not k.startswith("_")}, erg))
    return fehler, ausgabe


# --- Schreibweg der Firmenvorgabe und Konto in der Ansicht der Admins (Entscheid des Betreibers, 27.11.2026) ----------
# README Abschnitte 15.12 und 15.13. Die Referenz beschreibt Pflichten des Dienstes und des Schreibhelfers, keine Funktion
# von regelebenen. Die Fälle stehen in faelle-schreiben/ und erwartet-schreiben/. Konten sind Platzhalter der Form konto-*,
# uids sind Beispielzahlen. Der Helfer läuft unter dem Systemnutzer HELFER (rust/spec/paket/README.md, Teil A3).
HELFER = "werkbank-vorgabe"
UNSICHER = 0o022  # Schreibrecht für Gruppe oder andere
GRENZE_ANFRAGE = 1048576  # Bytes einer Anfrage, höchstens
SCHRITTE_SCHREIBEN = ("temp", "fsync", "rename", "ordner_fsync")
FEHLER_HELFER = {"eingeschlossen", "zu_gross", "frist", "rueckweg"} | set(SCHRITTE_SCHREIBEN)
FEHLER_DIENST = {"fremd_geaendert", "helfer_weg", "antwort_fehlt"}
CODE_SCHRITT = {"temp": "firmenvorgabe.temp_fehlgeschlagen", "fsync": "firmenvorgabe.fsync_fehlgeschlagen",
                "rename": "firmenvorgabe.rename_fehlgeschlagen", "ordner_fsync": "firmenvorgabe.ordner_fsync_fehlgeschlagen"}
KONTO_FORM = re.compile(r"^[a-z_][a-z0-9_-]*$")
HEX64_W = re.compile(r"^[0-9a-f]{64}$")


def bytes_der(inhalt):
    return (json.dumps(inhalt, ensure_ascii=False, indent=1, sort_keys=True) + "\n").encode("utf-8")


def sha(inhalt):
    return hashlib.sha256(bytes_der(inhalt)).hexdigest()


def sicher(ort, eigentuemer):
    # Regulär (kein Verweis), der verlangte Eigentümer, kein Schreibrecht für Gruppe oder andere.
    return ort.get("art") in ("datei", "ordner") and ort["eigentuemer"] == eigentuemer and int(ort["modus"], 8) & UNSICHER == 0


def form_vorgabe(inhalt):
    # Form der Firmenvorgabe, soweit der Helfer sie prüft (15.12, Stufe H4 und H8): Kontenliste und leseklassen.
    # Klassen und Gruppen prüft der Helfer nicht; das tut setzen im Dienst.
    if not isinstance(inhalt, dict) or not {"admins", "leseklassen"} <= set(inhalt):
        return False
    a, l = inhalt["admins"], inhalt["leseklassen"]
    if not isinstance(a, list) or not all(isinstance(k, str) and KONTO_FORM.match(k) for k in a) or len(set(a)) != len(a):
        return False
    if not isinstance(l, list) or len(l) > 4096:
        return False
    paare = set()
    for e in l:
        if not isinstance(e, dict) or set(e) != {"ebene", "wer", "klasse"} or not all(isinstance(v, str) for v in e.values()):
            return False
        if e["ebene"] not in ("nutzer", "gruppe") or (e["ebene"], e["wer"]) in paare:
            return False
        paare.add((e["ebene"], e["wer"]))
    return True


def helfer(zustand, partner, anfrage, fehler):
    """Der Schreibhelfer: genau eine Anfrage, genau eine Antwort (README 15.12, Stufen H1 bis H12).

    zustand hält datei, ordner, eltern und konten (Konto zu uid, wie getpwnam_r). partner ist die uid aus SO_PEERCRED
    oder None. Gibt den Code zurück; None heißt: ersetzt. Ändert zustand["datei"] nur beim Ersetzen und bei zustand_unklar.
    """
    datei = zustand["datei"]
    if partner is None or partner not in zustand["konten"].values():
        return "firmenvorgabe.partner_unbekannt"                              # H1
    if "eingeschlossen" in fehler:
        return "firmenvorgabe.partner_eingeschlossen"                         # H2
    if not (sicher(datei, HELFER) and sicher(zustand["ordner"], HELFER) and sicher(zustand["eltern"], "root")):
        return "firmenvorgabe.datei_unsicher"                                 # H3
    if not form_vorgabe(datei["inhalt"]):
        return "firmenvorgabe.ladefehler"                                     # H4
    admins = {zustand["konten"][k] for k in datei["inhalt"]["admins"] if k in zustand["konten"]}
    if partner not in admins:
        return "firmenvorgabe.partner_nicht_admin"                            # H5, gegen die gültige Datei
    if "zu_gross" in fehler:
        return "firmenvorgabe.anfrage_zu_gross"                               # H6
    if "frist" in fehler:
        return "firmenvorgabe.anfrage_frist"                                  # H7
    if not (isinstance(anfrage, dict) and set(anfrage) == {"art", "fassung", "basis", "inhalt"}
            and anfrage["art"] == "Ersetzen" and anfrage["fassung"] == 1
            and isinstance(anfrage["basis"], str) and HEX64_W.match(anfrage["basis"]) and form_vorgabe(anfrage["inhalt"])):
        return "firmenvorgabe.anfrage_ungueltig"                              # H8
    if anfrage["basis"] != sha(datei["inhalt"]):
        return "firmenvorgabe.veraltet"                                       # H9
    neu = anfrage["inhalt"]
    if {k: v for k, v in neu.items() if k != _L} != {k: v for k, v in datei["inhalt"].items() if k != _L}:
        return "firmenvorgabe.ausser_leseklassen"                             # H10, auch die Kontenliste der Admins
    for schritt in SCHRITTE_SCHREIBEN:                                        # H11 bis H12
        if schritt in fehler:
            if schritt == "ordner_fsync" and "rueckweg" in fehler:
                datei["inhalt"] = copy.deepcopy(neu)  # der neue Inhalt steht ganz, nie ein Teil
                return "firmenvorgabe.zustand_unklar"
            return CODE_SCHRITT[schritt]
    datei["inhalt"] = copy.deepcopy(neu)
    return None


def in_pseudonyme(liste, zuordnung):
    # Pflicht des Dienstes (15.2): Konto zu Pseudonym. Ein Konto ohne Zuordnung und ein Pseudonym aus der Ansicht
    # werden zu wer null, das setzen mit ebenen.wert_ungueltig ablehnt; so steht kein Konto in der Zeile.
    if not isinstance(liste, list):
        return liste
    aus = []
    for x in liste:
        if isinstance(x, dict) and x.get("ebene") == "nutzer":
            x = dict(x, wer=zuordnung.get(x.get("wer")) if isinstance(x.get("wer"), str) else None)
        aus.append(x)
    return aus


def datei_erg(datei, vorher):
    return {"datei": {"eigentuemer": datei["eigentuemer"], "modus": datei["modus"], "sha256": sha(datei["inhalt"]),
                      "inhalt": copy.deepcopy(datei["inhalt"])},
            "unveraendert": sha(datei["inhalt"]) == vorher, "temp_bleibt": False}


def lauf_schreiben(fall):
    zu = fall["zuordnung"]
    zustand = {"datei": copy.deepcopy(fall["firmenvorgabe"]), "ordner": fall["ordner"], "eltern": fall["eltern"],
               "konten": fall["konten"]}
    datei = zustand["datei"]
    konto_von = {u: k for k, u in fall["konten"].items()}
    geladen = copy.deepcopy(datei["inhalt"])  # was der Dienst zuletzt geladen hat
    gesperrt = False

    def stand_aus():
        werte = {"firma": {_L: in_pseudonyme(geladen[_L], zu)}, "gruppen": {}, "nutzer": {}}
        return {"klassen": fall["firma"]["klassen"], "admins": [zu[k] for k in geladen["admins"]],
                "gruppen": fall["firma"]["gruppen"], "werte": werte}

    zurueck = {p: k for k, p in zu.items()}
    erg = []
    for s in fall["schritte"]:
        if "neuladen" in s:
            # Der Dienst lädt die Firmenvorgabe neu; das hebt die Sperre nach zustand_unklar auf.
            geladen, gesperrt = copy.deepcopy(datei["inhalt"]), False
            erg.append({"geladen": sha(geladen), "gesperrt": gesperrt})
            continue
        if "nachricht" in s:
            stand = stand_aus()
            wer = zu[konto_von[s["nachricht"]["partner"]]]
            admin = wer in stand["admins"]
            werte = sicht(stand, wer)["einstellungen"]
            if admin:
                # Der eine Weg eines Kontos zur Ansicht: für einen Admin an der Stelle des Pseudonyms (15.13).
                werte[_L]["wert"] = [dict(x, wer=zurueck.get(x["wer"], x["wer"])) if x["ebene"] == "nutzer" else x
                                     for x in werte[_L]["wert"]]
            erg.append({"einstellungen": {"art": "Einstellungen", "fassung": 2, "admin": admin, "werte": werte}})
            continue
        vorher = sha(datei["inhalt"])
        if "anfrage" in s:
            # Ein Prozess spricht den Socket des Helfers direkt an, ohne Dienst.
            a = s["anfrage"]
            code = helfer(zustand, a["partner"], a["anfrage"], set(a["fehler"]))
            erg.append(dict({"antwort": {"code": code, "sha256": sha(datei["inhalt"]) if code is None else None}},
                            **datei_erg(datei, vorher)))
            continue
        a = s["schreiben"]
        fehler = set(a["fehler"])
        stand = stand_aus()
        wer = zu[konto_von[a["partner"]]]   # Stufe 0: uid des lokalen Wegs, dann Pseudonym über die Stelle
        wert_ps = in_pseudonyme(a["wert"], zu)
        if gesperrt:
            code = "firmenvorgabe.zustand_unklar"                         # Sperre bis zum Neuladen
        elif wer not in stand["admins"]:
            code = "ebenen.nicht_befugt"                                  # Stufe 1, vor jedem Kontakt zum Helfer
        else:
            code = setzen(copy.deepcopy(stand), wer, "admin", "firma", _L, wert_ps)["code"]  # Stufe 2, auf einer Kopie
        if code is None and "helfer_weg" in fehler:
            code = "firmenvorgabe.helfer_nicht_erreichbar"                # Stufe 3, nichts gesendet
        if code is None:
            basis = sha(geladen) if "fremd_geaendert" not in fehler else sha({"veraltet": geladen})
            anfrage = {"art": "Ersetzen", "fassung": 1, "basis": basis, "inhalt": dict(geladen, **{_L: copy.deepcopy(a["wert"])})}
            code = helfer(zustand, a["partner"], anfrage, fehler & FEHLER_HELFER)  # Stufe 4: Code des Helfers unverändert
            if "antwort_fehlt" in fehler:
                code = "firmenvorgabe.zustand_unklar"                     # gesendet, aber keine Antwort: Ausgang unbekannt
            if code is None:
                geladen = copy.deepcopy(datei["inhalt"])
            if code == "firmenvorgabe.zustand_unklar":
                gesperrt = True
        zeile = zeile_setzen(wer, "firma", None, _L, (True, stand["werte"]["firma"][_L]), wert_ps, code)
        erg.append(dict({"angenommen": code is None, "code": code, "zeile": zeile}, **datei_erg(datei, vorher)))
    return {"schritte": erg}


KA, KA2, KU, KV = "konto-admin", "konto-admin-zwei", "konto-nutzer", "konto-nutzer-zwei"
ZU = {KA: ADM, KA2: ADM2, KU: U, KV: V}
UID = {KA: 1001, KA2: 1002, KU: 1003, KV: 1004}
UA, UA2, UU, UV = UID[KA], UID[KA2], UID[KU], UID[KV]
UID_FREMD = 1999  # eine uid ohne Konto
ALT_LISTE = [LE("gruppe", "g-eins", "intern"), LE("nutzer", KU, "oeffentlich")]
NEU_LISTE = [LE("gruppe", "g-eins", "intern"), LE("nutzer", KU, "kundendaten"), LE("nutzer", KV, "oeffentlich")]


def inhalt_alt(leseklassen=None, admins=None):
    return {"admins": list(admins or [KA, KA2]), "leseklassen": copy.deepcopy(ALT_LISTE if leseklassen is None else leseklassen)}


def vorgabe(eigentuemer=HELFER, modus="0644", art="datei", inhalt=None):
    return {"art": art, "eigentuemer": eigentuemer, "modus": modus, "inhalt": inhalt or inhalt_alt()}


def SW(partner, wert, *fehler):
    return {"schreiben": {"partner": partner, "wert": wert, "fehler": list(fehler)}}


def AN(partner, inhalt, *fehler, basis=None, roh=None):
    # Eine Anfrage direkt an den Socket des Helfers; roh ersetzt die ganze Anfrage (für Formfehler).
    anfrage = roh if roh is not None else {"art": "Ersetzen", "fassung": 1, "basis": basis or SHA_ALT, "inhalt": inhalt}
    return {"anfrage": {"partner": partner, "anfrage": anfrage, "fehler": list(fehler)}}


def NA(partner):
    return {"nachricht": {"partner": partner}}


NL = {"neuladen": {}}
FAELLE_SCHREIBEN = []
ORDNER = {"art": "ordner", "eigentuemer": HELFER, "modus": "0755"}
ELTERN = {"art": "ordner", "eigentuemer": "root", "modus": "0755"}


def fall_w(name, anforderungen, beschreibung, schritte, kern, datei=None, ordner=None, eltern=None):
    FAELLE_SCHREIBEN.append({"name": name, "anforderungen": anforderungen, "beschreibung": beschreibung,
                             "firma": {"klassen": ["oeffentlich", "intern", "kundendaten"], "gruppen": {"g-eins": [U], "g-zwei": [V]}},
                             "konten": UID, "zuordnung": ZU, "firmenvorgabe": datei or vorgabe(),
                             "ordner": ordner or ORDNER, "eltern": eltern or ELTERN,
                             "schritte": schritte, "_kern": kern})


SHA_ALT = sha(inhalt_alt())
SHA_NEU = sha(inhalt_alt(NEU_LISTE))


def unveraendert(e):
    return (e["unveraendert"], e["datei"]["sha256"])


def antwort(e):
    return e["antwort"]["code"]


def unsicher(name, beschreibung, **ort):
    fall_w(name, ["A05"], beschreibung, [SW(UA, NEU_LISTE)],
           [(0, code, "firmenvorgabe.datei_unsicher"), (0, unveraendert, (True, SHA_ALT))], **ort)


# Weg über die Ansicht: der Dienst prüft, ruft setzen und reicht die Datei an den Helfer.
fall_w("schreiben-admin-erfolgreich", ["A05", "A27"],
       "ein Admin ändert die Leseklassen aus der Ansicht; der Dienst prüft den Partner, ruft setzen und reicht die neue "
       "Datei an den Helfer, der sie atomar ersetzt. Die Datei trägt Konten, die Zeile nur Pseudonyme",
       [SW(UA, NEU_LISTE), NA(UA)],
       [(0, angenommen, True), (0, code, None), (0, lambda e: e["zeile"]["aktionsname"], "regel.geaendert"),
        (0, lambda e: e["datei"]["sha256"], SHA_NEU), (0, lambda e: e["datei"]["inhalt"]["leseklassen"], NEU_LISTE),
        (0, lambda e: (e["datei"]["eigentuemer"], e["datei"]["modus"]), (HELFER, "0644")),
        (1, lambda e: e["einstellungen"]["werte"][_L]["wert"], NEU_LISTE)])
fall_w("schreiben-nichtadmin", ["A05", "A27"],
       "ein Nutzer, der kein Admin ist, schreibt die Leseklassen aus der Ansicht: der Dienst lehnt ab, bevor er den Helfer "
       "anspricht, auch wenn der Nutzer sich selbst eintragen will; die Datei bleibt byte-gleich",
       [SW(UU, NEU_LISTE), SW(UU, [LE("nutzer", KU, "kundendaten")])],
       [(i, code, "ebenen.nicht_befugt") for i in range(2)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(2)]
       + [(0, lambda e: e["zeile"]["aktionsname"], "regel.abgelehnt"), (0, lambda e: e["zeile"]["handelnd"], U)])
unsicher("schreiben-datei-fremder-eigentuemer", "die Firmenvorgabe gehört einem Nutzerkonto, nicht dem Helfernutzer; "
         "der Helfer schreibt nicht, die Datei bleibt byte-gleich", datei=vorgabe(eigentuemer=KU))
unsicher("schreiben-datei-root-eigen", "die Firmenvorgabe gehört root, etwa nach einer Änderung von Hand ohne chown; "
         "der Helfer schreibt nicht, bis die IT den Eigentümer zurücksetzt", datei=vorgabe(eigentuemer="root"))
unsicher("schreiben-datei-gruppe-schreibbar", "die Firmenvorgabe ist für die Gruppe schreibbar (0664)", datei=vorgabe(modus="0664"))
unsicher("schreiben-datei-andere-schreibbar", "die Firmenvorgabe ist für andere schreibbar (0646)", datei=vorgabe(modus="0646"))
unsicher("schreiben-datei-verweis", "die Firmenvorgabe ist ein symbolischer Verweis; der Helfer folgt ihm nicht",
         datei=vorgabe(art="verweis"))
unsicher("schreiben-ordner-schreibbar", "der Ordner der Firmenvorgabe ist für andere schreibbar (0777); ein Fremder "
         "könnte die Datei ersetzen", ordner=dict(ORDNER, modus="0777"))
unsicher("schreiben-ordner-fremder-eigentuemer", "der Ordner der Firmenvorgabe gehört einem Nutzerkonto",
         ordner=dict(ORDNER, eigentuemer=KU))
unsicher("schreiben-ordner-verweis", "der Ordner der Firmenvorgabe ist ein symbolischer Verweis",
         ordner=dict(ORDNER, art="verweis"))
unsicher("schreiben-eltern-nicht-root", "der Elternordner gehört nicht root; sein Eigentümer könnte den Ordner der "
         "Firmenvorgabe austauschen", eltern=dict(ELTERN, eigentuemer=KU))
unsicher("schreiben-eltern-schreibbar", "der Elternordner ist für die Gruppe schreibbar (0775)", eltern=dict(ELTERN, modus="0775"))
fall_w("schreiben-veraltet", ["A05", "A27"],
       "root hat die Firmenvorgabe nach dem Laden geändert; die Basis der Anfrage stimmt nicht, der Helfer überschreibt "
       "diese Änderung nicht",
       [SW(UA, NEU_LISTE, "fremd_geaendert")],
       [(0, code, "firmenvorgabe.veraltet"), (0, unveraendert, (True, SHA_ALT))])
fall_w("schreiben-wert-ungueltig", ["A05", "A12"],
       "ein Admin nennt ein Konto, das der Dienst nicht zuordnen kann, und ein Pseudonym statt eines Kontos; "
       "setzen lehnt ab, der Helfer wird nicht angesprochen, und weder Konto noch Pseudonym aus der Eingabe steht in der Zeile",
       [SW(UA, [LE("nutzer", "konto-unbekannt", "intern")]), SW(UA, [LE("nutzer", U, "intern")])],
       [(i, code, "ebenen.wert_ungueltig") for i in range(2)] + [(i, unveraendert, (True, SHA_ALT)) for i in range(2)])
fall_w("schreiben-helfer-nicht-erreichbar", ["A05"],
       "der Socket des Helfers fehlt oder nimmt keine Verbindung an; der Dienst sendet nichts, die Datei bleibt byte-gleich",
       [SW(UA, NEU_LISTE, "helfer_weg")],
       [(0, code, "firmenvorgabe.helfer_nicht_erreichbar"), (0, unveraendert, (True, SHA_ALT))])
fall_w("schreiben-temp-scheitert", ["A05"],
       "die Temp-Datei lässt sich nicht anlegen oder nicht ganz schreiben (etwa ENOSPC)",
       [SW(UA, NEU_LISTE, "temp")],
       [(0, code, "firmenvorgabe.temp_fehlgeschlagen"), (0, unveraendert, (True, SHA_ALT)),
        (0, lambda e: e["temp_bleibt"], False)])
fall_w("schreiben-fsync-scheitert", ["A05"],
       "fsync der Temp-Datei scheitert (EIO); der Helfer entfernt die Temp-Datei und benennt nichts um",
       [SW(UA, NEU_LISTE, "fsync")],
       [(0, code, "firmenvorgabe.fsync_fehlgeschlagen"), (0, unveraendert, (True, SHA_ALT))])
fall_w("schreiben-rename-scheitert", ["A05"],
       "rename scheitert; die alte Datei bleibt, die Temp-Datei ist entfernt",
       [SW(UA, NEU_LISTE, "rename")],
       [(0, code, "firmenvorgabe.rename_fehlgeschlagen"), (0, unveraendert, (True, SHA_ALT))])
fall_w("schreiben-ordner-fsync-scheitert", ["A05"],
       "fsync des Ordners scheitert nach dem rename; der Helfer schreibt die alten Bytes auf demselben Weg zurück, "
       "die Datei ist danach byte-gleich",
       [SW(UA, NEU_LISTE, "ordner_fsync"), NA(UA)],
       [(0, code, "firmenvorgabe.ordner_fsync_fehlgeschlagen"), (0, unveraendert, (True, SHA_ALT)),
        (1, lambda e: e["einstellungen"]["werte"][_L]["wert"], ALT_LISTE)])
fall_w("schreiben-rueckweg-scheitert", ["A05"],
       "fsync des Ordners scheitert, und der Rückweg scheitert auch; der neue Inhalt steht ganz, nie halb, "
       "und der Dienst sperrt bis zum Neuladen",
       [SW(UA, NEU_LISTE, "ordner_fsync", "rueckweg")],
       [(0, code, "firmenvorgabe.zustand_unklar"), (0, unveraendert, (False, SHA_NEU)), (0, angenommen, False)])
fall_w("schreiben-antwort-fehlt", ["A05"],
       "der Helfer ersetzt die Datei, aber die Verbindung bricht vor der Antwort ab (Frist des Dienstes); der Dienst kennt "
       "den Ausgang nicht, meldet zustand_unklar und sperrt",
       [SW(UA, NEU_LISTE, "antwort_fehlt")],
       [(0, code, "firmenvorgabe.zustand_unklar"), (0, unveraendert, (False, SHA_NEU))])
fall_w("schreiben-sperre-bis-neuladen", ["A05"],
       "nach zustand_unklar lehnt der Dienst jede weitere Änderung ab, ohne den Helfer anzusprechen; nach dem Neuladen "
       "gelingt die nächste Änderung auf der Basis des geladenen Inhalts",
       [SW(UA, NEU_LISTE, "ordner_fsync", "rueckweg"), SW(UA2, ALT_LISTE), NL, SW(UA2, ALT_LISTE)],
       [(0, code, "firmenvorgabe.zustand_unklar"), (1, code, "firmenvorgabe.zustand_unklar"),
        (1, unveraendert, (True, SHA_NEU)), (2, lambda e: e["gesperrt"], False), (2, lambda e: e["geladen"], SHA_NEU),
        (3, angenommen, True), (3, lambda e: e["datei"]["sha256"], SHA_ALT)])
fall_w("schreiben-nach-fehler-erfolgreich", ["A05"],
       "nach einem gescheiterten rename gelingt der nächste Versuch eines anderen Admins; der Stand des ersten Versuchs "
       "wirkt nie",
       [SW(UA, NEU_LISTE, "rename"), SW(UA2, NEU_LISTE)],
       [(0, unveraendert, (True, SHA_ALT)), (1, angenommen, True), (1, lambda e: e["datei"]["sha256"], SHA_NEU)])
# Weg direkt über den Socket des Helfers: ein beliebiger Prozess am Rechner verbindet sich ohne Dienst.
fall_w("socket-admin-erfolgreich", ["A05"],
       "ein Prozess eines Admins spricht den Helfer direkt an; der Helfer prüft selbst und ersetzt die Datei",
       [AN(UA, inhalt_alt(NEU_LISTE))],
       [(0, antwort, None), (0, lambda e: e["antwort"]["sha256"], SHA_NEU), (0, unveraendert, (False, SHA_NEU)),
        (0, lambda e: (e["datei"]["eigentuemer"], e["datei"]["modus"]), (HELFER, "0644"))])
fall_w("socket-nichtadmin", ["A05"],
       "ein Prozess eines Nutzers, der kein Admin ist, spricht den Helfer direkt an, auch für einen eigenen Eintrag",
       [AN(UU, inhalt_alt(NEU_LISTE)), AN(UV, inhalt_alt([LE("nutzer", KV, "kundendaten")]))],
       [(i, antwort, "firmenvorgabe.partner_nicht_admin") for i in range(2)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(2)])
fall_w("socket-partner-unbekannt", ["A05"],
       "der Partner ist nicht feststellbar (SO_PEERCRED scheitert) oder seine uid hat kein Konto",
       [AN(None, inhalt_alt(NEU_LISTE)), AN(UID_FREMD, inhalt_alt(NEU_LISTE))],
       [(i, antwort, "firmenvorgabe.partner_unbekannt") for i in range(2)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(2)])
fall_w("socket-admin-nur-in-neuer-datei", ["A05"],
       "ein Nutzer reicht eine Datei ein, die ihn als Admin nennt; befugt macht nur die gültige Datei, nicht die neue",
       [AN(UU, inhalt_alt(NEU_LISTE, admins=[KA, KA2, KU])), AN(UU, inhalt_alt(admins=[KU]))],
       [(i, antwort, "firmenvorgabe.partner_nicht_admin") for i in range(2)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(2)])
fall_w("socket-admins-geaendert", ["A05"],
       "ein Admin reicht eine Datei mit geänderter Kontenliste der Admins ein; der Helfer ändert nur leseklassen, die "
       "Admins ändern sich nur von Hand mit Systemrechten",
       [AN(UA, inhalt_alt(admins=[KA, KA2, KU])), AN(UA, inhalt_alt(admins=[KA])),
        AN(UA, dict(inhalt_alt(), weiteres=True))],
       [(i, antwort, "firmenvorgabe.ausser_leseklassen") for i in range(3)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(3)])
fall_w("socket-eingeschlossen", ["A05", "A12"],
       "ein Prozess eines Admins in der Einheit eines Laufs (etwa ein Agent) spricht den Helfer an; abgelehnt, bevor der "
       "Helfer die Datei liest",
       [AN(UA, inhalt_alt(NEU_LISTE), "eingeschlossen")],
       [(0, antwort, "firmenvorgabe.partner_eingeschlossen"), (0, unveraendert, (True, SHA_ALT))])
fall_w("socket-anfrage-zu-gross", ["A05"],
       "die Anfrage ist länger als 1048576 Bytes; der Helfer liest nicht weiter",
       [AN(UA, inhalt_alt(NEU_LISTE), "zu_gross")],
       [(0, antwort, "firmenvorgabe.anfrage_zu_gross"), (0, unveraendert, (True, SHA_ALT))])
fall_w("socket-anfrage-frist", ["A05"],
       "die Anfrage ist 2 s nach der Annahme der Verbindung nicht vollständig",
       [AN(UA, inhalt_alt(NEU_LISTE), "frist")],
       [(0, antwort, "firmenvorgabe.anfrage_frist"), (0, unveraendert, (True, SHA_ALT))])
fall_w("socket-anfrage-ungueltig", ["A05"],
       "die Anfrage hat nicht die Form: Basis keine Prüfsumme, Inhalt keine Firmenvorgabe, ein Eintrag mit fremdem Feld, "
       "unbekannte Art",
       [AN(UA, inhalt_alt(NEU_LISTE), basis="alt"), AN(UA, ["kein", "objekt"]),
        AN(UA, inhalt_alt([dict(LE("nutzer", KU, "intern"), kunden=[])])),
        AN(UA, None, roh={"art": "Lesen", "fassung": 1, "basis": SHA_ALT, "inhalt": inhalt_alt()})],
       [(i, antwort, "firmenvorgabe.anfrage_ungueltig") for i in range(4)]
       + [(i, unveraendert, (True, SHA_ALT)) for i in range(4)])
fall_w("socket-veraltet", ["A05"],
       "die Basis der Anfrage ist die Prüfsumme eines älteren Inhalts",
       [AN(UA, inhalt_alt(NEU_LISTE)), AN(UA2, inhalt_alt([]))],
       [(0, antwort, None), (1, antwort, "firmenvorgabe.veraltet"), (1, unveraendert, (True, SHA_NEU))])
fall_w("socket-ladefehler", ["A05"],
       "die gültige Datei hat nicht die Form (Kontenliste kein Feld aus Konten); ohne lesbare Kontenliste ist niemand Admin",
       [AN(UA, inhalt_alt(NEU_LISTE))],
       [(0, antwort, "firmenvorgabe.ladefehler"), (0, unveraendert, (True, sha({"admins": "konto-admin", "leseklassen": []})))],
       datei=vorgabe(inhalt={"admins": "konto-admin", "leseklassen": []}))
# Konto in der Ansicht der Admins (15.13): ein Fall je Richtung und die Zeile.
fall_w("konto-admin-sieht", ["A12", "A27", "E3"],
       "ein Admin sieht in der Nachricht Einstellungen das Konto an der Stelle des Pseudonyms; sicht selbst liefert Pseudonyme",
       [NA(UA)],
       [(0, lambda e: e["einstellungen"]["admin"], True), (0, lambda e: e["einstellungen"]["werte"][_L]["wert"], ALT_LISTE)])
fall_w("konto-nutzer-sieht-keins", ["A12", "E3"],
       "ein Nutzer, der kein Admin ist, sieht seinen Eintrag mit dem Pseudonym, nie mit einem Konto",
       [NA(UU), NA(UV)],
       [(0, lambda e: e["einstellungen"]["admin"], False),
        (0, lambda e: e["einstellungen"]["werte"][_L]["wert"], [LE("gruppe", "g-eins", "intern"), LE("nutzer", U, "oeffentlich")]),
        (1, lambda e: e["einstellungen"]["werte"][_L]["wert"], [])])
fall_w("konto-zeile-ohne-konto", ["A12", "A27"],
       "die Zeile einer Änderung, die ein Admin mit Konten eingibt, nennt nur Pseudonyme: handelnd, und in alt und neu "
       "die Prüfsumme über die Liste mit Pseudonymen",
       [SW(UA, NEU_LISTE)],
       [(0, lambda e: e["zeile"]["handelnd"], ADM),
        (0, lambda e: e["zeile"]["neu"], kanonisch_hash({"schluessel": _L, "wert": in_pseudonyme(NEU_LISTE, ZU)})),
        (0, lambda e: e["zeile"]["alt"], kanonisch_hash({"schluessel": _L, "wert": in_pseudonyme(ALT_LISTE, ZU)}))])


def bauen_schreiben():
    fehler, ausgabe = [], []
    for f in FAELLE_SCHREIBEN:
        erg = lauf_schreiben(f)
        for i, prob, soll in f["_kern"]:
            try:
                ist = prob(erg["schritte"][i])
            except (KeyError, TypeError, IndexError) as e:
                ist = f"<{type(e).__name__}: {e}>"
            if ist != soll:
                fehler.append(f"{f['name']} Schritt {i}: Referenz {ist!r}, Tabelle {soll!r}")
        ausgabe.append((f["name"], {k: v for k, v in f.items() if not k.startswith("_")}, erg))
    return fehler, ausgabe


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
    fehler_s, ausgabe_s = bauen_setzen()
    fehler += fehler_s
    fehler_w, ausgabe_w = bauen_schreiben()
    fehler += fehler_w
    if fehler:
        for z in fehler:
            print("FEHLER", z)
        return 1
    for ordner in ("faelle-setzen", "erwartet-setzen"):
        shutil.rmtree(HIER / ordner, ignore_errors=True)
        (HIER / ordner).mkdir()
    for name, sichtbar, erg in ausgabe_s:
        (HIER / "faelle-setzen" / f"{name}.json").write_text(json.dumps(sichtbar, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "erwartet-setzen" / f"{name}.json").write_text(json.dumps(erg, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    for ordner in ("faelle-schreiben", "erwartet-schreiben"):
        shutil.rmtree(HIER / ordner, ignore_errors=True)
        (HIER / ordner).mkdir()
    for name, sichtbar, erg in ausgabe_w:
        (HIER / "faelle-schreiben" / f"{name}.json").write_text(json.dumps(sichtbar, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "erwartet-schreiben" / f"{name}.json").write_text(json.dumps(erg, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    for ordner in ("faelle", "erwartet"):
        shutil.rmtree(HIER / ordner, ignore_errors=True)
        (HIER / ordner).mkdir()
    for name, sichtbar, erg in ausgabe:
        (HIER / "faelle" / f"{name}.json").write_text(json.dumps(sichtbar, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(json.dumps(erg, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(ausgabe)} Fälle, {len(ausgabe_s)} Fälle zu setzen und sicht und {len(ausgabe_w)} Fälle zum Schreibweg "
          "geschrieben, Kernwerte stimmen")
    return 0


if __name__ == "__main__":
    sys.exit(main())
