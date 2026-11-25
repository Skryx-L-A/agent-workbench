"""Builds the visible cases of the contract `schreibtor` (rust/spec/VERTRAEGE.md).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json (the input on stdin of the program) and sichtbar/erwartet/<name>.json (exit code and
expected output). Every expectation is written out by hand in the tables below. The script also checks each one against a
small reference of the rules in rust/spec/VERTRAEGE.md, sections 3 to 6, and each case against the form rules of section 7; a mismatch
stops the script before anything is written. All data is invented.

The reference of `darf` and of the effective class (D5) follows rust/spec/datentor/VERTRAG.md, section 4. If that contract
changes, adapt `darf` and `wirksam` here and rebuild.
"""
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "sichtbar"
SCHLUESSEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
HEX64 = re.compile(r"[0-9a-f]{64}")
STEUER = re.compile(r"[\x00-\x1f\x7f]")
PSEUDONYM = re.compile(r"ps_[0-9a-f]{32}")
FEHLT = object()

KL3 = ["oeffentlich", "intern", "kundendaten"]
NIE = {"vorgabe": "nie", "freigaben": []}
H_A = "a" * 64
H_B = "b" * 64
H_C = "c" * 64
P1 = "ps_" + "1" * 32
P2 = "ps_" + "2" * 32
GRUND = "Alte Preisliste wurde veroeffentlicht"
WS = "brain_schreiben"
WK = "brain_senken"


def route(schluessel, art, klasse):
    return {"schluessel": schluessel, "art": art, "klasse": klasse}


BASIS = {
    "klassen": KL3,
    "routen": [route("cloud", "cloud", "intern"), route("lokal", "lokal", "kundendaten"),
               route("cloud-oeff", "cloud", "oeffentlich")],
    "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"]}, {"schluessel": "k-sued", "erlaubt": ["lokal"]}],
    "kundendaten_an_cloud": NIE,
}


def mit(schreiben=None, **zusatz):
    v = json.loads(json.dumps(BASIS))
    v.update(zusatz)
    if schreiben is not None:
        v["schreiben"] = schreiben
    return v


V_START = mit()                                                        # no block: defaults
V_KEINE_VORBELEGUNG = mit({"vorbelegung_mensch": "keine"})
V_STUFE_INTERN = mit(vorgabestufe="intern")
V_REGELN = mit({"regeln": [
    {"schluessel": "r-kunden-o", "ordner": "kunden/o", "kennzeichen": "k-markt", "klasse": "kundendaten"},
    {"schluessel": "r-handbuch", "ordner": "handbuch", "klasse": "oeffentlich"},
    {"schluessel": "r-betrieb", "ordner": "betrieb", "klasse": "intern"},
]})
V_REGELN_NEU = mit({"regeln": [
    {"schluessel": "r-kunden-o", "ordner": "kunden/o", "klasse": "kundendaten"},
]})
V_ROUTE_OHNE_KLASSE = mit()
V_ROUTE_OHNE_KLASSE["routen"].append({"schluessel": "cloud-n", "art": "cloud"})


def modell(route_="cloud", sitzung="s-1", freigabe=None, person=None, entscheidend=None):
    return {"herkunft": "modell", "sitzung": sitzung, "route": route_, "freigabe": freigabe,
            "person": person, "entscheidend": entscheidend}


def mensch(person=None, freigabe=None, entscheidend=None):
    return {"herkunft": "mensch", "sitzung": None, "route": None, "freigabe": freigabe,
            "person": person, "entscheidend": entscheidend}


def fg(klasse, kunde_=None, werkzeug=WS, zweite=False):
    """kontext.freigabe as the caller takes it from the answer of the Stelle (E7)."""
    return {"klasse": klasse, "kunde": kunde_, "werkzeug": werkzeug, "zweite_person": zweite}


def neu():
    return {"vorhanden": False}


def da(sha=H_A, klasse="intern", kunde_=None):
    return {"vorhanden": True, "sha256": sha, "klasse": klasse, "kunde": kunde_}


def lage(ziel=None, ort="intern", kennzeichen=(), erkannt=(None, ()), erbe=(None, ())):
    """erbe: (class, customers), the string "unklar", or None (only for a human)."""
    l_ = {"ziel": ziel if ziel is not None else neu(), "ort": ort, "kennzeichen": list(kennzeichen),
          "erkannt": {"klasse": erkannt[0], "kunden": list(erkannt[1])}}
    if erbe is None or erbe == "unklar":
        l_["erbe"] = erbe
    else:
        l_["erbe"] = {"klasse": erbe[0], "kunden": list(erbe[1])}
    return l_


def anl(ziel="betrieb/notiz.md", inhalt="Text", **felder):
    a = {"ziel": ziel, "art": "anlegen", "inhalt": inhalt}
    a.update(felder)
    return a


def aen(ziel="betrieb/notiz.md", inhalt="Text neu", vorher=H_A, **felder):
    a = {"ziel": ziel, "art": "aendern", "inhalt": inhalt, "vorher_sha256": vorher}
    a.update(felder)
    return a


def umk(klasse, kunde_=None, ziel="betrieb/notiz.md", vorher=H_A, grund=FEHLT):
    a = {"ziel": ziel, "art": "umklassieren", "vorher_sha256": vorher, "klasse": klasse, "kunde": kunde_}
    if grund is not FEHLT:
        a["grund"] = grund
    return a


def a_(kontext, anfrage, l_):
    if isinstance(anfrage, dict):
        anfrage = {k: v for k, v in anfrage.items() if v is not FEHLT}
    return {"kontext": kontext, "anfrage": anfrage, "lage": l_}


def ok(ergebnis, klasse, kunde_=None, klasse_alt=None, werkzeug=None):
    return {"ergebnis": ergebnis, "grund": None, "klasse": klasse, "kunde": kunde_, "klasse_alt": klasse_alt,
            "werkzeug": werkzeug}


def fn(klasse, kunde_=None, klasse_alt=None, werkzeug=WS):
    """freigabe-noetig with the tool the gate names (E7)."""
    return ok("freigabe-noetig", klasse, kunde_, klasse_alt, werkzeug)


def nein(grund):
    return {"ergebnis": "abgelehnt", "grund": grund, "klasse": None, "kunde": None, "klasse_alt": None,
            "werkzeug": None}


S, F = "schreiben", "freigabe-noetig"

# (name, rules, description, vorgabe, [(request, expected answer)])
FAELLE = [
    ("s1-mensch-auto-erbt-ort", ["S14", "A32", "A37"],
     "Ein Mensch legt mit Auto an; die Notiz erbt die Klasse des Orts, ohne Ort die Vorgabestufe.",
     V_START, [
         (a_(mensch(), anl(), lage(ort="intern")), ok(S, "intern")),
         (a_(mensch(), anl(klasse="auto"), lage(ort="oeffentlich")), ok(S, "oeffentlich")),
         (a_(mensch(), anl(), lage(ort=None)), ok(S, "kundendaten")),
     ]),
    ("s1-mensch-vorgabestufe", ["S14", "A35"],
     "Ohne Ort gilt die Vorgabestufe der Admins; hier intern statt der höchsten Klasse.",
     V_STUFE_INTERN, [
         (a_(mensch(), anl(), lage(ort=None)), ok(S, "intern")),
     ]),
    ("s2-mensch-waehlt", ["S14", "A37"],
     "Ein Mensch wählt die Klasse selbst; sie gilt auch unter dem Ort und mit erkanntem Namen. Ein gewählter Kunde wird übernommen.",
     V_START, [
         (a_(mensch(), anl(klasse="oeffentlich"), lage(ort="intern")), ok(S, "oeffentlich")),
         (a_(mensch(), anl(klasse="intern", kunde="k-nord"), lage(ort="intern")), ok(S, "intern", "k-nord")),
         (a_(mensch(), anl(klasse="oeffentlich"), lage(ort="intern", erkannt=("kundendaten", ["k-nord"]))),
          ok(S, "oeffentlich")),
     ]),
    ("s2-mensch-senkt-bestand", ["S14", "S17", "A37", "E7"],
     "Eine ausdrückliche Wahl eines Menschen darf den Bestand senken, mit Grund; das Tor fragt dafür die Stelle mit brain_senken. Die Antwort nennt die alte wirksame Klasse.",
     V_START, [
         (a_(mensch(), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(), umk("intern", None, grund=GRUND), lage(ziel=da(klasse="intern", kunde_="k-nord"))),
          fn("intern", None, "kundendaten", WK)),
     ]),
    ("s2-mensch-auto-senkt-nie", ["S11", "S14", "A37", "A39"],
     "Mit Auto senkt ein Mensch den Bestand nie; ein Kunde des Bestands bleibt.",
     V_START, [
         (a_(mensch(), aen(), lage(ziel=da(klasse="kundendaten"), ort="oeffentlich")),
          ok(S, "kundendaten", None, "kundendaten")),
         (a_(mensch(), aen(klasse="auto"), lage(ziel=da(klasse="oeffentlich", kunde_="k-nord"), ort="oeffentlich")),
          ok(S, "kundendaten", "k-nord", "kundendaten")),
     ]),
    ("s3-keine-vorbelegung", ["S6", "A37"],
     "Stellt der Admin die Vorbelegung auf keine, wird ein Eintrag ohne Wahl abgelehnt; Auto und eine Klasse gehen.",
     V_KEINE_VORBELEGUNG, [
         (a_(mensch(), anl(), lage()), nein("klasse-wahl-fehlt")),
         (a_(mensch(), anl(klasse="auto"), lage()), ok(S, "intern")),
         (a_(mensch(), anl(klasse="oeffentlich"), lage()), ok(S, "oeffentlich")),
     ]),
    ("s4-modell-braucht-freigabe", ["S15", "A90", "E7"],
     "Jedes Schreiben eines Modells geht über die Stelle (brain_schreiben); ob ein Mensch freigeben muss, entscheidet die Stelle nach ihrer Pflicht. Die Antwort nennt das Ergebnis vorab.",
     V_START, [
         (a_(modell(), anl(), lage(erbe=("intern", []))), fn("intern")),
         (a_(modell(), aen(), lage(ziel=da(klasse="oeffentlich"), erbe=(None, []))),
          fn("intern", None, "oeffentlich")),
     ]),
    ("s4-modell-eingeloest", ["S15", "A90", "A105", "E7"],
     "Nach dem Einlösen der Freigabe schreibt das Tor genau das freigegebene Ergebnis; weichen Ergebnis oder Werkzeug ab, lehnt es ab.",
     V_START, [
         (a_(modell(freigabe=fg("intern"), entscheidend=P2), anl(), lage(erbe=("intern", []))), ok(S, "intern")),
         (a_(modell(freigabe=fg("intern"), entscheidend=P2), anl(), lage(ort="kundendaten", erbe=("intern", []))),
          nein("ergebnis-abweichend")),
         (a_(modell(freigabe=fg("kundendaten"), entscheidend=P2), anl(),
             lage(erbe=("kundendaten", ["k-nord"]))), nein("ergebnis-abweichend")),
         (a_(modell(freigabe=fg("intern", werkzeug=WK), entscheidend=P2), anl(), lage(erbe=("intern", []))),
          nein("werkzeug-abweichend")),
     ]),
    ("s4-option-aus", ["S15", "S17", "A90", "E7"],
     "Ist die Pflicht mcp.brain.freigabe der Stelle aus, fragt das Tor trotzdem die Stelle; sie antwortet ohne_freigabe, und der zweite Aufruf ohne Entscheidenden schreibt. Ein senkendes Umklassieren nennt brain_senken; die Stelle verlangt dafür immer einen Menschen (A38).",
     V_START, [
         (a_(modell(), anl(), lage(erbe=("intern", []))), fn("intern")),
         (a_(modell(freigabe=fg("intern")), anl(), lage(erbe=("intern", []))), ok(S, "intern")),
         (a_(modell(), umk("oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"))),
          fn("oeffentlich", None, "intern", WK)),
         (a_(modell(freigabe=fg("oeffentlich", werkzeug=WK), entscheidend=P2), umk("oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), ok(S, "oeffentlich", None, "intern")),
     ]),
    ("s5-modell-setzt-keine-klasse", ["S5", "A36"],
     "Ein Modell nennt beim Anlegen und Ändern keine Klasse und keinen Kunden; das System ordnet zu.",
     V_START, [
         (a_(modell(), anl(klasse="oeffentlich"), lage(erbe=(None, []))), nein("klasse-nur-mensch")),
         (a_(modell(), anl(klasse="auto"), lage(erbe=(None, []))), nein("klasse-nur-mensch")),
         (a_(modell(), aen(klasse="intern", kunde=None), lage(ziel=da(), erbe=(None, []))), nein("klasse-nur-mensch")),
     ]),
    ("s6-modell-umklassieren", ["S15", "S17", "A38"],
     "Ein Modell darf eine Umklassierung nur anfragen; sie wirkt erst nach der Freigabe, auch wenn sie senkt.",
     V_START, [
         (a_(modell(route_="lokal"), umk("intern", grund=GRUND), lage(ziel=da(klasse="kundendaten", kunde_="k-nord"),
                                                                    erbe=("kundendaten", ["k-nord"]))),
          fn("intern", None, "kundendaten", WK)),
         (a_(modell(route_="lokal", freigabe=fg("intern", werkzeug=WK), entscheidend=P2), umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          ok(S, "intern", None, "kundendaten")),
     ]),
    ("s7-regeln-heben", ["S14", "A36"],
     "Admin-Regeln heben nach Ordner und Kennzeichen; eine Regel mit niedrigerer Klasse senkt nie unter den Ort.",
     V_REGELN, [
         (a_(modell(route_="lokal"), anl(ziel="kunden/o/n1.md"), lage(ort="intern", kennzeichen=["k-markt"])),
          fn("kundendaten")),
         (a_(modell(route_="lokal"), anl(ziel="kunden/o/n2.md"), lage(ort="intern")), fn("intern")),
         (a_(modell(route_="lokal"), anl(ziel="kunden/ox/n3.md"), lage(ort="intern", kennzeichen=["k-markt"])),
          fn("intern")),
         (a_(modell(route_="lokal"), anl(ziel="handbuch/n4.md"), lage(ort="intern")), fn("intern")),
         (a_(modell(route_="lokal"), anl(ziel="handbuch/n5.md"), lage(ort=None)), fn("kundendaten")),
         (a_(mensch(), anl(ziel="kunden/o/a/n6.md"), lage(ort="oeffentlich", kennzeichen=["k-markt"])),
          ok(S, "kundendaten")),
     ]),
    ("s7-regel-geaendert", ["S14", "A36"],
     "Nach der Änderung der Regel durch den Admin gilt die neue Regel, ohne Rückfrage an einen Menschen.",
     V_REGELN_NEU, [
         (a_(modell(route_="lokal"), anl(ziel="kunden/o/n2.md"), lage(ort="intern")), fn("kundendaten")),
     ]),
    ("s8-erkennung-hebt", ["S14", "A37"],
     "Die Erkennung (etwa ein interner Name) hebt bei Auto und beim Modell; ein erkannter Kunde macht Kundendaten.",
     V_START, [
         (a_(mensch(), anl(), lage(ort="oeffentlich", erkannt=("intern", []))), ok(S, "intern")),
         (a_(mensch(), anl(), lage(ort="oeffentlich", erkannt=(None, ["k-nord"]))), ok(S, "kundendaten", "k-nord")),
         (a_(modell(), anl(), lage(ort="oeffentlich", erkannt=("intern", []), erbe=(None, []))), fn("intern")),
     ]),
    ("s9-erbe-hebt", ["S14", "KE"],
     "Das Erbe der Quellen hebt ein Ergebnis des Modells; ein Kunde der Quellen wird der Kunde des Ergebnisses.",
     V_START, [
         (a_(modell(route_="lokal"), anl(), lage(ort="oeffentlich", erbe=("intern", []))), fn("intern")),
         (a_(modell(route_="lokal"), anl(), lage(ort="oeffentlich", erbe=("kundendaten", ["k-nord"]))),
          fn("kundendaten", "k-nord")),
         (a_(modell(route_="lokal"), anl(), lage(ort="oeffentlich", erbe=(None, []))), fn("oeffentlich")),
     ]),
    ("s9-mensch-ohne-erbe", ["S14"],
     "Ein Mensch schreibt ohne Sitzung; ein Erbe gilt für ihn nicht und darf fehlen.",
     V_START, [
         (a_(mensch(), anl(), lage(ort="oeffentlich", erbe=None)), ok(S, "oeffentlich")),
         (a_(mensch(), anl(), lage(ort="oeffentlich", erbe=("kundendaten", ["k-nord"]))), ok(S, "oeffentlich")),
     ]),
    ("s10-kunden-gemischt", ["S13", "KE"],
     "Zwei verschiedene Kunden aus Erbe, Bestand oder Erkennung lehnen das Schreiben ab; derselbe Kunde zweimal nicht.",
     V_START, [
         (a_(modell(route_="lokal"), anl(), lage(erbe=("kundendaten", ["k-nord", "k-sued"]))), nein("kunden-gemischt")),
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(klasse="kundendaten", kunde_="k-sued"),
                                                erbe=("kundendaten", ["k-nord"]))), nein("kunden-gemischt")),
         (a_(modell(route_="lokal"), anl(), lage(erkannt=(None, ["k-sued"]), erbe=("kundendaten", ["k-nord"]))),
          nein("kunden-gemischt")),
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(klasse="kundendaten", kunde_="k-nord"),
                                                erkannt=(None, ["k-nord"]), erbe=("kundendaten", ["k-nord"]))),
          fn("kundendaten", "k-nord", "kundendaten")),
         (a_(mensch(), anl(), lage(erkannt=(None, ["k-nord", "k-sued"]))), nein("kunden-gemischt")),
     ]),
    ("s11-modell-senkt-nie", ["S14", "A39", "A40"],
     "Ein Modell, das eine Notiz ändert, senkt ihre Klasse nie, auch nicht mit niedrigem Ort und leerem Erbe; ein Kunde bleibt.",
     V_START, [
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(klasse="intern"), ort="oeffentlich", erbe=(None, []))),
          fn("intern", None, "intern")),
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(klasse="oeffentlich", kunde_="k-nord"), ort="oeffentlich",
                                                erbe=(None, []))), fn("kundendaten", "k-nord", "kundendaten")),
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), ort="oeffentlich",
                                                erbe=(None, []))), fn("kundendaten", "k-nord", "kundendaten")),
     ]),
    ("s12-ziel-gesperrt", ["S8", "A34"],
     "Ein Modell ändert nur, was seine Route lesen darf; über einen gesperrten Bestand erfährt es nur ziel-gesperrt.",
     V_START, [
         (a_(modell(), aen(), lage(ziel=da(klasse="kundendaten"), erbe=(None, []))), nein("ziel-gesperrt")),
         (a_(modell(), aen(vorher=H_B), lage(ziel=da(klasse="kundendaten"), erbe=(None, []))), nein("ziel-gesperrt")),
         (a_(modell(), anl(), lage(ziel=da(klasse="kundendaten"), erbe=(None, []))), nein("ziel-gesperrt")),
         (a_(modell(), umk("oeffentlich"), lage(ziel=da(klasse="intern", kunde_="k-nord"), erbe=(None, []))),
          nein("ziel-gesperrt")),
         (a_(modell(), aen(), lage(ziel=da(klasse="geheim"), erbe=(None, []))), nein("ziel-gesperrt")),
         (a_(modell(), aen(), lage(ziel=da(klasse=None), erbe=(None, []))), nein("ziel-gesperrt")),
     ]),
    ("s12-schreiben-ueber-die-route", ["S14", "A32"],
     "Ein Modell darf in einen höheren Ort schreiben, den seine Route nicht liest; das Ergebnis trägt die höhere Klasse.",
     V_START, [
         (a_(modell(), anl(ziel="kunden/n.md"), lage(ort="kundendaten", erbe=("intern", []))), fn("kundendaten")),
     ]),
    ("s13-bestand", ["S9", "S10", "S11"],
     "Anlegen über einem Bestand, Ändern ohne Bestand und ein veralteter Stand werden abgelehnt.",
     V_START, [
         (a_(mensch(), anl(), lage(ziel=da())), nein("ziel-vorhanden")),
         (a_(mensch(), aen(), lage(ziel=neu())), nein("ziel-nicht-vorhanden")),
         (a_(mensch(), umk("intern"), lage(ziel=neu())), nein("ziel-nicht-vorhanden")),
         (a_(mensch(), aen(vorher=H_B), lage(ziel=da(sha=H_A))), nein("stand-abweichend")),
         (a_(modell(route_="lokal"), aen(vorher=H_C), lage(ziel=da(sha=H_A), erbe=(None, []))),
          nein("stand-abweichend")),
         (a_(mensch(), aen(), lage(ziel=da(klasse="geheim"))), nein("bestand-ohne-klasse")),
         (a_(mensch(), aen(), lage(ziel=da(klasse=7))), nein("bestand-ohne-klasse")),
         (a_(mensch(), aen(klasse="intern"), lage(ziel=da(klasse="geheim"))), ok(S, "intern")),
     ]),
    ("s14-ziel", ["S2", "S3", "A15"],
     "Ohne benanntes Ziel bricht jedes Schreiben ab; ein Ziel muss ein relativer Pfad einer Notiz sein.",
     V_START, [
         (a_(mensch(), anl(ziel=FEHLT), lage()), nein("ziel-fehlt")),
         (a_(mensch(), anl(ziel=None), lage()), nein("ziel-fehlt")),
         (a_(mensch(), anl(ziel=""), lage()), nein("ziel-fehlt")),
         (a_(mensch(), anl(ziel="/betrieb/n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="betrieb/../n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="betrieb//n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="betrieb/n.txt"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel=".versteckt/n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="betrieb/n\n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="betrieb\\n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="a/" * 255 + "n.md"), lage()), nein("ziel-ungueltig")),
         (a_(mensch(), anl(ziel="a/" * 254 + "n.md"), lage()), ok(S, "intern")),
         (a_(mensch(), anl(ziel="betrieb/ä n.md"), lage()), ok(S, "intern")),
     ]),
    ("s15-anfrage-form", ["S1"],
     "Felder, die das Tor oder der Aufrufer setzt, und jede falsche Form machen die Anfrage ungültig; ausdrücklich route, herkunft, sitzung, freigabe und werkzeug.",
     V_START, [
         (a_(modell(), anl(route="lokal"), lage(erbe=(None, []))), nein("anfrage-ungueltig")),
         (a_(modell(), anl(herkunft="mensch"), lage(erbe=(None, []))), nein("anfrage-ungueltig")),
         (a_(modell(), anl(sitzung="s-2"), lage(erbe=(None, []))), nein("anfrage-ungueltig")),
         (a_(modell(), anl(freigabe={"klasse": "intern", "kunde": None}), lage(erbe=(None, []))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), {"ziel": "betrieb/n.md", "art": "loeschen"}, lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), {"ziel": "betrieb/n.md", "inhalt": "x"}, lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(inhalt=FEHLT), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(inhalt=5), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(vorher_sha256=H_A), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), aen(vorher=FEHLT), lage(ziel=da())), nein("anfrage-ungueltig")),
         (a_(mensch(), aen(vorher="A" * 64), lage(ziel=da())), nein("anfrage-ungueltig")),
         (a_(mensch(), dict(umk("intern"), inhalt="x"), lage(ziel=da())), nein("anfrage-ungueltig")),
         (a_(mensch(), umk("auto"), lage(ziel=da())), nein("anfrage-ungueltig")),
         (a_(mensch(), {k: v for k, v in umk("intern").items() if k != "kunde"}, lage(ziel=da())),
          nein("anfrage-ungueltig")),
         (a_(mensch(), anl(kunde="k-nord"), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(klasse="auto", kunde="k-nord"), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(klasse=1), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(ziel=5), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), ["betrieb/n.md"], lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(inhalt="x" * 1048577), lage()), nein("anfrage-ungueltig")),
         (a_(modell(), anl(werkzeug=WK), lage(erbe=(None, []))), nein("anfrage-ungueltig")),
     ]),
    ("s16-wahl-unbekannt", ["S7"],
     "Eine gewählte Klasse oder ein gewählter Kunde, die die Vorgabe nicht kennt, lehnen ab; Vergleich byte-genau.",
     V_START, [
         (a_(mensch(), anl(klasse="geheim"), lage()), nein("klasse-unbekannt")),
         (a_(mensch(), anl(klasse="INTERN"), lage()), nein("klasse-unbekannt")),
         (a_(mensch(), anl(klasse="intern", kunde="k-west"), lage()), nein("kunde-unbekannt")),
         (a_(modell(route_="lokal"), umk("intern", "K-NORD"), lage(ziel=da(), erbe=(None, []))),
          nein("kunde-unbekannt")),
     ]),
    ("s17-route-der-sitzung", ["S4", "A42", "A43"],
     "Ein Modell ohne Route oder auf einer Route ohne Klasse schreibt nicht; die Route kommt aus dem Kontext, nie aus der Anfrage.",
     V_ROUTE_OHNE_KLASSE, [
         (a_(modell(route_=None), anl(), lage(erbe=(None, []))), nein("route-fehlt")),
         (a_(modell(route_=""), anl(), lage(erbe=(None, []))), nein("route-fehlt")),
         (a_(modell(route_="cloud-x"), anl(), lage(erbe=(None, []))), nein("route-ohne-klasse")),
         (a_(modell(route_="cloud-n"), anl(), lage(erbe=(None, []))), nein("route-ohne-klasse")),
         (a_(mensch(), anl(), lage()), ok(S, "intern")),
     ]),
    ("s18-erbe-unklar", ["S12", "KE"],
     "Meldet das Klassenerbe unklar (etwa eine Quelle mit ungültiger Klasse), schreibt das Modell nicht.",
     V_START, [
         (a_(modell(route_="lokal"), anl(), lage(erbe="unklar")), nein("erbe-unklar")),
     ]),
    ("s19-reihenfolge", ["S0"],
     "Die erste zutreffende Regel bestimmt den Grund.",
     V_START, [
         (a_(modell(route_=None), anl(ziel=None, klasse="intern"), lage(erbe=(None, []))), nein("ziel-fehlt")),
         (a_(modell(route_=None), anl(klasse="intern"), lage(erbe=(None, []))), nein("route-fehlt")),
         (a_(modell(), anl(klasse="intern"), lage(ziel=da(klasse="kundendaten"), erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(), aen(vorher=H_B), lage(ziel=da(klasse="kundendaten"), erbe="unklar")), nein("ziel-gesperrt")),
         (a_(modell(route_="lokal"), anl(), lage(ziel=da(), erbe="unklar")), nein("ziel-vorhanden")),
         (a_(modell(route_="lokal"), aen(), lage(ziel=da(), erbe="unklar")), nein("erbe-unklar")),
         (a_(mensch(), anl(klasse="geheim"), lage(ziel=da())), nein("klasse-unbekannt")),
     ]),
    ("s21-senken-mit-grund", ["S17", "S18", "A37", "E6", "E7"],
     "Pflicht der Stelle aus: Ein Mensch senkt mit eigener Wahl nur mit Grund; das Tor fragt die Stelle mit brain_senken, sie antwortet ohne_freigabe, und der zweite Aufruf mit dem Menschen selbst als Entscheidendem schreibt. Ohne Grund wird abgelehnt. Heben, Gleichstand, Auto und eine Reparatur ohne gültige Klasse brauchen keinen Grund und keine Stelle.",
     V_START, [
         (a_(mensch(), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"))),
          fn("oeffentlich", None, "intern", WK)),
         (a_(mensch(), aen(klasse="oeffentlich"), lage(ziel=da(klasse="intern"))), nein("grund-fehlt")),
         (a_(mensch(), umk("intern"), lage(ziel=da(klasse="kundendaten"))), nein("grund-fehlt")),
         (a_(mensch(), umk("intern", grund=GRUND), lage(ziel=da(klasse="kundendaten"))),
          fn("intern", None, "kundendaten", WK)),
         (a_(mensch(), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          ok(S, "kundendaten", None, "intern")),
         (a_(mensch(), aen(klasse="kundendaten", grund=GRUND), lage(ziel=da(klasse="intern"))),
          ok(S, "kundendaten", None, "intern")),
         (a_(mensch(), aen(klasse="intern"), lage(ziel=da(klasse="intern"))), ok(S, "intern", None, "intern")),
         (a_(mensch(), aen(), lage(ziel=da(klasse="kundendaten"), ort="oeffentlich")),
          ok(S, "kundendaten", None, "kundendaten")),
         (a_(mensch(), aen(klasse="intern"), lage(ziel=da(klasse="geheim"))), ok(S, "intern")),
         (a_(mensch(), anl(klasse="oeffentlich"), lage(ort="intern")), ok(S, "oeffentlich")),
         (a_(mensch(), aen(klasse="intern"), lage(ziel=da(klasse="oeffentlich", kunde_="k-nord"))),
          nein("grund-fehlt")),
         (a_(mensch(), aen(klasse="intern"), lage(ziel=da(klasse=None))), nein("grund-fehlt")),
         (a_(mensch(), aen(klasse="oeffentlich", vorher=H_B), lage(ziel=da(klasse="kundendaten"))),
          nein("stand-abweichend")),
         (a_(mensch(), aen(klasse="geheim"), lage(ziel=da(klasse="kundendaten"))), nein("klasse-unbekannt")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK), P1), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), ok(S, "oeffentlich", None, "intern")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), nein("person-abweichend")),
     ]),
    ("s22-grund-form", ["S1", "S17", "E6"],
     "Der Grund ist Text von 8 bis 500 Zeichen ohne Steuerzeichen, ohne Leerzeichen am Anfang oder Ende; er gehört nur zu einer gewählten Klasse oder zu umklassieren. Die Form wird auch geprüft, wenn nichts gesenkt wird.",
     V_START, [
         (a_(mensch(), aen(klasse="oeffentlich", grund="g" * 7), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund="g" * 8), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(), aen(klasse="oeffentlich", grund="g" * 500), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(), aen(klasse="oeffentlich", grund="g" * 501), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund="ä" * 8), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(), aen(klasse="oeffentlich", grund="ä" * 500), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(), aen(klasse="oeffentlich", grund="ä" * 501), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund=" " + "g" * 8), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund="g" * 8 + " "), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund="gggg\ngggg"), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund="gggg\tgggg"), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund=5), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund=None), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="oeffentlich", grund=["x" * 8]), lage(ziel=da(klasse="kundendaten"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), aen(klasse="kundendaten", grund="kurz"), lage(ziel=da(klasse="intern"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(), umk("intern", grund="kurz"), lage(ziel=da(klasse="kundendaten"))), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(grund=GRUND), lage()), nein("anfrage-ungueltig")),
         (a_(mensch(), anl(klasse="auto", grund=GRUND), lage()), nein("anfrage-ungueltig")),
         (a_(modell(), aen(grund=GRUND), lage(ziel=da(), erbe=(None, []))), nein("anfrage-ungueltig")),
         (a_(modell(), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(), erbe=(None, []))),
          nein("klasse-nur-mensch")),
     ]),
    ("s24-senken-zweite-person", ["S17", "S18", "S15", "E6", "E7"],
     "Pflicht der Stelle an: Ein senkender Mensch fragt mit brain_senken, eine andere Person erteilt. Im zweiten Aufruf mit zweite_person lehnt das Tor dieselbe Person, eine fehlende Person und ein falsches Werkzeug ab; mit einer anderen Person schreibt es. Heben und Auto bleiben ohne Stelle.",
     V_START, [
         (a_(mensch(P1), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="kundendaten"))),
          fn("oeffentlich", None, "kundendaten", WK)),
         (a_(mensch(P1), umk("intern", grund=GRUND), lage(ziel=da(klasse="kundendaten"))),
          fn("intern", None, "kundendaten", WK)),
         (a_(mensch(P1), aen(klasse="oeffentlich"), lage(ziel=da(klasse="kundendaten"))), nein("grund-fehlt")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK, zweite=True), P2), aen(klasse="oeffentlich"),
             lage(ziel=da(klasse="kundendaten"))), nein("grund-fehlt")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK, zweite=True), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="kundendaten"))), ok(S, "oeffentlich", None, "kundendaten")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK, zweite=True), P1), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="kundendaten"))), nein("selbe-person")),
         (a_(mensch(None, fg("oeffentlich", werkzeug=WK, zweite=True), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="kundendaten"))), nein("zweite-person-fehlt")),
         (a_(mensch(P1, fg("intern", werkzeug=WK, zweite=True), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="kundendaten"))), nein("ergebnis-abweichend")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK, zweite=True), P1), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), nein("selbe-person")),
         (a_(mensch(P1), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          ok(S, "kundendaten", None, "intern")),
         (a_(mensch(P1), aen(klasse="intern"), lage(ziel=da(klasse="intern"))), ok(S, "intern", None, "intern")),
         (a_(mensch(P1), aen(), lage(ziel=da(klasse="kundendaten"), ort="oeffentlich")),
          ok(S, "kundendaten", None, "kundendaten")),
         (a_(mensch(P1), anl(klasse="oeffentlich"), lage(ort="intern")), ok(S, "oeffentlich")),
         (a_(mensch(P1, fg("kundendaten"), P1), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          ok(S, "kundendaten", None, "intern")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK, zweite=True), P2), aen(klasse="kundendaten"),
             lage(ziel=da(klasse="intern"))), nein("ergebnis-abweichend")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WS, zweite=True), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="kundendaten"))), nein("werkzeug-abweichend")),
     ]),
    ("s26-modell-traegt-senken", ["S17", "S18", "A38", "E6", "E7"],
     "Ein Modell kann auf Anweisung eines Menschen umklassieren (A38); es schreibt erst nach dem zweiten Aufruf. Senken braucht einen Grund und nennt brain_senken; meldet die Stelle zweite_person, muss eine zweite Person entscheiden.",
     V_START, [
         (a_(modell(route_="lokal", person=P1), umk("intern"), lage(ziel=da(klasse="kundendaten", kunde_="k-nord"),
                                                                    erbe=("kundendaten", ["k-nord"]))),
          nein("grund-fehlt")),
         (a_(modell(route_="lokal", person=P1), umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          fn("intern", None, "kundendaten", WK)),
         (a_(modell(route_="lokal", freigabe=fg("intern", werkzeug=WK, zweite=True), person=P1, entscheidend=P1),
             umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          nein("selbe-person")),
         (a_(modell(route_="lokal", freigabe=fg("intern", werkzeug=WK, zweite=True), person=P1, entscheidend=P2),
             umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          ok(S, "intern", None, "kundendaten")),
         (a_(modell(route_="lokal", freigabe=fg("kundendaten", "k-nord"), person=P1, entscheidend=P1),
             umk("kundendaten", "k-nord"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          ok(S, "kundendaten", "k-nord", "intern")),
         (a_(modell(route_="lokal", freigabe=fg("intern", werkzeug=WK, zweite=True), person=P1),
             umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          nein("zweite-person-fehlt")),
         (a_(modell(route_="lokal", freigabe=fg("intern"), person=P1, entscheidend=P2),
             umk("intern", grund=GRUND),
             lage(ziel=da(klasse="kundendaten", kunde_="k-nord"), erbe=("kundendaten", ["k-nord"]))),
          nein("werkzeug-abweichend")),
     ]),
    ("s25-modell-senkt-nie-option-aus", ["S5", "S17", "E6"],
     "Ohne Freigabe: Ein Modell wählt nie selbst eine Klasse beim Anlegen oder Ändern, und eine Umklassierung ohne Grund lehnt das Tor ab.",
     V_START, [
         (a_(modell(route_="lokal"), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"), erbe=(None, []))), nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal"), aen(klasse="oeffentlich"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal"), anl(klasse="intern", grund=GRUND), lage(erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal"), umk("oeffentlich"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          nein("grund-fehlt")),
     ]),
    ("s25-modell-senkt-nie-option-an", ["S5", "S17", "E6", "E7"],
     "Mit einer Freigabe der Stelle samt zweiter Person: Ein Modell wählt trotzdem nie selbst eine Klasse beim Anlegen oder Ändern, und eine Umklassierung ohne Grund lehnt das Tor ab.",
     V_START, [
         (a_(modell(route_="lokal", freigabe=fg("oeffentlich", werkzeug=WK, zweite=True), person=P1, entscheidend=P2),
             aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal", freigabe=fg("oeffentlich", werkzeug=WK, zweite=True), person=P1, entscheidend=P2),
             aen(klasse="oeffentlich"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal", freigabe=fg("intern", zweite=True), person=P1, entscheidend=P2),
             anl(klasse="intern", grund=GRUND), lage(erbe=(None, []))),
          nein("klasse-nur-mensch")),
         (a_(modell(route_="lokal", freigabe=fg("oeffentlich", werkzeug=WK, zweite=True), person=P1, entscheidend=P2),
             umk("oeffentlich"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          nein("grund-fehlt")),
     ]),
    ("s27-werkzeug", ["S1", "S14", "E7"],
     "Das Tor nennt das Werkzeug selbst: brain_schreiben für jedes Schreiben eines Modells, brain_senken für jedes Senken; ein hebender Mensch schreibt ohne Stelle und ohne Werkzeug. Eine Anfrage mit dem Feld werkzeug ist ungültig.",
     V_START, [
         (a_(modell(), anl(), lage(erbe=(None, []))), fn("intern")),
         (a_(modell(route_="lokal"), umk("oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          fn("oeffentlich", None, "intern", WK)),
         (a_(modell(route_="lokal"), umk("kundendaten", "k-nord"), lage(ziel=da(klasse="intern"), erbe=(None, []))),
          fn("kundendaten", "k-nord", "intern")),
         (a_(mensch(P1), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"))),
          fn("oeffentlich", None, "intern", WK)),
         (a_(mensch(P1), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          ok(S, "kundendaten", None, "intern")),
         (a_(modell(), anl(werkzeug=WS), lage(erbe=(None, []))), nein("anfrage-ungueltig")),
         (a_(mensch(P1), aen(klasse="oeffentlich", grund=GRUND, werkzeug=WS), lage(ziel=da(klasse="intern"))),
          nein("anfrage-ungueltig")),
         (a_(mensch(P1), aen(klasse="kundendaten", werkzeug=None), lage(ziel=da(klasse="intern"))),
          nein("anfrage-ungueltig")),
     ]),
    ("s28-zweiter-aufruf", ["S15", "S18", "E7"],
     "Der zweite Aufruf übernimmt Werkzeug und zweite_person aus der Antwort der Stelle. Ein falsches Werkzeug lehnt das Tor ab; ohne zweite Person muss im Menschenweg der Mensch selbst entscheiden, im Modellweg darf der Entscheidende fehlen (ohne_freigabe).",
     V_START, [
         (a_(modell(freigabe=fg("intern", werkzeug=WK)), anl(), lage(erbe=(None, []))), nein("werkzeug-abweichend")),
         (a_(mensch(P1, fg("oeffentlich"), P1), aen(klasse="oeffentlich", grund=GRUND), lage(ziel=da(klasse="intern"))),
          nein("werkzeug-abweichend")),
         (a_(mensch(P1, fg("kundendaten", werkzeug=WK), P1), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          nein("werkzeug-abweichend")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), nein("person-abweichend")),
         (a_(mensch(None, fg("oeffentlich", werkzeug=WK), P2), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), nein("person-abweichend")),
         (a_(mensch(P1, fg("kundendaten"), P2), aen(klasse="kundendaten"), lage(ziel=da(klasse="intern"))),
          nein("person-abweichend")),
         (a_(modell(freigabe=fg("intern")), anl(), lage(erbe=(None, []))), ok(S, "intern")),
         (a_(modell(freigabe=fg("intern"), person=P1, entscheidend=P1), anl(), lage(erbe=(None, []))), ok(S, "intern")),
         (a_(modell(freigabe=fg("intern", zweite=True), person=P1, entscheidend=P2), anl(), lage(erbe=(None, []))),
          ok(S, "intern")),
         (a_(modell(freigabe=fg("intern", zweite=True), person=P1), anl(), lage(erbe=(None, []))),
          nein("zweite-person-fehlt")),
         (a_(modell(freigabe=fg("intern", zweite=True), entscheidend=P2), anl(), lage(erbe=(None, []))),
          nein("zweite-person-fehlt")),
         (a_(modell(freigabe=fg("intern", zweite=True), person=P2, entscheidend=P2), anl(), lage(erbe=(None, []))),
          nein("selbe-person")),
         (a_(mensch(P1, fg("oeffentlich", werkzeug=WK), P1), aen(klasse="oeffentlich", grund=GRUND),
             lage(ziel=da(klasse="intern"))), ok(S, "oeffentlich", None, "intern")),
     ]),
    ("s20-keine-anfrage", ["S0"], "Eine leere Liste von Anfragen ergibt eine leere Liste von Antworten.", V_START, []),
]


# ---------------------------------------------------------------- reference


def wirksam(v, roh_klasse, roh_kunde):
    """Effective class by D5 of contract 3, or None for an invalid class."""
    kl = v["klassen"]
    if roh_klasse is None:
        eigene = v.get("vorgabestufe", kl[-1])
    elif isinstance(roh_klasse, str) and roh_klasse in kl:
        eigene = roh_klasse
    else:
        return None
    return kl[-1] if roh_kunde is not None else eigene


def kunde_text(roh):
    """Customer as contract 4 passes it: None stays None, a non-text becomes the empty text."""
    if roh is None:
        return None
    return roh if isinstance(roh, str) else ""


def darf(v, roh_klasse, route_, kunde_):
    """True if `darf` of contract 3 answers frei."""
    kl = v["klassen"]
    top = kl[-1]
    routen = {r["schluessel"]: r for r in v["routen"]}
    kunden = {k["schluessel"]: k for k in v["kunden"]}
    frei = {(f["route"], f["kunde"]) for f in v.get("kundendaten_an_cloud", {}).get("freigaben", [])}
    w = wirksam(v, roh_klasse, kunde_)
    r = routen.get(route_)
    if not route_ or r is None or r.get("klasse") is None or w is None:
        return False
    if kl.index(w) > kl.index(r["klasse"]):
        return False
    if kunde_ is not None or w == top:
        k = kunden.get(kunde_)
        if k is None or route_ in k.get("verboten", []) or route_ not in k.get("erlaubt", []):
            return False
        if r["art"] == "cloud" and (route_, kunde_) not in frei:
            return False
    return True


def ziel_gueltig(z):
    if len(z.encode("utf-8")) > 512 or STEUER.search(z) or "\\" in z or not z.endswith(".md"):
        return False
    teile = z.split("/")
    return all(t and t not in (".", "..") and not t.startswith(".") for t in teile) and teile[-1] != ".md"


def unter(ziel_, ordner):
    return ziel_.startswith(ordner + "/")


def anfrage_gueltig(a):
    if not isinstance(a, dict) or not set(a) <= {"ziel", "art", "inhalt", "vorher_sha256", "klasse", "kunde", "grund"}:
        return False
    art = a.get("art")
    if art not in ("anlegen", "aendern", "umklassieren"):
        return False
    if "ziel" in a and not isinstance(a["ziel"], (str, type(None))):
        return False
    if art == "umklassieren":
        if "inhalt" in a:
            return False
    elif not isinstance(a.get("inhalt"), str) or len(a["inhalt"].encode("utf-8")) > 1048576:
        return False
    vs = a.get("vorher_sha256", FEHLT)
    if art == "anlegen":
        if vs is not FEHLT and vs is not None:
            return False
    elif not (isinstance(vs, str) and HEX64.fullmatch(vs)):
        return False
    if "klasse" in a and not isinstance(a["klasse"], str):
        return False
    if "kunde" in a and not isinstance(a["kunde"], (str, type(None))):
        return False
    if art == "umklassieren":
        if "klasse" not in a or "kunde" not in a or a["klasse"] == "auto":
            return False
    elif "kunde" in a and a.get("klasse", "auto") == "auto":
        return False
    if "grund" in a:
        g = a["grund"]
        if not isinstance(g, str) or not 8 <= len(g) <= 500 or STEUER.search(g) or g[0] == " " or g[-1] == " ":
            return False
        if art != "umklassieren" and a.get("klasse", "auto") == "auto":
            return False
    return True


def schreib(v, kontext, a, l_):
    """Answer by rust/spec/VERTRAEGE.md, sections 3 to 6."""
    kl = v["klassen"]
    top = kl[-1]
    sch = v.get("schreiben", {})
    modell_ = kontext["herkunft"] == "modell"
    if not anfrage_gueltig(a):
        return nein("anfrage-ungueltig")
    art = a["art"]
    z = a.get("ziel")
    if not z:
        return nein("ziel-fehlt")
    if not ziel_gueltig(z):
        return nein("ziel-ungueltig")
    if modell_:
        r_ = kontext["route"]
        if not r_:
            return nein("route-fehlt")
        r = next((x for x in v["routen"] if x["schluessel"] == r_), None)
        if r is None or r.get("klasse") is None:
            return nein("route-ohne-klasse")
        if art != "umklassieren" and ("klasse" in a or "kunde" in a):
            return nein("klasse-nur-mensch")
    wahl = a.get("klasse", "auto") if not modell_ or art == "umklassieren" else "auto"
    if not modell_ and "klasse" not in a and sch.get("vorbelegung_mensch", "auto") == "keine":
        return nein("klasse-wahl-fehlt")
    if wahl != "auto" and wahl not in kl:
        return nein("klasse-unbekannt")
    if a.get("kunde") is not None and a["kunde"] not in {k["schluessel"] for k in v["kunden"]}:
        return nein("kunde-unbekannt")
    zl = l_["ziel"]
    if modell_ and zl["vorhanden"] and not darf(v, zl["klasse"], kontext["route"], kunde_text(zl["kunde"])):
        return nein("ziel-gesperrt")
    if art == "anlegen" and zl["vorhanden"]:
        return nein("ziel-vorhanden")
    if art != "anlegen" and not zl["vorhanden"]:
        return nein("ziel-nicht-vorhanden")
    if art != "anlegen" and a["vorher_sha256"] != zl["sha256"]:
        return nein("stand-abweichend")
    alt = wirksam(v, zl["klasse"], kunde_text(zl["kunde"])) if zl["vorhanden"] else None
    if wahl == "auto" and zl["vorhanden"] and alt is None:
        return nein("bestand-ohne-klasse")
    senken = wahl != "auto" and alt is not None and kl.index(wahl) < kl.index(alt)
    if senken and "grund" not in a:
        return nein("grund-fehlt")
    if wahl == "auto" and modell_ and l_["erbe"] == "unklar":
        return nein("erbe-unklar")
    if wahl == "auto":
        ort = l_["ort"] if l_["ort"] is not None else v.get("vorgabestufe", top)
        stufen = [ort]
        for regel in sch.get("regeln", []):
            if unter(z, regel["ordner"]) and ("kennzeichen" not in regel or regel["kennzeichen"] in l_["kennzeichen"]):
                stufen.append(regel["klasse"])
        if l_["erkannt"]["klasse"] is not None:
            stufen.append(l_["erkannt"]["klasse"])
        kunden = set(l_["erkannt"]["kunden"])
        if alt is not None:
            stufen.append(alt)
            if kunde_text(zl["kunde"]) is not None:
                kunden.add(kunde_text(zl["kunde"]))
        if modell_:
            if l_["erbe"]["klasse"] is not None:
                stufen.append(l_["erbe"]["klasse"])
            kunden |= set(l_["erbe"]["kunden"])
        if len(kunden) > 1:
            return nein("kunden-gemischt")
        kunde_ = next(iter(kunden), None)
        klasse = top if kunde_ is not None else max(stufen, key=kl.index)
    else:
        klasse, kunde_ = wahl, a.get("kunde")
    werkzeug = "brain_senken" if senken else "brain_schreiben"
    f = kontext["freigabe"]
    if f is not None:
        if (f["klasse"], f["kunde"]) != (klasse, kunde_):
            return nein("ergebnis-abweichend")
        if f["werkzeug"] != werkzeug:
            return nein("werkzeug-abweichend")
        if f["zweite_person"]:
            if kontext["person"] is None or kontext["entscheidend"] is None:
                return nein("zweite-person-fehlt")
            if kontext["person"] == kontext["entscheidend"]:
                return nein("selbe-person")
        elif not modell_ and kontext["entscheidend"] != kontext["person"]:
            return nein("person-abweichend")
    elif modell_ or senken:
        return fn(klasse, kunde_, alt, werkzeug)
    return ok("schreiben", klasse, kunde_, alt)


# ---------------------------------------------------------------- form of a case (section 7)


def ist_schl(x):
    return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None


def ordner_gueltig(o):
    return (isinstance(o, str) and o and not o.startswith("/") and not o.endswith("/") and "//" not in o
            and ".." not in o.split("/") and not STEUER.search(o))


def vorgabe_gueltig(v):
    """Contract 3, section 2, plus the block `schreiben` (section 7)."""
    if not isinstance(v, dict) or not set(v) <= {"klassen", "vorgabestufe", "routen", "kunden",
                                                 "kundendaten_an_cloud", "schreiben"}:
        return False
    if not {"klassen", "routen", "kunden"} <= set(v):
        return False
    kl = v["klassen"]
    if not isinstance(kl, list) or not kl or not all(ist_schl(k) for k in kl) or len(set(kl)) != len(kl):
        return False
    if "vorgabestufe" in v and v["vorgabestufe"] not in kl:
        return False
    if not isinstance(v["routen"], list) or not isinstance(v["kunden"], list):
        return False
    for r in v["routen"]:
        if not isinstance(r, dict) or not set(r) <= {"schluessel", "art", "klasse"} or not {"schluessel", "art"} <= set(r):
            return False
        if not ist_schl(r["schluessel"]) or r["art"] not in ("cloud", "lokal"):
            return False
        if r.get("klasse") is not None and r["klasse"] not in kl:
            return False
    rn = [r["schluessel"] for r in v["routen"]]
    if len(set(rn)) != len(rn):
        return False
    for k in v["kunden"]:
        if not isinstance(k, dict) or not set(k) <= {"schluessel", "erlaubt", "verboten"} or not ist_schl(k.get("schluessel")):
            return False
        for feld in ("erlaubt", "verboten"):
            if not isinstance(k.get(feld, []), list) or not all(x in rn for x in k.get(feld, [])):
                return False
    kn = [k["schluessel"] for k in v["kunden"]]
    if len(set(kn)) != len(kn):
        return False
    kc = v.get("kundendaten_an_cloud", {"vorgabe": "nie"})
    if not isinstance(kc, dict) or not set(kc) <= {"vorgabe", "freigaben"} or kc.get("vorgabe") != "nie":
        return False
    art = {r["schluessel"]: r["art"] for r in v["routen"]}
    for f in kc.get("freigaben", []):
        if not isinstance(f, dict) or set(f) != {"route", "kunde"} or art.get(f["route"]) != "cloud" or f["kunde"] not in kn:
            return False
    sch = v.get("schreiben", {})
    if not isinstance(sch, dict) or not set(sch) <= {"vorbelegung_mensch", "regeln"}:
        return False
    if sch.get("vorbelegung_mensch", "auto") not in ("auto", "keine"):
        return False
    regeln = sch.get("regeln", [])
    if not isinstance(regeln, list) or len(regeln) > 1024:
        return False
    for g in regeln:
        if not isinstance(g, dict) or not set(g) <= {"schluessel", "ordner", "kennzeichen", "klasse"}:
            return False
        if not {"schluessel", "ordner", "klasse"} <= set(g) or not ist_schl(g["schluessel"]):
            return False
        if not ordner_gueltig(g["ordner"]) or g["klasse"] not in kl:
            return False
        if "kennzeichen" in g and not ist_schl(g["kennzeichen"]):
            return False
    gn = [g["schluessel"] for g in regeln]
    return len(set(gn)) == len(gn)


def klassenpaar_gueltig(x, kl, unklar_erlaubt):
    if unklar_erlaubt and x == "unklar":
        return True
    if not isinstance(x, dict) or set(x) != {"klasse", "kunden"}:
        return False
    if x["klasse"] is not None and x["klasse"] not in kl:
        return False
    return isinstance(x["kunden"], list) and all(ist_schl(k) for k in x["kunden"])


def eintrag_gueltig(v, e):
    """Form of one entry of `anfragen`: kontext and lage come from the transport (section 7)."""
    kl = v["klassen"]
    if not isinstance(e, dict) or set(e) != {"kontext", "anfrage", "lage"}:
        return False
    k, l_ = e["kontext"], e["lage"]
    if not isinstance(k, dict) or set(k) != {"herkunft", "sitzung", "route", "freigabe", "person", "entscheidend"}:
        return False
    for feld in ("person", "entscheidend"):
        if k[feld] is not None and not (isinstance(k[feld], str) and PSEUDONYM.fullmatch(k[feld])):
            return False
    if k["entscheidend"] is not None and k["freigabe"] is None:
        return False
    fgb = k["freigabe"]
    if fgb is not None and not (isinstance(fgb, dict) and set(fgb) == {"klasse", "kunde", "werkzeug", "zweite_person"}
                                and fgb["klasse"] in kl and (fgb["kunde"] is None or ist_schl(fgb["kunde"]))
                                and fgb["werkzeug"] in ("brain_schreiben", "brain_senken")
                                and isinstance(fgb["zweite_person"], bool)):
        return False
    if fgb is not None and k["herkunft"] == "mensch" and k["entscheidend"] is None:
        return False
    if k["herkunft"] not in ("modell", "mensch"):
        return False
    if not isinstance(k["route"], (str, type(None))):
        return False
    if k["herkunft"] == "modell":
        if not ist_schl(k["sitzung"]):
            return False
    elif k["sitzung"] is not None or k["route"] is not None:
        return False
    if not isinstance(l_, dict) or set(l_) != {"ziel", "ort", "kennzeichen", "erkannt", "erbe"}:
        return False
    z = l_["ziel"]
    if z == {"vorhanden": False}:
        pass
    elif not (isinstance(z, dict) and set(z) == {"vorhanden", "sha256", "klasse", "kunde"} and z["vorhanden"] is True
              and isinstance(z["sha256"], str) and HEX64.fullmatch(z["sha256"])):
        return False
    if l_["ort"] is not None and l_["ort"] not in kl:
        return False
    if not isinstance(l_["kennzeichen"], list) or not all(ist_schl(x) for x in l_["kennzeichen"]):
        return False
    if not klassenpaar_gueltig(l_["erkannt"], kl, False):
        return False
    if l_["erbe"] is None:
        return k["herkunft"] == "mensch"
    return klassenpaar_gueltig(l_["erbe"], kl, True)


def fall_gueltig(fall):
    if not isinstance(fall, dict) or not set(fall) <= {"name", "regeln", "beschreibung", "vorgabe", "anfragen"}:
        return False
    if not vorgabe_gueltig(fall.get("vorgabe")) or not isinstance(fall.get("anfragen"), list):
        return False
    return all(eintrag_gueltig(fall["vorgabe"], e) for e in fall["anfragen"])


def ersetze(eintrag, pfad, wert):
    e = json.loads(json.dumps(eintrag))
    ziel_ = e
    for teil in pfad[:-1]:
        ziel_ = ziel_[teil]
    if wert is FEHLT:
        del ziel_[pfad[-1]]
    else:
        ziel_[pfad[-1]] = wert
    return e


E_MODELL = a_(modell(), anl(), lage(erbe=("intern", [])))
E_MENSCH = a_(mensch(), anl(), lage())
E_MENSCH_FG = a_(mensch(P1, fg("intern"), P1), anl(), lage())


def u(name, beschreibung, eintrag=None, vorgabe=None, zusatz=None):
    fall = {"vorgabe": vorgabe if vorgabe is not None else V_START, "anfragen": [eintrag if eintrag is not None else E_MENSCH]}
    if zusatz:
        fall.update(zusatz)
    return (name, ["S16"], beschreibung, fall)


V_REGEL_FALSCH = mit({"regeln": [{"schluessel": "r", "ordner": "/betrieb", "klasse": "intern"}]})
V_BLOCK_FELD = mit({"vorbelegung_mensch": "auto", "senken": True})

# (name, rules, description, case as object, or bytes for a raw input)
UNGUELTIG = [
    ("u-kein-json", ["S16"], "Die Eingabe ist kein JSON.", b'{"vorgabe": '),
    ("u-liste", ["S16"], "Die Eingabe ist kein Objekt.", b"[]"),
    ("u-doppelter-schluessel", ["S16"], "Ein Schlüssel steht doppelt im selben Objekt, hier in beschreibung.",
     b'{"beschreibung": {"a": 1, "a": 2}, "vorgabe": ' + json.dumps(V_START).encode() + b', "anfragen": []}'),
    u("u-feld-im-fall", "Ein unbekanntes Feld im Fall.", zusatz={"zusatz": 1}),
    u("u-block-feld", "Der Block schreiben hat ein unbekanntes Feld.", vorgabe=V_BLOCK_FELD),
    u("u-regel-ordner", "Der Ordner einer Regel ist absolut.", vorgabe=V_REGEL_FALSCH),
    u("u-herkunft-unbekannt", "Die Herkunft ist weder modell noch mensch.",
      ersetze(E_MODELL, ["kontext", "herkunft"], "agent")),
    u("u-modell-ohne-sitzung", "Ein Modell ohne Sitzung.", ersetze(E_MODELL, ["kontext", "sitzung"], None)),
    u("u-modell-ohne-erbe", "Für ein Modell fehlt das Erbe.", ersetze(E_MODELL, ["lage", "erbe"], None)),
    u("u-entscheidend-ohne-freigabe", "Ein Entscheidender ohne Freigabe.",
      ersetze(E_MENSCH, ["kontext", "entscheidend"], P2)),
    u("u-person-form", "Die Person ist kein Pseudonym.", ersetze(E_MENSCH, ["kontext", "person"], "Erika")),
    u("u-freigabe-form", "Die Freigabe trägt ein fünftes Feld.",
      E_MENSCH_FG | {"kontext": E_MENSCH_FG["kontext"] | {"freigabe": fg("intern") | {"entscheidend": P2}}}),
    u("u-freigabe-alte-form", "Die Freigabe hat nur klasse und kunde (Form vor E7).",
      E_MENSCH_FG | {"kontext": E_MENSCH_FG["kontext"] | {"freigabe": {"klasse": "intern", "kunde": None}}}),
    u("u-freigabe-werkzeug", "Das Werkzeug der Freigabe ist keines des Tors.",
      E_MENSCH_FG | {"kontext": E_MENSCH_FG["kontext"] | {"freigabe": fg("intern", werkzeug="brain_suche")}}),
    u("u-freigabe-zweite-person-form", "zweite_person der Freigabe ist kein Wahrheitswert.",
      E_MENSCH_FG | {"kontext": E_MENSCH_FG["kontext"] | {"freigabe": fg("intern", zweite=1)}}),
    u("u-mensch-freigabe-ohne-entscheidend", "Ein Mensch mit Freigabe ohne Entscheidenden.",
      E_MENSCH_FG | {"kontext": E_MENSCH_FG["kontext"] | {"entscheidend": None}}),
    u("u-option-form", "Die Option vorbelegung_mensch hat einen unbekannten Wert.",
      vorgabe=mit({"vorbelegung_mensch": "ja"})),
    u("u-vorgabe-freigabe-modell", "Die Vorgabe trägt die alte Option freigabe_modell; die Pflicht führt nur die Stelle.",
      vorgabe=mit({"freigabe_modell": False})),
    u("u-vorgabe-senken-zweite-person", "Die Vorgabe trägt die alte Option senken_zweite_person; die Pflicht führt nur die Stelle.",
      vorgabe=mit({"senken_zweite_person": True})),
    u("u-kontext-ohne-person", "Dem Kontext fehlt das Feld person.",
      ersetze(E_MENSCH, ["kontext"], {"herkunft": "mensch", "sitzung": None, "route": None, "freigabe": None,
                                       "entscheidend": None})),
    u("u-ort-unbekannt", "Die Ortsklasse steht nicht in klassen.", ersetze(E_MENSCH, ["lage", "ort"], "geheim")),
    u("u-ziel-ohne-sha", "Ein vorhandener Bestand ohne sha256.",
      ersetze(E_MENSCH, ["lage", "ziel"], {"vorhanden": True, "sha256": None, "klasse": None, "kunde": None})),
    u("u-kontext-feld", "Der Kontext trägt ein unbekanntes Feld.", ersetze(E_MENSCH, ["kontext", "zusatz"], 1)),
]


def antwort_fuer_vergleich(x):
    return x


def main():
    dateien = {}
    namen = set()
    for name, regeln, beschreibung, v, paare in FAELLE:
        assert name not in namen, name
        namen.add(name)
        fall = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v,
                "anfragen": [e for e, _ in paare]}
        assert fall_gueltig(fall), f"{name}: Fall ungueltig"
        for i, (e, erw) in enumerate(paare):
            ref = schreib(v, e["kontext"], e["anfrage"], e["lage"])
            assert ref == erw, f"{name}[{i}]: erwartet {erw}, Referenz {ref}"
        dateien[name] = (json.dumps(fall, ensure_ascii=False, indent=1).encode() + b"\n",
                         {"exit": 0, "ausgabe": {"antworten": [erw for _, erw in paare]}})
    for name, regeln, beschreibung, fall in UNGUELTIG:
        assert name not in namen, name
        namen.add(name)
        if isinstance(fall, bytes):
            roh = fall
        else:
            assert not fall_gueltig(fall), f"{name}: Fall waere gueltig"
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
