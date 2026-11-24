"""Builds the visible cases of the partial contract `datentor klassen` (klassen/VERTRAG.md).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json (the input on stdin of `datentor klassen`) and sichtbar/erwartet/<name>.json (exit code
and expected output). The decisions of every expectation are written out by hand in the tables below: class, origin,
reason of the lowering and reason of `darf`. The writer adds only what follows mechanically from them (proposal, log
fields, message, delivered parts). The script then checks each full answer against a small reference of the rules in
klassen/VERTRAG.md and each case against the validity rules; a mismatch stops the script before anything is written.
The rules of contract 3 (`darf`, validity of `vorgabe`) come from ../faelle_bauen.py. All data is invented.
"""
import importlib.util
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "sichtbar"

_spec = importlib.util.spec_from_file_location("vertrag3", HIER.parent / "faelle_bauen.py")
V3 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V3)

SCHLUESSEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
ARTEN = ["regel", "bedienwissen", "oeffentliche-quelle", "leere-vorlage"]  # A39, default of the list
# A40, fixed by the contract and not configurable (Nachtrag 20.11., F3)
NIE = ["personenbezug", "kundendaten", "geheimnis", "zugangsdaten", "vertrag", "preis", "finanzen", "netz-und-system",
       "gespraechsprotokoll", "sitzungsprotokoll", "entscheidung", "plan", "organisation", "quelltext", "entwurf"]
INHALT = {"klasse", "kunde", "art", "befund", "modell"}
LAGE = {"geerbt", "gruppe", "senkung_nutzer", "bestaetigt"}
BEFUND = {"geheimnis", "personenbezug", "interner_name"}
MODELL = {"frei", "verhindert", "senken"}
NUTZER = {"folgt", "aus", "bestaetigung"}
TEILE_MAX = 64
FALL_FELDER = {"name", "regeln", "beschreibung", "vorgabe", "tiefe", "anfragen"}
FEHLT = V3.FEHLT

KL3 = V3.KL3
route, kunde = V3.route, V3.kunde

# ---------------------------------------------------------------- settings of contract 3 used by the cases
V = {
    "klassen": KL3,
    "routen": [route("r-oeff", "lokal", "oeffentlich"), route("r-intern", "lokal", "intern"),
               route("r-kd", "lokal", "kundendaten"), route("cloud", "cloud", "intern")],
    "kunden": [kunde("k-a", erlaubt=["r-kd"]), kunde("k-b", erlaubt=["r-kd"])],
}
V_STUFE_INTERN = dict(V, vorgabestufe="intern")
V4 = {"klassen": ["s0", "s1", "s2", "s3"],
      "routen": [route("r0", "lokal", "s0"), route("r1", "lokal", "s1"), route("r2", "lokal", "s2"),
                 route("r3", "lokal", "s3")],
      "kunden": [kunde("k-a", erlaubt=["r3"])]}
V_EINE = V3.V_EINE

# ---------------------------------------------------------------- options of this contract
T_AN = {"herabstufen": {"an": True}}
T_AN_ZWEI = {"herabstufen": {"an": True, "arten": ["regel", "bedienwissen"]}}
T_AN_LEER = {"herabstufen": {"an": True, "arten": []}}
T_AN_G = {"herabstufen": {"an": True, "gruppen": [{"schluessel": "g-a", "streicht": ["regel", "regel"]},
                                                  {"schluessel": "g-b"}]}}
T_AN_G_FREMD = {"herabstufen": {"an": True, "arten": ["regel"],
                                "gruppen": [{"schluessel": "g-a", "streicht": ["bedienwissen"]}]}}
T_AN_EXPLIZIT_AUS = {"aufteilen": True, "herabstufen": {"an": False, "arten": ARTEN, "gruppen": []}}
T_TEILEN_AUS = {"aufteilen": False}
T_TEILEN_AN = {"aufteilen": True}
T_LISTE_OHNE_SCHALTER = {"herabstufen": {"arten": ["regel", "glossar"]}}
T_EIGENE_ART = {"herabstufen": {"an": True, "arten": ["regel", "glossar"],
                                "gruppen": [{"schluessel": "g-a", "streicht": ["glossar"]},
                                            {"schluessel": "g-b", "streicht": ["faq"]}]}}
T_AN_TEILEN_AUS = {"aufteilen": False, "herabstufen": {"an": True}}
T_NIE_ABWANDLUNG = {"herabstufen": {"an": True, "arten": ["vertraege", "kunden-daten", "vertrag."]}}  # Nachtrag 20.11., 2

BEF0 = {"geheimnis": False, "personenbezug": False, "interner_name": False}


def bef(**wahr):
    return dict(BEF0, **{k: True for k in wahr})


def eh(**felder):
    """One unit; FEHLT leaves a field out. `nutzer` is written as senkung_nutzer."""
    if "nutzer" in felder:
        felder["senkung_nutzer"] = felder.pop("nutzer")
    return {k: w for k, w in felder.items() if w is not FEHLT}


def senkbar(**felder):
    """A unit the lowering may lower when the option is on: inherited intern, kind regel, nothing found."""
    basis = {"geerbt": "intern", "art": "regel", "befund": BEF0}
    basis.update(felder)
    return eh(**basis)


def anf(einheit, route_=FEHLT):
    a = {"einheit": einheit}
    if route_ is not FEHLT:
        a["route"] = route_
    return a


def T(*paare):
    """Expectation for a unit with parts and splitting on: one tuple per part."""
    return ("teile", list(paare))


# (name, rules, description, vorgabe, tiefe or None, [(request, expectation)])
# expectation of a unit: (klasse, herkunft, senkung, darf-grund); with parts and splitting on: T(tuple, ...)
FAELLE = [
    # K1: base class, inheritance and vorgabestufe (A32, A35)
    ("k1-vererbung", ["K1", "A32"],
     "Ort P intern, Ort Q kundendaten: ohne eigene Klasse erbt die Einheit, eine eigene Klasse geht vor.",
     V, None, [
         (anf(eh(geerbt="intern"), "r-intern"), ("intern", "geerbt", "aus", "frei")),
         (anf(eh(geerbt="intern", klasse="oeffentlich"), "r-oeff"), ("oeffentlich", "eigen", "aus", "frei")),
         (anf(eh(geerbt="kundendaten"), "r-kd"), ("kundendaten", "geerbt", "aus", "kunde-fehlt")),
         (anf(eh(geerbt="intern"), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
         (anf(eh(geerbt="oeffentlich", klasse="intern"), "r-oeff"), ("intern", "eigen", "aus", "klasse-ueber-route")),
         (anf(eh(geerbt="oeffentlich", klasse=None), "r-oeff"), ("oeffentlich", "geerbt", "aus", "frei")),
     ]),
    ("k1-vorgabestufe-fehlt", ["K1", "A35"],
     "Weder eigene noch geerbte Klasse: ohne Feld vorgabestufe gilt die höchste Klasse.",
     V, None, [
         (anf(eh(), "r-intern"), ("kundendaten", "vorgabestufe", "aus", "klasse-ueber-route")),
         (anf(eh(geerbt=None, klasse=None), "r-kd"), ("kundendaten", "vorgabestufe", "aus", "kunde-fehlt")),
     ]),
    ("k1-vorgabestufe-intern", ["K1", "A35"],
     "Der Admin setzt die Vorgabestufe auf intern; dieselbe Einheit kommt über r-intern an, eine geerbte Klasse geht vor.",
     V_STUFE_INTERN, None, [
         (anf(eh(), "r-intern"), ("intern", "vorgabestufe", "aus", "frei")),
         (anf(eh(geerbt="kundendaten"), "r-intern"), ("kundendaten", "geerbt", "aus", "klasse-ueber-route")),
     ]),
    ("k1-ungueltig", ["K1", "D4", "D14"],
     "Eine eigene oder geerbte Klasse, die kein Text ist oder nicht in klassen steht, sperrt; auch wenn die andere gültig ist.",
     V, None, [
         (anf(eh(klasse="geheim"), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(klasse="intern", geerbt="geheim"), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(geerbt=5), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(geerbt="INTERN"), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(klasse=""), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(klasse=["intern"]), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(klasse="geheim", kunde="k-a"), "r-kd"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(klasse="geheim")), (None, "ungueltig", "klasse-ungueltig", "route-fehlt")),
         (anf(eh(klasse="geheim"), "r-x"), (None, "ungueltig", "klasse-ungueltig", "route-ohne-klasse")),
     ]),
    # K2: a customer raises to the highest class (D5)
    ("k2-kunde-hebt", ["K2", "D5"],
     "Eine Einheit mit Kunde hat die höchste Klasse, gleich ob eigen, geerbt oder Vorgabestufe; ein leerer Text ist ein Kunde.",
     V_STUFE_INTERN, None, [
         (anf(eh(klasse="intern", kunde="k-a"), "r-kd"), ("kundendaten", "kunde", "aus", "frei")),
         (anf(eh(geerbt="oeffentlich", kunde="k-a"), "r-intern"), ("kundendaten", "kunde", "aus", "klasse-ueber-route")),
         (anf(eh(kunde="k-a"), "r-kd"), ("kundendaten", "kunde", "aus", "frei")),
         (anf(eh(geerbt="intern", kunde=""), "r-kd"), ("kundendaten", "kunde", "aus", "kunde-ohne-eintrag")),
         (anf(eh(geerbt="intern", kunde=None), "r-intern"), ("intern", "geerbt", "aus", "frei")),
         (anf(eh(geerbt="kundendaten", kunde="k-b"), "r-kd"), ("kundendaten", "kunde", "aus", "frei")),
     ]),
    # K3: lowering by the automatic (A39, A40, A41)
    ("k3-vorgabe-aus", ["K3", "H1", "A39"],
     "Ohne Feld tiefe ist das Herabstufen aus (Entscheidung vom 20.11.2026); auch ein Vorschlag des lokalen Modells senkt nichts.",
     V, None, [
         (anf(senkbar(), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
         (anf(senkbar(modell="senken"), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
         (anf(senkbar(), "r-intern"), ("intern", "geerbt", "aus", "frei")),
         (anf(senkbar(klasse="geheim"), "r-intern"), (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
     ]),
    ("k3-explizit-aus", ["K3", "H1"],
     "Alle Felder von tiefe ausgeschrieben, an ist false: nichts wird gesenkt.",
     V, T_AN_EXPLIZIT_AUS, [
         (anf(senkbar(), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
     ]),
    ("k3-gesenkt", ["K3", "H0", "K5", "A39"],
     "Option an: eine geerbte Einheit jeder herabstufbaren Art ohne Befund sinkt genau eine Stufe; das Protokoll nennt Art und alte Klasse.",
     V, T_AN, [
         (anf(senkbar(), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="bedienwissen"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="oeffentliche-quelle"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="leere-vorlage"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(modell="frei"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(modell="senken"), "cloud"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(modell=None), "r-intern"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art=FEHLT), "r-oeff"), ("intern", "geerbt", "art-unbekannt", "klasse-ueber-route")),
     ]),
    ("k3-eine-stufe", ["K3", "H0", "H4", "H3", "H2", "A40"],
     "Vier Klassen: s2 sinkt auf s1, nie auf s0; s1 auf s0; s0 bleibt; s3 bleibt; eine eigene Klasse sinkt nie weiter. Wiederholt gleich.",
     V4, T_AN, [
         (anf(senkbar(geerbt="s2"), "r1"), ("s1", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(geerbt="s2"), "r0"), ("s1", "gesenkt", "gesenkt", "klasse-ueber-route")),
         (anf(senkbar(geerbt="s2"), "r0"), ("s1", "gesenkt", "gesenkt", "klasse-ueber-route")),
         (anf(senkbar(geerbt="s1"), "r0"), ("s0", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(geerbt="s0"), "r0"), ("s0", "geerbt", "niedrigste", "frei")),
         (anf(senkbar(geerbt="s3"), "r2"), ("s3", "geerbt", "kundendaten", "klasse-ueber-route")),
         (anf(senkbar(geerbt="s2", klasse="s1"), "r0"), ("s1", "eigen", "nicht-geerbt", "klasse-ueber-route")),
         (anf(senkbar(geerbt="s3", klasse="s2"), "r1"), ("s2", "eigen", "nicht-geerbt", "klasse-ueber-route")),
     ]),
    ("k3-eine-klasse", ["K3", "H3"], "Mit nur einer Klasse ist sie die höchste; nichts sinkt.",
     V_EINE, T_AN, [
         (anf(senkbar(geerbt="nur"), "r"), ("nur", "geerbt", "kundendaten", "kunde-fehlt")),
     ]),
    ("k3-nicht-geerbt", ["K3", "H2"],
     "Die Automatik senkt nur unter die geerbte Klasse: eine eigene Klasse und die Vorgabestufe bleiben.",
     V_STUFE_INTERN, T_AN, [
         (anf(senkbar(geerbt=FEHLT, klasse="intern"), "r-oeff"), ("intern", "eigen", "nicht-geerbt", "klasse-ueber-route")),
         (anf(senkbar(geerbt=FEHLT), "r-oeff"), ("intern", "vorgabestufe", "nicht-geerbt", "klasse-ueber-route")),
         (anf(senkbar(geerbt=None), "r-intern"), ("intern", "vorgabestufe", "nicht-geerbt", "frei")),
         (anf(senkbar(klasse="intern"), "r-oeff"), ("intern", "eigen", "nicht-geerbt", "klasse-ueber-route")),
     ]),
    ("k3-kundendaten", ["K3", "H3", "K2", "A40"],
     "Kundendaten sinken nie: weder mit Kunde noch mit geerbter höchster Klasse; der Kunde hebt weiter auf die höchste Klasse.",
     V, T_AN, [
         (anf(senkbar(kunde="k-a"), "r-kd"), ("kundendaten", "kunde", "kundendaten", "frei")),
         (anf(senkbar(kunde="k-a"), "r-intern"), ("kundendaten", "kunde", "kundendaten", "klasse-ueber-route")),
         (anf(senkbar(geerbt="kundendaten"), "r-kd"), ("kundendaten", "geerbt", "kundendaten", "kunde-fehlt")),
         (anf(senkbar(kunde=""), "r-kd"), ("kundendaten", "kunde", "kundendaten", "kunde-ohne-eintrag")),
         (anf(senkbar(geerbt="oeffentlich", kunde="k-a"), "r-kd"), ("kundendaten", "kunde", "kundendaten", "frei")),
     ]),
    ("k3-nie-arten", ["K3", "H5", "H7", "A40", "D14"],
     "Arten der Nie-Liste, unbekannte Arten und Abweichungen in der Schreibweise sinken nie; ein Modellvorschlag ändert nichts.",
     V, T_AN, [
         (anf(senkbar(art="vertrag"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="personenbezug"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="sitzungsprotokoll"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="quelltext", modell="senken"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="Regel"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="regel "), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art=""), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art=None), "r-oeff"), ("intern", "geerbt", "art-unbekannt", "klasse-ueber-route")),
     ]),
    ("k3-befund", ["K3", "H8", "A40"],
     "Findet der Filterkern ein Geheimnis, einen Personenbezug oder einen internen Namen, bleibt die geerbte Klasse; ohne Befund ebenso.",
     V, T_AN, [
         (anf(senkbar(befund=bef(geheimnis=1)), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(befund=bef(personenbezug=1)), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(befund=bef(interner_name=1)), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(befund=bef(geheimnis=1, personenbezug=1, interner_name=1)), "r-oeff"),
          ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(befund=FEHLT), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(befund=None), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
     ]),
    ("k3-modell", ["K3", "H9", "A40"],
     "Ein lokales Modell darf eine Senkung nur verhindern, nie auslösen.",
     V, T_AN, [
         (anf(senkbar(modell="verhindert"), "r-oeff"), ("intern", "geerbt", "modell-verhindert", "klasse-ueber-route")),
         (anf(senkbar(modell="senken", befund=bef(geheimnis=1)), "r-oeff"), ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(modell="senken", art=FEHLT), "r-oeff"), ("intern", "geerbt", "art-unbekannt", "klasse-ueber-route")),
     ]),
    ("k3-admin-liste", ["K3", "H7", "A39"],
     "Der Admin streicht zwei Arten aus der Liste; die übrigen sinken weiter.",
     V, T_AN_ZWEI, [
         (anf(senkbar(), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="bedienwissen"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="leere-vorlage"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="oeffentliche-quelle"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
     ]),
    ("k3-admin-liste-leer", ["K3", "H7", "A39"], "Eine leere Liste der Arten: nichts sinkt.",
     V, T_AN_LEER, [
         (anf(senkbar(), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
     ]),
    ("k3-gruppe", ["K3", "H6", "H7", "K5", "A41"],
     "Gruppe g-a streicht regel (doppelt genannt, gleiche Wirkung); g-b streicht nichts; eine Gruppe ohne Eintrag senkt nie.",
     V, T_AN_G, [
         (anf(senkbar(gruppe="g-a"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(gruppe="g-a", art="bedienwissen"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(gruppe="g-b"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(gruppe=None), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(gruppe="g-x"), "r-oeff"), ("intern", "geerbt", "gruppe-ohne-eintrag", "klasse-ueber-route")),
         (anf(senkbar(gruppe="G-A"), "r-oeff"), ("intern", "geerbt", "gruppe-ohne-eintrag", "klasse-ueber-route")),
         (anf(senkbar(gruppe=""), "r-oeff"), ("intern", "geerbt", "gruppe-ohne-eintrag", "klasse-ueber-route")),
     ]),
    ("k3-gruppe-fuegt-nicht-hinzu", ["K3", "H7", "A41"],
     "Die Firma erlaubt nur regel; eine Gruppe, die bedienwissen streicht, fügt dadurch nichts hinzu.",
     V, T_AN_G_FREMD, [
         (anf(senkbar(gruppe="g-a", art="bedienwissen"), "r-oeff"),
          ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(gruppe="g-a"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="bedienwissen"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
     ]),
    ("k3-nutzer", ["K3", "H10", "H11", "A41"],
     "Der Nutzer schaltet für seine Einheiten ab oder verlangt Bestätigung; erst nach der Bestätigung sinkt die Klasse.",
     V, T_AN, [
         (anf(senkbar(nutzer="aus"), "r-oeff"), ("intern", "geerbt", "nutzer-aus", "klasse-ueber-route")),
         (anf(senkbar(nutzer="aus", bestaetigt=True), "r-oeff"), ("intern", "geerbt", "nutzer-aus", "klasse-ueber-route")),
         (anf(senkbar(nutzer="bestaetigung"), "r-oeff"), ("intern", "geerbt", "bestaetigung-fehlt", "klasse-ueber-route")),
         (anf(senkbar(nutzer="bestaetigung", bestaetigt=False), "r-oeff"),
          ("intern", "geerbt", "bestaetigung-fehlt", "klasse-ueber-route")),
         (anf(senkbar(nutzer="bestaetigung", bestaetigt=True), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(nutzer="folgt"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(nutzer="folgt", bestaetigt=False), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(nutzer="bestaetigung", art="vertrag"), "r-oeff"),
          ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
     ]),
    ("k3-reihenfolge", ["K3", "D12"],
     "Treffen mehrere Gründe zu, gilt der erste in der Reihenfolge der Tabelle.",
     V, T_AN, [
         (anf(senkbar(klasse="geheim", kunde="k-a", art="vertrag"), "r-kd"),
          (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(senkbar(klasse="intern", kunde="k-a"), "r-kd"), ("kundendaten", "kunde", "nicht-geerbt", "frei")),
         (anf(senkbar(kunde="k-a", art="vertrag", befund=bef(geheimnis=1), nutzer="aus"), "r-kd"),
          ("kundendaten", "kunde", "kundendaten", "frei")),
         (anf(senkbar(geerbt="oeffentlich", art=FEHLT), "r-oeff"), ("oeffentlich", "geerbt", "niedrigste", "frei")),
         (anf(senkbar(art=FEHLT, gruppe="g-x"), "r-oeff"), ("intern", "geerbt", "art-unbekannt", "klasse-ueber-route")),
         (anf(senkbar(art="vertrag", gruppe="g-x"), "r-oeff"), ("intern", "geerbt", "gruppe-ohne-eintrag", "klasse-ueber-route")),
         (anf(senkbar(art="vertrag", befund=bef(geheimnis=1)), "r-oeff"),
          ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(befund=bef(personenbezug=1), modell="verhindert"), "r-oeff"),
          ("intern", "geerbt", "befund", "klasse-ueber-route")),
         (anf(senkbar(modell="verhindert", nutzer="aus"), "r-oeff"), ("intern", "geerbt", "modell-verhindert", "klasse-ueber-route")),
         (anf(senkbar(modell="verhindert", nutzer="bestaetigung"), "r-oeff"),
          ("intern", "geerbt", "modell-verhindert", "klasse-ueber-route")),
         (anf(senkbar(), FEHLT), ("oeffentlich", "gesenkt", "gesenkt", "route-fehlt")),
     ]),
    ("k3-schalter-aus-liste", ["K3", "H1", "A39"],
     "Nachtrag 20.11. (F1): Die Liste ist gesetzt, der Schalter fehlt; die Liste gilt erst bei Schalter an, nichts sinkt.",
     V, T_LISTE_OHNE_SCHALTER, [
         (anf(senkbar(), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
         (anf(senkbar(art="glossar"), "r-oeff"), ("intern", "geerbt", "aus", "klasse-ueber-route")),
     ]),
    ("k3-eigene-art", ["K3", "H0", "H7", "A39", "A41"],
     "Nachtrag 20.11. (F3): Der Admin ergänzt die eigene Art glossar; sie sinkt. Eine Gruppe streicht sie wieder; "
     "eine Gruppe, die eine nicht erlaubte Art streicht, fügt nichts hinzu.",
     V, T_EIGENE_ART, [
         (anf(senkbar(art="glossar"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="regel"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="bedienwissen"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="glossar", gruppe="g-a"), "r-oeff"),
          ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="regel", gruppe="g-a"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="faq", gruppe="g-b"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="glossar", gruppe="g-b"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="Glossar"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="glossar", befund=bef(interner_name=1)), "r-oeff"),
          ("intern", "geerbt", "befund", "klasse-ueber-route")),
     ]),
    ("k3-nie-abwandlung", ["K3", "T1", "D14", "A40"],
     "Nachtrag 20.11., 2: T1 vergleicht byte-genau; vertraege, kunden-daten und vertrag. sind eigene Arten und sinken, "
     "vertrag selbst bleibt.",
     V, T_NIE_ABWANDLUNG, [
         (anf(senkbar(art="vertraege"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="kunden-daten"), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="vertrag."), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
         (anf(senkbar(art="vertrag"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
         (anf(senkbar(art="regel"), "r-oeff"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route")),
     ]),
    # K4: splitting (A33)
    ("k4-aufteilen", ["K4", "A33"],
     "Vorgabe an: Teil 1 (allgemein, geerbt) kommt über r-oeff, Teil 2 (Kundenangabe) nur über r-kd; die Reihenfolge bleibt.",
     V, None, [
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="kundendaten", kunde="k-a")]), "r-oeff"),
          T(("oeffentlich", "geerbt", "aus", "frei"), ("kundendaten", "kunde", "aus", "klasse-ueber-route"))),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="kundendaten", kunde="k-a")]), "r-kd"),
          T(("oeffentlich", "geerbt", "aus", "frei"), ("kundendaten", "kunde", "aus", "frei"))),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="intern"), eh(klasse="oeffentlich")]), "r-oeff"),
          T(("oeffentlich", "geerbt", "aus", "frei"), ("intern", "eigen", "aus", "klasse-ueber-route"),
            ("oeffentlich", "eigen", "aus", "frei"))),
         (anf(eh(geerbt="oeffentlich", teile=[eh(klasse="geheim"), eh()]), "r-oeff"),
          T((None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse"), ("oeffentlich", "geerbt", "aus", "frei"))),
         (anf(eh(geerbt="intern", teile=[eh(klasse="kundendaten"), eh()]), "r-kd"),
          T(("kundendaten", "eigen", "aus", "kunde-fehlt"), ("intern", "geerbt", "aus", "frei"))),
         (anf(eh(teile=[eh(), eh(klasse="oeffentlich")]), "r-oeff"),
          T(("kundendaten", "vorgabestufe", "aus", "klasse-ueber-route"), ("oeffentlich", "eigen", "aus", "frei"))),
         (anf(eh(geerbt="oeffentlich", teile=[eh(kunde="k-a"), eh(kunde="k-b")]), "r-kd"),
          T(("kundendaten", "kunde", "aus", "frei"), ("kundendaten", "kunde", "aus", "frei"))),
         (anf(eh(geerbt="oeffentlich", teile=[eh()])), T(("oeffentlich", "geerbt", "aus", "route-fehlt"))),
     ]),
    ("k4-aufteilen-ausdruecklich-an", ["K4", "A33"],
     "Nachtrag 20.11. (F2): aufteilen ausdrücklich true wirkt wie die Vorgabe.",
     V, T_TEILEN_AN, [
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="kundendaten", kunde="k-a")]), "r-oeff"),
          T(("oeffentlich", "geerbt", "aus", "frei"), ("kundendaten", "kunde", "aus", "klasse-ueber-route"))),
     ]),
    ("k4-aufteilen-aus", ["K4", "A33"],
     "Aufteilen aus: die Einheit bleibt ganz in der höchsten Klasse ihrer Teile; zwei Kunden ergeben einen leeren Kunden.",
     V, T_TEILEN_AUS, [
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="kundendaten", kunde="k-a")]), "r-oeff"),
          ("kundendaten", "teile", "nicht-aufgeteilt", "klasse-ueber-route")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="kundendaten", kunde="k-a")]), "r-kd"),
          ("kundendaten", "teile", "nicht-aufgeteilt", "frei")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(kunde="k-a"), eh(kunde="k-b")]), "r-kd"),
          ("kundendaten", "teile", "nicht-aufgeteilt", "kunde-ohne-eintrag")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(kunde="k-a"), eh(kunde="k-a")]), "r-kd"),
          ("kundendaten", "teile", "nicht-aufgeteilt", "frei")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="intern")]), "r-intern"),
          ("intern", "teile", "nicht-aufgeteilt", "frei")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="oeffentlich")]), "r-oeff"),
          ("oeffentlich", "teile", "nicht-aufgeteilt", "frei")),
         (anf(eh(geerbt="oeffentlich", teile=[eh(), eh(klasse="geheim")]), "r-kd"),
          (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(geerbt="geheim", teile=[eh(klasse="intern")]), "r-kd"),
          (None, "ungueltig", "klasse-ungueltig", "einheit-ohne-klasse")),
         (anf(eh(geerbt="intern"), "r-intern"), ("intern", "geerbt", "aus", "frei")),
     ]),
    ("k4-teile-senkung", ["K4", "K3", "A33", "A39"],
     "Aufteilen an, Herabstufen an: je Teil eine Klasse; der Regelteil sinkt, der Vertragsteil nicht; die Lage gilt für alle Teile.",
     V, T_AN, [
         (anf(eh(geerbt="intern", teile=[eh(art="regel", befund=BEF0), eh(art="vertrag", befund=BEF0)]), "r-oeff"),
          T(("oeffentlich", "gesenkt", "gesenkt", "frei"), ("intern", "geerbt", "art-nicht-herabstufbar", "klasse-ueber-route"))),
         (anf(eh(geerbt="intern", senkung_nutzer="bestaetigung",
                 teile=[eh(art="regel", befund=BEF0), eh(art="bedienwissen", befund=BEF0)]), "r-oeff"),
          T(("intern", "geerbt", "bestaetigung-fehlt", "klasse-ueber-route"),
            ("intern", "geerbt", "bestaetigung-fehlt", "klasse-ueber-route"))),
         (anf(eh(geerbt="intern", gruppe="g-x", teile=[eh(art="regel", befund=BEF0)]), "r-oeff"),
          T(("intern", "geerbt", "gruppe-ohne-eintrag", "klasse-ueber-route"))),
         (anf(eh(geerbt="intern", teile=[eh(art="regel", befund=BEF0, kunde="k-a"), eh(art="regel", befund=BEF0)]), "r-oeff"),
          T(("kundendaten", "kunde", "kundendaten", "klasse-ueber-route"), ("oeffentlich", "gesenkt", "gesenkt", "frei"))),
     ]),
    ("k4-teile-senkung-aus", ["K4", "K3", "A33"],
     "Aufteilen aus, Herabstufen an: eine Einheit mit Teilen sinkt nie, auch wenn jeder Teil herabstufbar wäre.",
     V, T_AN_TEILEN_AUS, [
         (anf(eh(geerbt="intern", teile=[eh(art="regel", befund=BEF0), eh(art="regel", befund=BEF0)]), "r-oeff"),
          ("intern", "teile", "nicht-aufgeteilt", "klasse-ueber-route")),
         (anf(senkbar(), "r-oeff"), ("oeffentlich", "gesenkt", "gesenkt", "frei")),
     ]),
    ("k-tiefe-64", ["D13"], "Verschachtelung genau 64; das oberste Objekt ist Ebene 1.",
     V, None, [
         (anf(eh(geerbt="intern"), "r-intern"), ("intern", "geerbt", "aus", "frei")),
     ]),
]
TIEFE = {"k-tiefe-64": 64}


# ---------------------------------------------------------------- invalid cases (exit 2)

def fall(tiefe=FEHLT, anfragen=None, vorgabe=None, **extra):
    f = {"vorgabe": V if vorgabe is None else vorgabe,
         "anfragen": [anf(eh(geerbt="intern"), "r-intern")] if anfragen is None else anfragen}
    if tiefe is not FEHLT:
        f["tiefe"] = tiefe
    f.update(extra)
    return f


def mit_einheit(**felder):
    return fall(anfragen=[anf(eh(**felder), "r-intern")])


# (name, rules, description, case as object, or bytes for a raw input)
UNGUELTIG = [
    ("u-k-kein-json", ["D13"], "Die Eingabe ist kein JSON.", b'{"tiefe": '),
    ("u-k-feld-im-fall", ["D13"], "Ein unbekanntes Feld im Fall.", fall(zusatz=1)),
    ("u-k-ohne-anfragen", ["D13"], "Das Feld anfragen fehlt.", {"vorgabe": V}),
    ("u-k-vorgabe-ungueltig", ["D13"], "Die Vorgabe von Vertrag 3 ist ungültig (klassen leer).",
     fall(vorgabe=dict(V, klassen=[]))),
    ("u-k-tiefe-liste", ["D13"], "tiefe ist kein Objekt.", fall(tiefe=[])),
    ("u-k-tiefe-null", ["D13"], "tiefe ist null.", fall(tiefe=None)),
    ("u-k-tiefe-feld", ["D13"], "Ein unbekanntes Feld in tiefe.", fall(tiefe={"senken": True})),
    ("u-k-aufteilen-text", ["D13"], "aufteilen ist kein Wahrheitswert.", fall(tiefe={"aufteilen": "an"})),
    ("u-k-aufteilen-null", ["D13"], "aufteilen ist null.", fall(tiefe={"aufteilen": None})),
    ("u-k-an-null", ["D13"], "herabstufen.an ist null.", fall(tiefe={"herabstufen": {"an": None}})),
    ("u-k-an-zahl", ["D13"], "herabstufen.an ist 1 statt true.", fall(tiefe={"herabstufen": {"an": 1}})),
    ("u-k-herabstufen-feld", ["D13"], "Ein unbekanntes Feld in herabstufen.",
     fall(tiefe={"herabstufen": {"an": True, "stufen": 2}})),
    ("u-k-arten-fremd", ["D13", "A40"], "Die Firmenliste nennt eine Art außerhalb der vier herabstufbaren Arten.",
     fall(tiefe={"herabstufen": {"an": True, "arten": ["regel", "vertrag"]}})),
    ("u-k-arten-form", ["D13", "T1"], "Eine eigene Art der Firmenliste hat nicht die Schlüsselform.",
     fall(tiefe={"herabstufen": {"an": True, "arten": ["regel", "Glossar"]}})),
    ("u-k-arten-nie-schalter-aus", ["D13", "T1", "A40"],
     "Nachtrag 20.11. (F3): Eine Nie-Art in der Firmenliste ist ein Ladefehler, auch bei Schalter aus.",
     fall(tiefe={"herabstufen": {"an": False, "arten": ["regel", "geheimnis"]}})),
    ("u-k-streicht-nie", ["D13", "T1", "A40"], "Eine Gruppe nennt eine Nie-Art in streicht.",
     fall(tiefe={"herabstufen": {"gruppen": [{"schluessel": "g-a", "streicht": ["personenbezug"]}]}})),
    ("u-k-gruppe-arten", ["D13", "A41"], "Nachtrag 20.11. (F3): Eine Gruppe versucht, mit eigener Liste arten hinzuzufügen.",
     fall(tiefe={"herabstufen": {"an": True, "gruppen": [{"schluessel": "g-a", "arten": ["glossar"]}]}})),
    ("u-k-arten-doppelt", ["D13"], "Eine Art steht doppelt in der Firmenliste.",
     fall(tiefe={"herabstufen": {"arten": ["regel", "regel"]}})),
    ("u-k-arten-text", ["D13"], "arten ist keine Liste.", fall(tiefe={"herabstufen": {"arten": "regel"}})),
    ("u-k-gruppe-fuegt-hinzu", ["D13", "A41"], "Eine Gruppe versucht, eine Art hinzuzufügen.",
     fall(tiefe={"herabstufen": {"an": True, "gruppen": [{"schluessel": "g-a", "fuegt_hinzu": ["vertrag"]}]}})),
    ("u-k-streicht-fremd", ["D13", "A41"], "Eine Gruppe streicht eine Art außerhalb der vier herabstufbaren Arten.",
     fall(tiefe={"herabstufen": {"gruppen": [{"schluessel": "g-a", "streicht": ["vertrag"]}]}})),
] + [
    (f"u-k-arten-nie-{art}", ["D13", "T1", "A40"],
     f"Nachtrag 20.11. (F3): Die Nie-Art {art} in der Firmenliste ist ein Ladefehler.",
     fall(tiefe={"herabstufen": {"an": True, "arten": ["regel", art]}}))
    for art in ["personenbezug", "kundendaten", "geheimnis", "zugangsdaten", "vertrag", "preis", "finanzen",
                "netz-und-system", "gespraechsprotokoll", "sitzungsprotokoll", "entscheidung", "plan", "organisation",
                "quelltext", "entwurf"]  # written out by hand, not taken from NIE
] + [
    ("u-k-gruppe-form", ["D13"], "Ein Gruppenschlüssel hat nicht die Schlüsselform.",
     fall(tiefe={"herabstufen": {"gruppen": [{"schluessel": "G-A"}]}})),
    ("u-k-gruppe-doppelt", ["D13"], "Ein Gruppenschlüssel steht doppelt.",
     fall(tiefe={"herabstufen": {"gruppen": [{"schluessel": "g-a"}, {"schluessel": "g-a", "streicht": ["regel"]}]}})),
    ("u-k-gruppe-ohne-schluessel", ["D13"], "Eine Gruppe ohne schluessel.",
     fall(tiefe={"herabstufen": {"gruppen": [{"streicht": ["regel"]}]}})),
    ("u-k-anfrage-kunde", ["D13"], "Die Anfrage trägt kunde; der Kunde steht in der Einheit.",
     fall(anfragen=[dict(anf(eh(geerbt="intern"), "r-intern"), kunde="k-a")])),
    ("u-k-route-zahl", ["D13"], "route ist weder Text noch null.", fall(anfragen=[anf(eh(), 5)])),
    ("u-k-ohne-einheit", ["D13"], "Eine Anfrage ohne einheit.", fall(anfragen=[{"route": "r-intern"}])),
    ("u-k-einheit-feld", ["D13"], "Die Einheit trägt ein unbekanntes Feld.", mit_einheit(geerbt="intern", titel="x")),
    ("u-k-kunde-zahl", ["D13"], "kunde ist weder Text noch null.", mit_einheit(kunde=5)),
    ("u-k-art-zahl", ["D13"], "art ist weder Text noch null.", mit_einheit(art=1)),
    ("u-k-gruppe-zahl", ["D13"], "gruppe ist weder Text noch null.", mit_einheit(gruppe=1)),
    ("u-k-befund-teil", ["D13"], "befund ohne interner_name.",
     mit_einheit(befund={"geheimnis": False, "personenbezug": False})),
    ("u-k-befund-feld", ["D13"], "befund mit einem unbekannten Feld.", mit_einheit(befund=dict(BEF0, kunde=False))),
    ("u-k-befund-text", ["D13"], "Ein Wert in befund ist kein Wahrheitswert.", mit_einheit(befund=dict(BEF0, geheimnis="nein"))),
    ("u-k-befund-liste", ["D13"], "befund ist keine Liste und kein Objekt.", mit_einheit(befund=[])),
    ("u-k-modell-wert", ["D13"], "modell hat einen unbekannten Wert.", mit_einheit(modell="ja")),
    ("u-k-nutzer-wert", ["D13"], "senkung_nutzer hat einen unbekannten Wert.", mit_einheit(nutzer="an")),
    ("u-k-modell-liste", ["D13"], "Nachtrag 20.11., 2: modell ist eine Liste statt Text.", mit_einheit(modell=["frei"])),
    ("u-k-nutzer-liste", ["D13"], "Nachtrag 20.11., 2: senkung_nutzer ist eine Liste statt Text.",
     mit_einheit(nutzer=["folgt"])),
    ("u-k-nutzer-null", ["D13"], "senkung_nutzer ist null.", mit_einheit(nutzer=None)),
    ("u-k-bestaetigt-text", ["D13"], "bestaetigt ist kein Wahrheitswert.", mit_einheit(bestaetigt="ja")),
    ("u-k-bestaetigt-null", ["D13"], "bestaetigt ist null.", mit_einheit(bestaetigt=None)),
    ("u-k-teile-leer", ["D13", "A33"], "teile ist leer.", mit_einheit(geerbt="intern", teile=[])),
    ("u-k-teile-objekt", ["D13", "A33"], "teile ist keine Liste.", mit_einheit(geerbt="intern", teile={})),
    ("u-k-teile-null", ["D13", "A33"], "teile ist null.", mit_einheit(geerbt="intern", teile=None)),
    ("u-k-teil-text", ["D13", "A33"], "Ein Teil ist kein Objekt.", mit_einheit(teile=["intern"])),
    ("u-k-teile-und-inhalt", ["D13", "A33"], "Eine Einheit mit teile trägt selbst eine Klasse.",
     mit_einheit(klasse="intern", teile=[eh()])),
    ("u-k-teil-lage", ["D13", "A33"], "Ein Teil trägt geerbt; die Lage gilt für die ganze Einheit.",
     mit_einheit(teile=[eh(geerbt="intern")])),
    ("u-k-teil-teile", ["D13", "A33"], "Ein Teil trägt selbst teile.", mit_einheit(teile=[eh(teile=[eh()])])),
    ("u-k-teil-kunde-zahl", ["D13", "A33"], "kunde eines Teils ist weder Text noch null.", mit_einheit(teile=[eh(kunde=5)])),
    ("u-k-teile-65", ["D13", "A33"], "65 Teile, eins über der Grenze.", mit_einheit(teile=[eh()] * 65)),
    ("u-k-zu-tief", ["D13"], "Verschachtelung 65 im nicht ausgewerteten Feld beschreibung.",
     fall(beschreibung=V3.tief(63))),
    ("u-k-doppel-tiefe", ["D13", "A139"], "Ein doppelter Schlüssel in herabstufen; der zweite Wert wäre lockerer.",
     b'{"vorgabe": {"klassen": ["oeffentlich", "intern"], "routen": [], "kunden": []}, '
     b'"tiefe": {"herabstufen": {"an": false, "an": true}}, "anfragen": []}'),
    ("u-k-doppel-escape", ["D13", "A139"], "Doppel in Escape-Schreibweise: an und \\u0061n sind derselbe Schlüssel.",
     b'{"vorgabe": {"klassen": ["oeffentlich", "intern"], "routen": [], "kunden": []}, '
     b'"tiefe": {"herabstufen": {"an": false, "\\u0061n": true}}, "anfragen": []}'),
    ("u-k-doppel-teil", ["D13", "A139"], "Ein doppelter Schlüssel drei Ebenen tief in einem Teil.",
     b'{"vorgabe": {"klassen": ["oeffentlich", "kundendaten"], "routen": [{"schluessel": "r", "art": "lokal", '
     b'"klasse": "oeffentlich"}], "kunden": []}, "anfragen": [{"einheit": {"teile": [{"klasse": "kundendaten", '
     b'"klasse": "oeffentlich"}]}, "route": "r"}]}'),
    ("u-k-doppel-befund", ["D13", "A139"], "Ein doppelter Schlüssel in befund.",
     b'{"vorgabe": {"klassen": ["oeffentlich", "intern"], "routen": [], "kunden": []}, "anfragen": [{"einheit": '
     b'{"geerbt": "intern", "art": "regel", "befund": {"geheimnis": true, "personenbezug": false, '
     b'"interner_name": false, "geheimnis": false}}, "route": null}]}'),
]


# ---------------------------------------------------------------- reference (only to check the tables above)

def ist_schl(x):
    return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None


def tiefe_gueltig(t):
    if not isinstance(t, dict) or not set(t) <= {"aufteilen", "herabstufen"}:
        return False
    if "aufteilen" in t and not isinstance(t["aufteilen"], bool):
        return False
    h = t.get("herabstufen", {})
    if not isinstance(h, dict) or not set(h) <= {"an", "arten", "gruppen"}:
        return False
    if "an" in h and not isinstance(h["an"], bool):
        return False
    arten = h.get("arten", ARTEN)
    if not isinstance(arten, list) or not all(ist_schl(a) and a not in NIE for a in arten):
        return False
    if len(arten) != len(set(arten)):
        return False
    gruppen = h.get("gruppen", [])
    if not isinstance(gruppen, list):
        return False
    for g in gruppen:
        if not isinstance(g, dict) or not set(g) <= {"schluessel", "streicht"} or "schluessel" not in g:
            return False
        if not ist_schl(g["schluessel"]):
            return False
        st = g.get("streicht", [])
        if not isinstance(st, list) or not all(ist_schl(a) and a not in NIE for a in st):
            return False
    namen = [g["schluessel"] for g in gruppen]
    return len(namen) == len(set(namen))


def inhalt_gueltig(i):
    if not isinstance(i.get("kunde"), (str, type(None))) or not isinstance(i.get("art"), (str, type(None))):
        return False
    b = i.get("befund")
    if b is not None and (not isinstance(b, dict) or set(b) != BEFUND or not all(isinstance(x, bool) for x in b.values())):
        return False
    return i.get("modell") is None or (isinstance(i["modell"], str) and i["modell"] in MODELL)


def lage_gueltig(e):
    if not isinstance(e.get("gruppe"), (str, type(None))):
        return False
    if "senkung_nutzer" in e and (not isinstance(e["senkung_nutzer"], str) or e["senkung_nutzer"] not in NUTZER):
        return False
    return "bestaetigt" not in e or isinstance(e["bestaetigt"], bool)


def einheit_gueltig(e):
    if not isinstance(e, dict) or not lage_gueltig(e):
        return False
    if "teile" not in e:
        return set(e) <= LAGE | INHALT and inhalt_gueltig(e)
    teile = e["teile"]
    if not set(e) <= LAGE | {"teile"} or not isinstance(teile, list) or not 1 <= len(teile) <= TEILE_MAX:
        return False
    return all(isinstance(t, dict) and set(t) <= INHALT and inhalt_gueltig(t) for t in teile)


def fall_gueltig(f):
    if not isinstance(f, dict) or not set(f) <= FALL_FELDER or "vorgabe" not in f or not V3.gueltig(f["vorgabe"]):
        return False
    if "tiefe" in f and not tiefe_gueltig(f["tiefe"]):
        return False
    if not isinstance(f.get("anfragen"), list) or V3.tiefe(f) > 64:
        return False
    return all(isinstance(a, dict) and set(a) <= {"einheit", "route"} and "einheit" in a
               and isinstance(a.get("route"), (str, type(None))) and einheit_gueltig(a["einheit"])
               for a in f["anfragen"])


def optionen(t):
    t = t or {}
    h = t.get("herabstufen", {})
    gruppen = {g["schluessel"]: g.get("streicht", []) for g in h.get("gruppen", [])}
    return t.get("aufteilen", True), h.get("an", False), h.get("arten", ARTEN), gruppen


UNG = object()


def angabe(x, kl):
    if x is None:
        return None
    return x if isinstance(x, str) and x in kl else UNG


def grundklasse(v, lage, inh):
    """(index or None, origin) by K1."""
    kl = v["klassen"]
    eigen, geerbt = angabe(inh.get("klasse"), kl), angabe(lage.get("geerbt"), kl)
    if eigen is UNG or geerbt is UNG:
        return None, "ungueltig"
    if eigen is not None:
        return kl.index(eigen), "eigen"
    if geerbt is not None:
        return kl.index(geerbt), "geerbt"
    return kl.index(v.get("vorgabestufe", kl[-1])), "vorgabestufe"


def senkgrund(v, t, lage, inh, i, herkunft):
    """Reason by K3 (H1 to H11 in table order, H0 otherwise)."""
    _, an, arten, gruppen = optionen(t)
    if not an:
        return "aus"
    if herkunft != "geerbt":
        return "nicht-geerbt"
    if inh.get("kunde") is not None or i == len(v["klassen"]) - 1:
        return "kundendaten"
    if i == 0:
        return "niedrigste"
    art = inh.get("art")
    if art is None:
        return "art-unbekannt"
    g = lage.get("gruppe")
    if g is not None and g not in gruppen:
        return "gruppe-ohne-eintrag"
    if art not in [a for a in arten if g is None or a not in gruppen[g]]:
        return "art-nicht-herabstufbar"
    b = inh.get("befund")
    if b is None or any(b.values()):
        return "befund"
    if inh.get("modell") == "verhindert":
        return "modell-verhindert"
    n = lage.get("senkung_nutzer", "folgt")
    if n == "aus":
        return "nutzer-aus"
    if n == "bestaetigung" and lage.get("bestaetigt") is not True:
        return "bestaetigung-fehlt"
    return "gesenkt"


def darf_antwort(v, klasse, kunde_, route_):
    a = {"einheit": {"klasse": "" if klasse is None else klasse}, "kunde": kunde_}
    if route_ is not FEHLT:
        a["route"] = route_
    grund, k = V3.referenz(v, a)
    return V3.antwort(grund, k, a.get("route"))


def einzeln(v, t, lage, inh, route_):
    """Full answer for one unit or one part by K1 to K5."""
    kl = v["klassen"]
    i, herkunft = grundklasse(v, lage, inh)
    kunde_ = inh.get("kunde")
    if i is None:
        return {"klasse": None, "herkunft": "ungueltig", "senkung": {"grund": "klasse-ungueltig", "vorschlag": None},
                "protokoll": None, "darf": darf_antwort(v, None, kunde_, route_)}
    grund = senkgrund(v, t, lage, inh, i, herkunft)
    klasse, vorschlag, protokoll = kl[i], None, None
    if grund == "bestaetigung-fehlt":
        vorschlag = kl[i - 1]
    if grund == "gesenkt":
        klasse, herkunft = kl[i - 1], "gesenkt"
        protokoll = {"art": inh["art"], "erkannt_durch": ["typ:" + inh["art"], "filter:frei"]
                     + (["modell:frei"] if inh.get("modell") in ("frei", "senken") else []),
                     "regel_ebene": "firma" if lage.get("gruppe") is None else "gruppe",
                     "klasse_alt": kl[i], "klasse_neu": kl[i - 1]}
    if kunde_ is not None:
        klasse, herkunft = kl[-1], "kunde"
    return {"klasse": klasse, "herkunft": herkunft, "senkung": {"grund": grund, "vorschlag": vorschlag},
            "protokoll": protokoll, "darf": darf_antwort(v, klasse, kunde_, route_)}


def referenz(v, t, a):
    e = a["einheit"]
    route_ = a.get("route", FEHLT)
    lage = {k: e[k] for k in LAGE if k in e}
    if "teile" not in e:
        return einzeln(v, t, lage, e, route_)
    aufteilen = optionen(t)[0]
    if aufteilen:
        teile = [einzeln(v, t, lage, x, route_) for x in e["teile"]]
        aus = [n for n, x in enumerate(teile) if x["darf"]["darf"]]
        return {"teile": teile, "ausgeliefert": aus, "gesperrt": len(teile) - len(aus)}
    kl = v["klassen"]
    stufen = []
    for x in e["teile"]:
        i, _ = grundklasse(v, lage, x)
        stufen.append(None if i is None else (len(kl) - 1 if x.get("kunde") is not None else i))
    kunden = sorted({x["kunde"] for x in e["teile"] if x.get("kunde") is not None})
    kunde_ = None if not kunden else (kunden[0] if len(kunden) == 1 else "")
    if None in stufen:
        return {"klasse": None, "herkunft": "ungueltig", "senkung": {"grund": "klasse-ungueltig", "vorschlag": None},
                "protokoll": None, "darf": darf_antwort(v, None, kunde_, route_)}
    klasse = kl[max(stufen)]
    return {"klasse": klasse, "herkunft": "teile", "senkung": {"grund": "nicht-aufgeteilt", "vorschlag": None},
            "protokoll": None, "darf": darf_antwort(v, klasse, kunde_, route_)}


# ---------------------------------------------------------------- answers from the handwritten expectations

def aus_erwartung(v, a, inh, erw, lage):
    """Answer built from one handwritten tuple; only mechanical fields are derived."""
    klasse, herkunft, grund, darf_grund = erw
    kl = v["klassen"]
    vorschlag = protokoll = None
    if grund == "bestaetigung-fehlt":
        vorschlag = kl[kl.index(klasse) - 1]
    if grund == "gesenkt":
        modell = ["modell:frei"] if inh.get("modell") in ("frei", "senken") else []
        protokoll = {"art": inh["art"], "erkannt_durch": ["typ:" + inh["art"], "filter:frei"] + modell,
                     "regel_ebene": "firma" if lage.get("gruppe") is None else "gruppe",
                     "klasse_alt": kl[kl.index(klasse) + 1], "klasse_neu": klasse}
    route_ = a.get("route")
    darf_klasse = klasse
    return {"klasse": klasse, "herkunft": herkunft, "senkung": {"grund": grund, "vorschlag": vorschlag},
            "protokoll": protokoll,
            "darf": V3.antwort(darf_grund, darf_klasse, route_)}


def antwort_erwartet(v, a, erw):
    e = a["einheit"]
    lage = {k: e[k] for k in LAGE if k in e}
    if isinstance(erw, tuple) and erw and erw[0] == "teile":
        assert len(e["teile"]) == len(erw[1]), "eine Erwartung je Teil"
        teile = [aus_erwartung(v, a, x, t, lage) for x, t in zip(e["teile"], erw[1])]
        aus = [n for n, x in enumerate(teile) if x["darf"]["darf"]]
        return {"teile": teile, "ausgeliefert": aus, "gesperrt": len(teile) - len(aus)}
    return aus_erwartung(v, a, e, erw, lage)


def main():
    dateien = {}
    namen = set()
    zaehler = {"anfragen": 0}
    for name, regeln, beschreibung, v, t, paare in FAELLE:
        assert name not in namen, name
        namen.add(name)
        anfragen, antworten = [], []
        for a, erw in paare:
            ist = antwort_erwartet(v, a, erw)
            ref = referenz(v, t, a)
            assert ist == ref, f"{name}: {json.dumps(a, ensure_ascii=False)}\n erwartet {ist}\n Referenz {ref}"
            anfragen.append(a)
            antworten.append(ist)
        fall_ = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v, "anfragen": anfragen}
        if t is not None:
            fall_["tiefe"] = t
        if name in TIEFE:
            fall_["beschreibung"] = V3.tief(TIEFE[name] - 2)
            assert V3.tiefe(fall_) == TIEFE[name], (name, V3.tiefe(fall_))
        assert fall_gueltig(fall_), f"{name}: Fall ungueltig"
        zaehler["anfragen"] += len(anfragen)
        dateien[name] = (json.dumps(fall_, ensure_ascii=False, indent=1).encode() + b"\n",
                         {"exit": 0, "ausgabe": {"antworten": antworten}})
    for name, regeln, beschreibung, f in UNGUELTIG:
        assert name not in namen, name
        namen.add(name)
        if isinstance(f, bytes):
            roh = f
        else:
            assert not fall_gueltig(f), f"{name}: Fall waere gueltig"
            roh = json.dumps(f, ensure_ascii=False).encode() + b"\n"
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
    print(f"{len(dateien)} Faelle ({gueltige} gueltig mit {zaehler['anfragen']} Anfragen, "
          f"{len(dateien) - gueltige} ungueltig)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
