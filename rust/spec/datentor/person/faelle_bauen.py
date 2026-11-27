"""Builds the visible cases of the part contract `datentor person` (person/VERTRAG.md).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json (the input on stdin of `datentor person`) and sichtbar/erwartet/<name>.json (exit code
and expected output). Every expectation is written out by hand in the tables below: reason, effective class and read
class of the person. The script checks each one against a small reference of the rules in person/VERTRAG.md, section 4,
and each case against the validity rules of section 3; a mismatch stops the script before anything is written.

Counter-check: every valid case has at least one request whose answer differs from `darf` of contract 3 alone (the
reference without the person rule). The script counts these requests and stops if a case has none.

The vorgabe part and `darf` come from the reference of contract 3 (../faelle_bauen.py), loaded, not copied. All data is
invented.
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

_spec = importlib.util.spec_from_file_location("datentor_vertrag3", HIER.parent / "faelle_bauen.py")
DT3 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(DT3)

MELDUNG = dict(DT3.MELDUNG, **{
    "person-fehlt": "Anfrage ohne Person",
    "person-ohne-leseklasse": "Person ohne Leseklasse",
    "klasse-ueber-person": "Klasse der Einheit liegt über der Leseklasse der Person",
})
EBENEN = ("nutzer", "gruppe")
FALL_FELDER = {"name", "regeln", "beschreibung", "vorgabe", "personen", "anfragen"}

KL3 = DT3.KL3
route, kunde, freigaben = DT3.route, DT3.kunde, DT3.freigaben
FEHLT = DT3.FEHLT

# ---------------------------------------------------------------- settings
ROUTEN = [route("r-oe", "lokal", "oeffentlich"), route("r-in", "lokal", "intern"), route("r-kd", "lokal", "kundendaten"),
          route("c-kd", "cloud", "kundendaten"), route("r-ohne", "lokal")]
V = {"klassen": KL3, "routen": ROUTEN,
     "kunden": [kunde("k-1", erlaubt=["r-kd", "c-kd"]), kunde("k-2", erlaubt=["r-kd"], verboten=["c-kd"])],
     "kundendaten_an_cloud": freigaben(("c-kd", "k-1"))}
V_STUFE = dict(V, vorgabestufe="intern")


def lk(ebene, wer, klasse):
    return {"ebene": ebene, "wer": wer, "klasse": klasse}


LK = [lk("nutzer", "p-oe", "oeffentlich"), lk("nutzer", "p-in", "intern"), lk("nutzer", "p-kd", "kundendaten"),
      lk("gruppe", "g-oe", "oeffentlich"), lk("gruppe", "g-in", "intern"), lk("gruppe", "g-kd", "kundendaten"),
      lk("nutzer", "p-tief", "oeffentlich")]
P = {"leseklassen": LK}


FEHLT_KONTEXT = object()  # pa(..., kontext=FEHLT_KONTEXT) leaves kontext out


def pa(klasse, route_, kunde_=None, nutzer=FEHLT, gruppen=FEHLT, kontext=FEHLT):
    """One request. klasse FEHLT: unit without class. kontext built from nutzer and gruppen unless given."""
    a = {"einheit": {} if klasse is FEHLT else {"klasse": klasse}}
    if route_ is not FEHLT:
        a["route"] = route_
    if kunde_ is not FEHLT:
        a["kunde"] = kunde_
    if kontext is FEHLT:
        kontext = {}
        if nutzer is not FEHLT:
            kontext["nutzer"] = nutzer
        if gruppen is not FEHLT:
            kontext["gruppen"] = gruppen
    if kontext is not FEHLT_KONTEXT:
        a["kontext"] = kontext
    return a


def pl(klasse, kunde_=FEHLT, nutzer=FEHLT, gruppen=FEHLT, kontext=FEHLT):
    """One read of the person herself, without a route (P7, Nachtrag 27.11.2026)."""
    a = pa(klasse, FEHLT, kunde_, nutzer, gruppen, kontext)
    a["lesen"] = True
    return a


PS_A = "ps_" + "0123456789abcdef" * 2  # pseudonym form of regelebenen, a valid key
PS_B = "ps_" + "fedcba9876543210" * 2
P_PS = {"leseklassen": [lk("nutzer", PS_A, "intern"), lk("gruppe", "g-alle", "oeffentlich")]}
P_VOLL = {"leseklassen": [lk("gruppe", f"g{i}", "intern") for i in range(4095)] + [lk("nutzer", "p-letzt", "kundendaten")]}


# (name, rules, description, vorgabe, personen, [(request, (reason, effective class, read class))])
FAELLE = [
    ("p3-a34-gegenprobe", ["P3", "A34", "A05"],
     "A34, zweite Gegenprobe: Route intern, Person nur mit Leseklasse oeffentlich; die Einheit intern ist gesperrt. "
     "Mit Leseklasse intern ist sie frei.",
     V, P, [
         (pa("intern", "r-in", nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pa("oeffentlich", "r-in", nutzer="p-oe"), ("frei", "oeffentlich", "oeffentlich")),
         (pa("intern", "r-in", nutzer="p-in"), ("frei", "intern", "intern")),
         (pa("intern", "r-in", nutzer="p-kd"), ("frei", "intern", "kundendaten")),
     ]),
    ("p1-person-fehlt", ["P1", "D2"],
     "Ohne Person wird gesperrt: kontext leer, nutzer fehlt, ist null oder leer. Eine Gruppe allein ist keine Person.",
     V, P, [
         (pa("oeffentlich", "r-oe"), ("person-fehlt", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", kontext=FEHLT_KONTEXT), ("person-fehlt", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer=None), ("person-fehlt", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer=""), ("person-fehlt", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer=None, gruppen=["g-kd"]), ("person-fehlt", "oeffentlich", None)),
         (pa("oeffentlich", FEHLT, nutzer=None), ("route-fehlt", "oeffentlich", None)),
     ]),
    ("p2-ohne-leseklasse", ["P2", "D14"],
     "Eine Person ohne geltenden Eintrag hat keine Leseklasse und wird gesperrt; Schlüssel byte-genau.",
     V, P, [
         (pa("oeffentlich", "r-oe", nutzer="p-x"), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="P-OE"), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="p-oe "), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer=" "), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="p-x", gruppen=["g-x", "G-OE"]), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="g-oe"), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="p-x", gruppen=["p-oe"]), ("person-ohne-leseklasse", "oeffentlich", None)),
     ]),
    ("p3-kunde-hebt", ["P3", "D5"],
     "Eine Einheit mit Kunde hat die höchste Klasse; nur eine Person mit der höchsten Leseklasse bekommt sie.",
     V, P, [
         (pa("oeffentlich", "r-kd", "k-1", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pa("oeffentlich", "r-kd", "k-1", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "c-kd", "k-1", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "c-kd", "k-1", nutzer="p-oe"), ("klasse-ueber-person", "kundendaten", "oeffentlich")),
         (pa("intern", "r-kd", "", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
     ]),
    ("p4-gruppen", ["P4", "F1"],
     "Ohne eigenen Eintrag gilt die höchste Leseklasse der Gruppen der Person; unbekannte Gruppen zählen nicht.",
     V, P, [
         (pa("intern", "r-in", nutzer="p-g", gruppen=["g-oe"]), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pa("intern", "r-in", nutzer="p-g", gruppen=["g-oe", "g-in"]), ("frei", "intern", "intern")),
         (pa("intern", "r-in", nutzer="p-g", gruppen=["g-in", "g-oe"]), ("frei", "intern", "intern")),
         (pa("intern", "r-in", nutzer="p-g", gruppen=["g-x", "g-in"]), ("frei", "intern", "intern")),
         (pa("kundendaten", "r-kd", "k-1", nutzer="p-g", gruppen=["g-oe", "g-kd", "g-in"]),
          ("frei", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "r-oe", nutzer="p-g", gruppen=[]), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="p-g", gruppen=["G-IN"]), ("person-ohne-leseklasse", "oeffentlich", None)),
     ]),
    ("p4-nutzer-vor-gruppe", ["P4", "F2"],
     "Ein eigener Eintrag der Person gilt vor ihren Gruppen, auch wenn er niedriger liegt.",
     V, P, [
         (pa("intern", "r-in", nutzer="p-tief", gruppen=["g-kd"]), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pa("oeffentlich", "r-oe", nutzer="p-tief", gruppen=["g-kd"]), ("frei", "oeffentlich", "oeffentlich")),
         (pa("kundendaten", "r-kd", "k-2", nutzer="p-in", gruppen=["g-kd"]), ("klasse-ueber-person", "kundendaten", "intern")),
         (pa("kundendaten", "r-kd", "k-2", nutzer="p-kd", gruppen=["g-oe"]), ("frei", "kundendaten", "kundendaten")),
     ]),
    ("p5-reihenfolge", ["P5", "D2", "P1", "P2", "D3", "D4", "D6", "P3", "D7", "D8"],
     "Treffen mehrere Gründe zu, gilt der erste der Tabelle: D2, P1, P2, D3, D4, D6, P3, dann D7 bis D11.",
     V, P, [
         (pa("geheim", "", nutzer=None), ("route-fehlt", None, None)),
         (pa("geheim", "r-x", kontext=FEHLT_KONTEXT), ("person-fehlt", None, None)),
         (pa("geheim", "r-ohne", nutzer="p-x"), ("person-ohne-leseklasse", None, None)),
         (pa("geheim", "r-ohne", nutzer="p-oe"), ("route-ohne-klasse", None, "oeffentlich")),
         (pa("geheim", "r-oe", nutzer="p-oe"), ("einheit-ohne-klasse", None, "oeffentlich")),
         (pa("kundendaten", "r-in", nutzer="p-oe"), ("klasse-ueber-route", "kundendaten", "oeffentlich")),
         (pa("kundendaten", "r-kd", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pa("intern", "r-kd", "k-x", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pa("kundendaten", "r-kd", nutzer="p-kd"), ("kunde-fehlt", "kundendaten", "kundendaten")),
         (pa("intern", "r-kd", "k-x", nutzer="p-kd"), ("kunde-ohne-eintrag", "kundendaten", "kundendaten")),
         (pa("intern", "c-kd", "k-2", nutzer="p-kd"), ("route-fuer-kunde-verboten", "kundendaten", "kundendaten")),
         (pa("intern", "c-kd", "k-2", nutzer="p-oe"), ("klasse-ueber-person", "kundendaten", "oeffentlich")),
     ]),
    ("p6-vorgabestufe", ["P3", "D5", "A35"],
     "Eine Einheit ohne Klasse bekommt die Vorgabestufe intern; eine Person mit Leseklasse oeffentlich bekommt sie nicht.",
     V_STUFE, P, [
         (pa(FEHLT, "r-in", nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pa(None, "r-in", nutzer="p-in"), ("frei", "intern", "intern")),
     ]),
    ("p6-gleich-darf", ["P6", "D1", "D3", "D6", "D7", "D9", "D10", "D11"],
     "Mit der höchsten Leseklasse ist die Antwort gleich darf aus Vertrag 3; mit einer niedrigeren nicht.",
     V, P, [
         (pa("intern", "r-ohne", nutzer="p-kd"), ("route-ohne-klasse", "intern", "kundendaten")),
         (pa("intern", "r-oe", nutzer="p-kd"), ("klasse-ueber-route", "intern", "kundendaten")),
         (pa("kundendaten", "r-kd", nutzer="p-kd"), ("kunde-fehlt", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "c-kd", "k-2", nutzer="p-kd"), ("route-fuer-kunde-verboten", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "r-kd", "k-1", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pa("oeffentlich", "r-oe", nutzer="p-kd"), ("frei", "oeffentlich", "kundendaten")),
         (pa("intern", "r-in", nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
     ]),
    ("p7-vier-klassen", ["P3", "A29"],
     "Vier Klassen mit vertauschten Namen: das Tor vergleicht nur Positionen; s3 ist die niedrigste, s0 die höchste.",
     {"klassen": ["s3", "s2", "s1", "s0"],
      "routen": [route("a", "lokal", "s0"), route("b", "lokal", "s1")], "kunden": [kunde("k", erlaubt=["a"])]},
     {"leseklassen": [lk("nutzer", "p-1", "s2"), lk("gruppe", "g-1", "s3"), lk("gruppe", "g-2", "s1")]}, [
         (pa("s3", "a", nutzer="p-1"), ("frei", "s3", "s2")),
         (pa("s2", "a", nutzer="p-1"), ("frei", "s2", "s2")),
         (pa("s1", "a", nutzer="p-1"), ("klasse-ueber-person", "s1", "s2")),
         (pa("s2", "a", nutzer="p-2", gruppen=["g-1"]), ("klasse-ueber-person", "s2", "s3")),
         (pa("s1", "a", nutzer="p-2", gruppen=["g-1", "g-2"]), ("frei", "s1", "s1")),
         (pa("s3", "a", "k", nutzer="p-2", gruppen=["g-2"]), ("klasse-ueber-person", "s0", "s1")),
     ]),
    ("p7-eine-klasse", ["P3", "D7"],
     "Eine Klasse: sie ist die höchste; die Person liest sie, die Einheit braucht trotzdem einen Kunden.",
     DT3.V_EINE, {"leseklassen": [lk("nutzer", "p-1", "nur")]}, [
         (pa("nur", "r", "k-a", nutzer="p-1"), ("frei", "nur", "nur")),
         (pa("nur", "r", nutzer="p-1"), ("kunde-fehlt", "nur", "nur")),
         (pa("nur", "r", "k-a", nutzer="p-2"), ("person-ohne-leseklasse", "nur", None)),
     ]),
    ("p7-leer", ["P2"], "Ohne einen einzigen Eintrag in leseklassen hat niemand eine Leseklasse; alles ist gesperrt.",
     V, {"leseklassen": []}, [
         (pa("oeffentlich", "r-oe", nutzer="p-oe"), ("person-ohne-leseklasse", "oeffentlich", None)),
         (pa("oeffentlich", "r-oe", nutzer="p-oe", gruppen=["g-oe"]), ("person-ohne-leseklasse", "oeffentlich", None)),
     ]),
    ("pl-lesen", ["P7", "P3", "D5"],
     "Lesen ohne Route (Ordner-Blatt): die Person liest Inhalt bis zu ihrer Leseklasse; Inhalt mit Kunde hat die höchste "
     "Klasse, Inhalt ohne Klasse die Vorgabestufe.",
     V, P, [
         (pl("oeffentlich", nutzer="p-oe"), ("frei", "oeffentlich", "oeffentlich")),
         (pl("intern", nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pl("intern", nutzer="p-in"), ("frei", "intern", "intern")),
         (pl("kundendaten", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pl("oeffentlich", "k-1", nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pl("oeffentlich", "k-x", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pl("kundendaten", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pl(FEHLT, nutzer="p-in"), ("klasse-ueber-person", "kundendaten", "intern")),
         (pl(None, nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pl("intern", nutzer="p-g", gruppen=["g-oe", "g-in"]), ("frei", "intern", "intern")),
         (pl("intern", nutzer="p-tief", gruppen=["g-kd"]), ("klasse-ueber-person", "intern", "oeffentlich")),
     ]),
    ("pl-reihenfolge", ["P7", "P1", "P2", "D4", "P3"],
     "Lesen ohne Route prüft P1, P2, D4, P3 in dieser Reihenfolge; Gründe der Route und der Kunden gibt es nicht.",
     V, P, [
         (pl("geheim", kontext=FEHLT_KONTEXT), ("person-fehlt", None, None)),
         (pl("geheim", nutzer=""), ("person-fehlt", None, None)),
         (pl("geheim", nutzer=None, gruppen=["g-kd"]), ("person-fehlt", None, None)),
         (pl("geheim", nutzer="p-x"), ("person-ohne-leseklasse", None, None)),
         (pl("geheim", nutzer="p-kd"), ("einheit-ohne-klasse", None, "kundendaten")),
         (pl("Intern", nutzer="p-kd"), ("einheit-ohne-klasse", None, "kundendaten")),
         (pl("kundendaten", nutzer="p-oe"), ("klasse-ueber-person", "kundendaten", "oeffentlich")),
         (pl("kundendaten", None, nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
         (pl("intern", "", nutzer="p-kd"), ("frei", "kundendaten", "kundendaten")),
     ]),
    ("pl-vorgabestufe", ["P7", "D5", "A35"],
     "Lesen ohne Route: Inhalt ohne Klasse bekommt die Vorgabestufe intern.",
     V_STUFE, P, [
         (pl(FEHLT, nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pl(FEHLT, nutzer="p-in"), ("frei", "intern", "intern")),
     ]),
    ("pl-pseudonym", ["P4", "P7", "D14"],
     "Die Person steht als Pseudonym der Regelebenen (ps_ und 32 Hexziffern); ein anderes Pseudonym hat keinen Eintrag.",
     V, P_PS, [
         (pl("intern", nutzer=PS_A), ("frei", "intern", "intern")),
         (pa("intern", "r-in", nutzer=PS_A), ("frei", "intern", "intern")),
         (pl("intern", nutzer=PS_B), ("person-ohne-leseklasse", "intern", None)),
         (pl("intern", nutzer=PS_B, gruppen=["g-alle"]), ("klasse-ueber-person", "intern", "oeffentlich")),
         (pl("intern", nutzer=PS_A.upper()), ("person-ohne-leseklasse", "intern", None)),
     ]),
    ("p-4096-eintraege", ["P0", "P4"],
     "Genau 4096 Einträge sind gültig; der letzte zählt wie jeder andere.",
     V, P_VOLL, [
         (pl("kundendaten", nutzer="p-letzt"), ("frei", "kundendaten", "kundendaten")),
         (pl("kundendaten", nutzer="p-x", gruppen=["g4094"]), ("klasse-ueber-person", "kundendaten", "intern")),
         (pa("kundendaten", "r-kd", nutzer="p-x"), ("person-ohne-leseklasse", "kundendaten", None)),
     ]),
    ("p-tiefe-64", ["D13"], "Verschachtelung genau 64; das oberste Objekt ist Ebene 1.",
     V, P, [
         (pa("intern", "r-in", nutzer="p-oe"), ("klasse-ueber-person", "intern", "oeffentlich")),
     ]),
]
TIEFE = {"p-tiefe-64": 64}
KOMPAKT = {"p-4096-eintraege"}  # written without indent, so that the file stays small


# ---------------------------------------------------------------- invalid cases (exit 2)

def basis():
    return {"vorgabe": V, "personen": P, "anfragen": [pa("intern", "r-in", nutzer="p-in", gruppen=["g-oe"])]}


def ungueltig(name, beschreibung, aendern, regeln=("D13",)):
    fall = json.loads(json.dumps(basis()))
    aendern(fall)
    return (name, list(regeln), beschreibung, fall)


setze, loesche = DT3.setze, DT3.loesche
LR = ["personen", "leseklassen"]
A0 = ["anfragen", 0]

UNGUELTIG = [
    ungueltig("u-p-kein-json", "", lambda f: None),  # replaced by raw bytes below
    ungueltig("u-p-feld-im-fall", "Ein unbekanntes Feld im Fall.", setze(["zusatz"], 1)),
    ungueltig("u-p-vorgabe-ungueltig", "Die Vorgabe ist nach Vertrag 3 ungültig.", setze(["vorgabe", "klassen"], [])),
    ungueltig("u-p-ohne-personen", "Das Feld personen fehlt; es gibt keine zweite Vorgabe.", loesche(["personen"])),
    ungueltig("u-p-personen-liste", "personen ist kein Objekt.", setze(["personen"], [])),
    ungueltig("u-p-personen-feld", "Ein unbekanntes Feld in personen.", setze(["personen", "kunden"], [])),
    ungueltig("u-p-ohne-leseklassen", "Das Pflichtfeld leseklassen fehlt.", loesche(LR)),
    ungueltig("u-p-leseklassen-null", "leseklassen ist null.", setze(LR, None)),
    ungueltig("u-p-leseklassen-objekt", "leseklassen ist ein Objekt statt einer Liste.", setze(LR, {"p-in": "intern"})),
    ungueltig("u-p-eintrag-text", "Ein Eintrag ist kein Objekt.", setze(LR + [0], "p-oe")),
    ungueltig("u-p-eintrag-ohne-klasse", "Einem Eintrag fehlt klasse.", loesche(LR + [0, "klasse"])),
    ungueltig("u-p-eintrag-ohne-wer", "Einem Eintrag fehlt wer.", loesche(LR + [1, "wer"])),
    ungueltig("u-p-eintrag-ohne-ebene", "Einem Eintrag fehlt ebene.", loesche(LR + [2, "ebene"])),
    ungueltig("u-p-klasse-unbekannt", "Ladefehler: Ein Eintrag gibt einer Person eine Klasse, die es nicht gibt.",
              setze(LR + [1, "klasse"], "geheim"), ("D13", "P0")),
    ungueltig("u-p-klasse-gross", "Ladefehler: Intern ist nicht intern.", setze(LR + [1, "klasse"], "Intern"), ("D13", "P0")),
    ungueltig("u-p-klasse-null", "Ladefehler: Die Klasse eines Eintrags ist null.", setze(LR + [3, "klasse"], None),
              ("D13", "P0")),
    ungueltig("u-p-klasse-zahl", "Ladefehler: Die Klasse eines Eintrags ist eine Zahl.", setze(LR + [3, "klasse"], 1),
              ("D13", "P0")),
    ungueltig("u-p-klasse-liste", "Ladefehler: Die Klasse eines Eintrags ist eine Liste.", setze(LR + [3, "klasse"], ["intern"]),
              ("D13", "P0")),
    ungueltig("u-p-ebene-firma", "Die Ebene ist weder nutzer noch gruppe.", setze(LR + [0, "ebene"], "firma")),
    ungueltig("u-p-wer-form", "wer hat nicht die Schlüsselform.", setze(LR + [0, "wer"], "P-OE")),
    ungueltig("u-p-wer-liste", "wer ist eine Liste.", setze(LR + [0, "wer"], ["p-oe"])),
    ungueltig("u-p-eintrag-feld", "Ein Eintrag trägt ein unbekanntes Feld (Kunden je Person gehören nicht in diesen Vertrag).",
              setze(LR + [0, "kunden"], ["k-1"])),
    ungueltig("u-p-eintrag-doppelt", "Dieselbe Person steht zweimal, mit verschiedenen Klassen.",
              lambda f: f["personen"]["leseklassen"].append(lk("nutzer", "p-in", "kundendaten"))),
    ungueltig("u-p-eintrag-doppelt-gleich", "Dieselbe Gruppe steht zweimal mit derselben Klasse; auch das ist ein Doppel.",
              lambda f: f["personen"]["leseklassen"].append(lk("gruppe", "g-oe", "oeffentlich"))),
    ungueltig("u-p-anfrage-feld", "Eine Anfrage trägt ein unbekanntes Feld person; die Person steht im kontext.",
              setze(A0 + ["person"], "p-in")),
    ungueltig("u-p-kontext-liste", "kontext ist kein Objekt.", setze(A0 + ["kontext"], [])),
    ungueltig("u-p-kontext-null", "kontext ist null; ein fehlender kontext heißt keine Person.", setze(A0 + ["kontext"], None)),
    ungueltig("u-p-kontext-feld", "kontext trägt ein Feld, das dieser Vertrag nicht kennt.", setze(A0 + ["kontext", "jetzt"], 1)),
    ungueltig("u-p-nutzer-zahl", "nutzer ist weder Text noch null.", setze(A0 + ["kontext", "nutzer"], 1)),
    ungueltig("u-p-gruppen-null", "gruppen ist null; die leere Liste heißt keine Gruppe.", setze(A0 + ["kontext", "gruppen"], None)),
    ungueltig("u-p-gruppe-zahl", "Eine Gruppe ist kein Text.", setze(A0 + ["kontext", "gruppen"], [1])),
    ungueltig("u-p-einheit-null", "einheit ist null; dieser Vertrag kennt keinen Lauf ohne Einheit.", setze(A0 + ["einheit"], None)),
    ungueltig("u-p-einheit-feld", "Die Einheit trägt ein unbekanntes Feld.", setze(A0 + ["einheit", "kunde"], "k-1")),
    ungueltig("u-p-route-zahl", "route ist weder Text noch null.", setze(A0 + ["route"], 1)),
    ungueltig("u-p-kunde-liste", "kunde ist weder Text noch null.", setze(A0 + ["kunde"], ["k-1"])),
    ungueltig("u-p-lesen-mit-route", "Ein Lesen ohne Route nennt eine Route.", setze(A0 + ["lesen"], True)),
    ungueltig("u-p-lesen-route-null", "Ein Lesen ohne Route nennt route, auch als null.",
              lambda f: f["anfragen"].append(dict(pl("intern", nutzer="p-in"), route=None))),
    ungueltig("u-p-lesen-falsch", "lesen hat nur den Wert true; false ist ungültig.",
              lambda f: f["anfragen"].append(dict(pl("intern", nutzer="p-in"), lesen=False))),
    ungueltig("u-p-lesen-text", "lesen ist ein Text.", lambda f: f["anfragen"].append(dict(pl("intern", nutzer="p-in"), lesen="ja"))),
    ungueltig("u-p-lesen-null", "lesen ist null.", lambda f: f["anfragen"].append(dict(pl("intern", nutzer="p-in"), lesen=None))),
    ungueltig("u-p-zu-viele", "4097 Einträge: mehr als die Grenze von 4096.",
              setze(LR, P_VOLL["leseklassen"] + [lk("nutzer", "p-mehr", "intern")]), ("D13", "P0")),
    ungueltig("u-p-zu-tief", "Verschachtelung 65 im nicht ausgewerteten Feld beschreibung.", setze(["beschreibung"], DT3.tief(63))),
]
UNGUELTIG[0] = ("u-p-kein-json", ["D13"], "Die Eingabe ist kein JSON.", b'{"personen": ')

# duplicate keys: (name, description, text in the dumped base case, text sent)
DOPPEL = [
    ("u-p-doppel-klasse", "Ein Eintrag nennt klasse zweimal, das zweite Mal als kl\\u0061sse; der zweite Wert wäre lockerer.",
     '"wer": "p-oe", "klasse": "oeffentlich"', '"wer": "p-oe", "klasse": "oeffentlich", "kl\\u0061sse": "kundendaten"'),
    ("u-p-doppel-nutzer", "Der Kontext nennt nutzer zweimal.", '"nutzer": "p-in"', '"nutzer": "p-in", "nutzer": "p-kd"'),
    ("u-p-doppel-personen", "Der Fall nennt personen zweimal.", '"personen": {',
     '"personen": {"leseklassen": []}, "personen": {'),
]


# ---------------------------------------------------------------- reference (only to check the tables above)

def ist_schl(x):
    return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None


def gueltig_personen(v, p):
    """True if personen is valid by person/VERTRAG.md, section 3 (the vorgabe is already valid)."""
    if not isinstance(p, dict) or set(p) != {"leseklassen"} or not isinstance(p["leseklassen"], list):
        return False
    if len(p["leseklassen"]) > 4096:
        return False
    gesehen = set()
    for e in p["leseklassen"]:
        if not isinstance(e, dict) or set(e) != {"ebene", "wer", "klasse"}:
            return False
        if not isinstance(e["ebene"], str) or e["ebene"] not in EBENEN or not ist_schl(e["wer"]):
            return False
        if not isinstance(e["klasse"], str) or e["klasse"] not in v["klassen"]:
            return False
        if (e["ebene"], e["wer"]) in gesehen:
            return False
        gesehen.add((e["ebene"], e["wer"]))
    return True


def gueltige_anfrage(a):
    if not isinstance(a, dict) or not set(a) <= {"einheit", "route", "kunde", "kontext", "lesen"} or "einheit" not in a:
        return False
    if "lesen" in a and (a["lesen"] is not True or "route" in a):
        return False
    e = a["einheit"]
    if not isinstance(e, dict) or not set(e) <= {"klasse"}:
        return False
    if not all(isinstance(a.get(f), (str, type(None))) for f in ("route", "kunde")):
        return False
    k = a.get("kontext", {})
    if not isinstance(k, dict) or not set(k) <= {"nutzer", "gruppen"} or not isinstance(k.get("nutzer"), (str, type(None))):
        return False
    g = k.get("gruppen", [])
    return isinstance(g, list) and all(isinstance(x, str) for x in g)


def gueltiger_fall(fall):
    if not isinstance(fall, dict) or not set(fall) <= FALL_FELDER:
        return False
    if not {"vorgabe", "personen", "anfragen"} <= set(fall) or not DT3.gueltig(fall["vorgabe"]):
        return False
    if not gueltig_personen(fall["vorgabe"], fall["personen"]) or not isinstance(fall["anfragen"], list):
        return False
    return all(gueltige_anfrage(a) for a in fall["anfragen"]) and DT3.tiefe(fall) <= 64


def leseklasse(v, p, kx):
    """Read class of the person by P4, or None."""
    nutzer, gruppen = kx.get("nutzer"), kx.get("gruppen", [])
    for e in p["leseklassen"]:
        if e["ebene"] == "nutzer" and e["wer"] == nutzer:
            return e["klasse"]
    kl = v["klassen"]
    aus_gruppen = [kl.index(e["klasse"]) for e in p["leseklassen"] if e["ebene"] == "gruppe" and e["wer"] in gruppen]
    return kl[max(aus_gruppen)] if aus_gruppen else None


def wirksam_lesen(v, a):
    """D5 for a read without a route: effective class or None for a class that is given but unknown."""
    kl, e = v["klassen"], a["einheit"]
    k = e.get("klasse")
    if k is None:
        eigene = v.get("vorgabestufe", kl[-1])
    elif isinstance(k, str) and k in kl:
        eigene = k
    else:
        return None
    return kl[-1] if a.get("kunde") is not None else eigene


def referenz_lesen(v, p, a, mit_person=True):
    """(reason, effective class, read class) by P7 (Nachtrag 27.11.2026): P1, P2, D4, P3, D1. mit_person=False: D4 or D1."""
    klasse = wirksam_lesen(v, a)
    kx = a.get("kontext") or {}
    lese = leseklasse(v, p, kx) if kx.get("nutzer") else None
    if not mit_person:
        return ("einheit-ohne-klasse" if klasse is None else "frei"), klasse, lese
    kl = v["klassen"]
    if not kx.get("nutzer"):
        return "person-fehlt", klasse, lese
    if lese is None:
        return "person-ohne-leseklasse", klasse, lese
    if klasse is None:
        return "einheit-ohne-klasse", klasse, lese
    if kl.index(klasse) > kl.index(lese):
        return "klasse-ueber-person", klasse, lese
    return "frei", klasse, lese


def referenz(v, p, a, mit_person=True):
    """(reason, effective class, read class) by person/VERTRAG.md, section 4. mit_person=False: darf of contract 3."""
    if a.get("lesen"):
        return referenz_lesen(v, p, a, mit_person)
    grund, klasse = DT3.referenz(v, {"einheit": a["einheit"], "route": a.get("route"), "kunde": a.get("kunde")})
    kx = a.get("kontext") or {}
    lese = leseklasse(v, p, kx) if kx.get("nutzer") else None
    if not mit_person:
        return grund, klasse, lese
    kl = v["klassen"]
    if grund == "route-fehlt":
        return grund, klasse, lese
    if not kx.get("nutzer"):
        return "person-fehlt", klasse, lese
    if lese is None:
        return "person-ohne-leseklasse", klasse, lese
    if grund in ("route-ohne-klasse", "einheit-ohne-klasse", "klasse-ueber-route"):
        return grund, klasse, lese
    if kl.index(klasse) > kl.index(lese):
        return "klasse-ueber-person", klasse, lese
    return grund, klasse, lese


def antwort(grund, klasse, route_, lese):
    text = MELDUNG[grund]
    if "{route}" in text:
        text = text.replace("{route}", route_)
    return {"darf": grund == "frei", "grund": grund, "klasse": klasse, "meldung": text, "leseklasse": lese}


def doppel_pruefen(roh):
    """True if a duplicate key exists after decoding and both readings (first and last value) give a valid case."""
    gefunden = []

    def merke(paare):
        ks = [k for k, _ in paare]
        if len(ks) != len(set(ks)):
            gefunden.append(ks)
        return dict(paare)

    def erster(paare):
        d = {}
        for k, w in paare:
            d.setdefault(k, w)
        return d

    letzter = json.loads(roh, object_pairs_hook=merke)
    erst = json.loads(roh, object_pairs_hook=erster)
    return bool(gefunden) and gueltiger_fall(letzter) and gueltiger_fall(erst)


def main():
    dateien, namen = {}, set()
    anfragen_n = abweichend = 0
    for name, regeln, beschreibung, v, p, paare in FAELLE:
        assert name not in namen, name
        namen.add(name)
        fall = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v, "personen": p,
                "anfragen": [a for a, _ in paare]}
        if name in TIEFE:
            fall["beschreibung"] = DT3.tief(TIEFE[name] - 2)
            assert DT3.tiefe(fall) == TIEFE[name], (name, DT3.tiefe(fall))
        assert gueltiger_fall(json.loads(json.dumps(fall))), f"{name}: Fall ungueltig"
        antworten, nur_mit_person = [], 0
        for a, erw in paare:
            a = json.loads(json.dumps(a))
            ref = referenz(v, p, a)
            assert ref == erw, f"{name}: {a} erwartet {erw}, Referenz {ref}"
            ohne = referenz(v, p, a, mit_person=False)
            nur_mit_person += ohne[0] != erw[0]
            antworten.append(antwort(erw[0], erw[1], a.get("route"), erw[2]))
        assert nur_mit_person, f"{name}: keine Anfrage haengt an der Personenregel"
        anfragen_n += len(paare)
        abweichend += nur_mit_person
        einzug = None if name in KOMPAKT else 1
        dateien[name] = (json.dumps(fall, ensure_ascii=False, indent=einzug).encode() + b"\n",
                         {"exit": 0, "ausgabe": {"antworten": antworten}})
    assert gueltiger_fall(json.loads(json.dumps(basis()))), "Basis der ungueltigen Faelle ungueltig"
    for name, regeln, beschreibung, fall in UNGUELTIG:
        assert name not in namen, name
        namen.add(name)
        if isinstance(fall, bytes):
            roh = fall
        else:
            assert not gueltiger_fall(fall), f"{name}: Fall waere gueltig"
            roh = json.dumps(fall, ensure_ascii=False).encode() + b"\n"
        dateien[name] = (roh, {"exit": 2, "regeln": regeln, "beschreibung": beschreibung})
    text = json.dumps(basis(), ensure_ascii=False)
    for name, beschreibung, alt, neu in DOPPEL:
        assert name not in namen, name
        namen.add(name)
        assert text.count(alt) == 1, (name, text.count(alt))
        roh = text.replace(alt, neu).encode() + b"\n"
        assert doppel_pruefen(roh), f"{name}: kein reines Doppel"
        dateien[name] = (roh, {"exit": 2, "regeln": ["D13", "A139"], "beschreibung": beschreibung})
    if ZIEL.exists():
        shutil.rmtree(ZIEL)
    (ZIEL / "faelle").mkdir(parents=True)
    (ZIEL / "erwartet").mkdir(parents=True)
    for name, (roh, erw) in sorted(dateien.items()):
        (ZIEL / "faelle" / f"{name}.json").write_bytes(roh)
        (ZIEL / "erwartet" / f"{name}.json").write_text(json.dumps(erw, ensure_ascii=False, indent=1) + "\n",
                                                       encoding="utf-8")
    gueltige = sum(1 for _, (_, e) in dateien.items() if e["exit"] == 0)
    print(f"{len(dateien)} Faelle ({gueltige} gueltig mit {anfragen_n} Anfragen, {len(dateien) - gueltige} ungueltig)")
    print(f"Gegenprobe ohne Personenregel: {abweichend} von {anfragen_n} Anfragen anders, "
          f"{gueltige} von {gueltige} gueltigen Faellen rot")
    return 0


if __name__ == "__main__":
    sys.exit(main())
