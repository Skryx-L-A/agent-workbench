"""Builds the visible cases of the contract `klassenerbe` (rust/spec/VERTRAEGE.md, draft for M3).

  python3 -I -B faelle_bauen.py

Writes sichtbar/faelle/<name>.json and sichtbar/erwartet/<name>.json. Every expectation is written out by hand in the
tables below and checked against a small reference of rust/spec/VERTRAEGE.md, sections 3 and 4; a mismatch stops the script before
anything is written. The effective class follows D5 of rust/spec/VERTRAEGE.md. All data is invented.
"""
import json
import re
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "sichtbar"
SCHLUESSEL = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}")
FEHLT = object()

KL3 = ["oeffentlich", "intern", "kundendaten"]
BASIS = {
    "klassen": KL3,
    "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "intern"},
               {"schluessel": "lokal", "art": "lokal", "klasse": "kundendaten"},
               {"schluessel": "cloud-oeff", "art": "cloud", "klasse": "oeffentlich"},
               {"schluessel": "cloud-n", "art": "cloud"}],
    "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"]}, {"schluessel": "k-sued", "erlaubt": ["lokal"]}],
}


def mit(boden=FEHLT, **zusatz):
    v = json.loads(json.dumps(BASIS))
    v.update(zusatz)
    if boden is not FEHLT:
        v["erbe"] = {"boden": boden}
    return v


V_ROUTE = mit()
V_GELESEN = mit("gelesen")
V_GELESEN_INTERN = mit("gelesen", vorgabestufe="intern")


def q(klasse=FEHLT, kunde_=FEHLT):
    e = {}
    if klasse is not FEHLT:
        e["klasse"] = klasse
    if kunde_ is not FEHLT:
        e["kunde"] = kunde_
    return e


def anf(route_, gelesen=(), sitzungskunde=None):
    return {"route": route_, "sitzungskunde": sitzungskunde, "gelesen": list(gelesen)}


def ok(klasse, kunden=()):
    return {"ergebnis": "ok", "klasse": klasse, "kunden": list(kunden), "grund": None}


def unklar(grund):
    return {"ergebnis": "unklar", "klasse": None, "kunden": [], "grund": grund}


FAELLE = [
    ("k1-boden-route", ["K3", "K5"],
     "Vorgabe: Der Boden ist die Klasse der Route. Ohne Quelle erbt das Ergebnis sie; eine niedrigere Quelle senkt nicht.",
     V_ROUTE, [
         (anf("cloud"), ok("intern")),
         (anf("cloud", [q("oeffentlich")]), ok("intern")),
         (anf("cloud-oeff", [q("oeffentlich")]), ok("oeffentlich")),
         (anf("lokal"), ok("kundendaten")),
     ]),
    ("k2-boden-gelesen", ["K3", "K5"],
     "Option „nur Gelesenes“: Das Ergebnis erbt das Maximum der gelesenen Einheiten; ohne Quelle gibt es keinen Boden.",
     V_GELESEN, [
         (anf("cloud"), ok(None)),
         (anf("cloud", [q("oeffentlich")]), ok("oeffentlich")),
         (anf("cloud", [q("oeffentlich"), q("intern"), q("oeffentlich")]), ok("intern")),
         (anf("lokal", [q("oeffentlich")]), ok("oeffentlich")),
     ]),
    ("k3-quelle-ohne-klasse", ["K4", "A35"],
     "Eine Quelle ohne Klasse bekommt die Vorgabestufe, hier zuerst die höchste, dann intern.",
     V_GELESEN, [
         (anf("lokal", [q(None)]), ok("kundendaten")),
         (anf("lokal", [q()]), ok("kundendaten")),
     ]),
    ("k3-quelle-ohne-klasse-intern", ["K4", "A35"],
     "Mit Vorgabestufe intern erbt ein Ergebnis aus einer Quelle ohne Klasse intern.",
     V_GELESEN_INTERN, [
         (anf("cloud", [q(None)]), ok("intern")),
     ]),
    ("k4-quelle-ungueltig", ["K2"],
     "Eine Quelle mit ungültiger Klasse macht das Erbe unklar; sie zählt nicht als niedrigste Klasse.",
     V_GELESEN, [
         (anf("lokal", [q("oeffentlich"), q("geheim")]), unklar("quelle-ohne-klasse")),
         (anf("lokal", [q(7)]), unklar("quelle-ohne-klasse")),
         (anf("lokal", [q("")]), unklar("quelle-ohne-klasse")),
         (anf("lokal", [q("INTERN")]), unklar("quelle-ohne-klasse")),
     ]),
    ("k5-kunde-der-quelle", ["K4", "K6", "D5"],
     "Eine Quelle mit Kunde macht das Ergebnis zu Kundendaten dieses Kunden; derselbe Kunde zweimal bleibt einer.",
     V_GELESEN, [
         (anf("lokal", [q("intern", "k-nord")]), ok("kundendaten", ["k-nord"])),
         (anf("lokal", [q("oeffentlich", "k-nord"), q("intern", "k-nord")]), ok("kundendaten", ["k-nord"])),
         (anf("lokal", [q("oeffentlich", None)]), ok("oeffentlich")),
     ]),
    ("k6-mehrere-kunden", ["K6"],
     "Quellen mehrerer Kunden ergeben alle Kunden, sortiert; das Schreibtor lehnt dann mit kunden-gemischt ab.",
     V_GELESEN, [
         (anf("lokal", [q("intern", "k-sued"), q("intern", "k-nord")]), ok("kundendaten", ["k-nord", "k-sued"])),
         (anf("lokal", [q("intern", "k-nord")], sitzungskunde="k-sued"), ok("kundendaten", ["k-nord", "k-sued"])),
     ]),
    ("k6-kunde-ungueltig", ["K2"],
     "Ein Kunde einer Quelle, der kein Schlüssel ist (kein Text, leer, Großbuchstaben), macht das Erbe unklar.",
     V_GELESEN, [
         (anf("lokal", [q("intern", 5)]), unklar("quelle-kunde-ungueltig")),
         (anf("lokal", [q("intern", "")]), unklar("quelle-kunde-ungueltig")),
         (anf("lokal", [q("intern", "K-NORD")]), unklar("quelle-kunde-ungueltig")),
     ]),
    ("k7-kunde-der-sitzung", ["K6"],
     "Der Kunde der Sitzung hebt nur: Er wird Kunde des Ergebnisses, auch ohne Quelle, und macht es zu Kundendaten.",
     V_GELESEN, [
         (anf("cloud", [], sitzungskunde="k-nord"), ok("kundendaten", ["k-nord"])),
         (anf("cloud", [q("oeffentlich")], sitzungskunde="k-nord"), ok("kundendaten", ["k-nord"])),
         (anf("lokal", [q("intern", "k-nord")], sitzungskunde="k-nord"), ok("kundendaten", ["k-nord"])),
     ]),
    ("k7-kunde-der-sitzung-route", ["K3", "K6"],
     "Mit dem Boden der Route: Eine lokale Sitzung für k-nord schreibt Kundendaten von k-nord, nicht Kundendaten ohne Kunde.",
     V_ROUTE, [
         (anf("lokal", [], sitzungskunde="k-nord"), ok("kundendaten", ["k-nord"])),
         (anf("lokal"), ok("kundendaten")),
     ]),
    ("k8-route", ["K1", "A42", "A43"],
     "Ohne Route oder mit einer Route ohne Klasse ist das Erbe unklar, gleich welcher Boden gilt.",
     V_GELESEN, [
         (anf(None), unklar("route-ohne-klasse")),
         (anf(""), unklar("route-ohne-klasse")),
         (anf("cloud-x", [q("oeffentlich")]), unklar("route-ohne-klasse")),
         (anf("cloud-n", [q("oeffentlich")]), unklar("route-ohne-klasse")),
     ]),
    ("k9-reihenfolge", ["K0"], "Die Route wird vor den Quellen geprüft.", V_ROUTE, [
        (anf(None, [q("geheim")]), unklar("route-ohne-klasse")),
        (anf("cloud", [q("geheim"), q("intern", "k-nord")]), unklar("quelle-ohne-klasse")),
    ]),
    ("k10-keine-anfrage", ["K0"], "Eine leere Liste von Anfragen ergibt eine leere Liste von Antworten.", V_ROUTE, []),
]


def erbe(v, a):
    """Answer by rust/spec/VERTRAEGE.md, sections 3 and 4."""
    kl = v["klassen"]
    top = kl[-1]
    stufe = v.get("vorgabestufe", top)
    boden = v.get("erbe", {}).get("boden", "route")
    r = next((x for x in v["routen"] if x["schluessel"] == a["route"]), None)
    if not a["route"] or r is None or r.get("klasse") is None:
        return unklar("route-ohne-klasse")
    stufen = [r["klasse"]] if boden == "route" else []
    kunden = set()
    for e in a["gelesen"]:
        roh = e.get("klasse")
        if roh is None:
            eigene = stufe
        elif isinstance(roh, str) and roh in kl:
            eigene = roh
        else:
            return unklar("quelle-ohne-klasse")
        k_ = e.get("kunde")
        if k_ is not None and not ist_schl(k_):
            return unklar("quelle-kunde-ungueltig")
        if k_ is not None:
            kunden.add(k_)
            stufen.append(top)
        else:
            stufen.append(eigene)
    if a["sitzungskunde"] is not None:
        kunden.add(a["sitzungskunde"])
    if kunden:
        stufen.append(top)
    klasse = max(stufen, key=kl.index) if stufen else None
    return ok(klasse, sorted(kunden))


def ist_schl(x):
    return isinstance(x, str) and SCHLUESSEL.fullmatch(x) is not None


def vorgabe_gueltig(v):
    """Contract 3, section 2 (only what the cases use) plus the block `erbe`."""
    if not isinstance(v, dict) or not set(v) <= {"klassen", "vorgabestufe", "routen", "kunden", "kundendaten_an_cloud", "erbe"}:
        return False
    if not {"klassen", "routen", "kunden"} <= set(v):
        return False
    kl = v["klassen"]
    if not isinstance(kl, list) or not kl or not all(ist_schl(k) for k in kl) or len(set(kl)) != len(kl):
        return False
    if "vorgabestufe" in v and v["vorgabestufe"] not in kl:
        return False
    for r in v["routen"]:
        if not isinstance(r, dict) or not set(r) <= {"schluessel", "art", "klasse"} or not ist_schl(r.get("schluessel")):
            return False
        if r.get("art") not in ("cloud", "lokal") or (r.get("klasse") is not None and r["klasse"] not in kl):
            return False
    if "erbe" in v and (not isinstance(v["erbe"], dict) or set(v["erbe"]) != {"boden"}
                        or v["erbe"]["boden"] not in ("route", "gelesen")):
        return False
    return True


def anfrage_gueltig(a):
    if not isinstance(a, dict) or set(a) != {"route", "sitzungskunde", "gelesen"}:
        return False
    if not isinstance(a["route"], (str, type(None))):
        return False
    if a["sitzungskunde"] is not None and not ist_schl(a["sitzungskunde"]):
        return False
    if not isinstance(a["gelesen"], list):
        return False
    return all(isinstance(e, dict) and set(e) <= {"klasse", "kunde"} for e in a["gelesen"])


def fall_gueltig(fall):
    if not isinstance(fall, dict) or not set(fall) <= {"name", "regeln", "beschreibung", "vorgabe", "anfragen"}:
        return False
    if not vorgabe_gueltig(fall.get("vorgabe")) or not isinstance(fall.get("anfragen"), list):
        return False
    return all(anfrage_gueltig(a) for a in fall["anfragen"])


def u(name, beschreibung, vorgabe=V_ROUTE, anfrage=None, zusatz=None):
    fall = {"vorgabe": vorgabe, "anfragen": [anfrage if anfrage is not None else anf("cloud")]}
    if zusatz:
        fall.update(zusatz)
    return (name, ["K7"], beschreibung, fall)


UNGUELTIG = [
    ("u-kein-json", ["K7"], "Die Eingabe ist kein JSON.", b'{"vorgabe": '),
    ("u-doppelter-schluessel", ["K7"], "Ein Schlüssel steht doppelt, hier in einer Quelle.",
     b'{"vorgabe": ' + json.dumps(V_ROUTE).encode()
     + b', "anfragen": [{"route": "lokal", "sitzungskunde": null, "gelesen": [{"klasse": "kundendaten", "klasse": "oeffentlich"}]}]}'),
    u("u-boden-unbekannt", "Der Boden ist weder route noch gelesen.", vorgabe=mit("nichts")),
    u("u-quelle-feld", "Eine Quelle trägt ein unbekanntes Feld, etwa einen Pfad.",
      anfrage=anf("cloud", [{"klasse": "intern", "pfad": "betrieb/x.md"}])),
    u("u-ohne-sitzungskunde", "Das Feld sitzungskunde fehlt; es muss ausdrücklich null sein.",
      anfrage={"route": "cloud", "gelesen": []}),
    u("u-sitzungskunde-form", "Der Kunde der Sitzung ist keine Kennung.", anfrage=anf("cloud", [], "K-Nord")),
    u("u-feld-im-fall", "Ein unbekanntes Feld im Fall.", zusatz={"zusatz": 1}),
]


def main():
    dateien = {}
    namen = set()
    for name, regeln, beschreibung, v, paare in FAELLE:
        assert name not in namen, name
        namen.add(name)
        fall = {"name": name, "regeln": regeln, "beschreibung": beschreibung, "vorgabe": v, "anfragen": [a for a, _ in paare]}
        assert fall_gueltig(fall), f"{name}: Fall ungueltig"
        for i, (a, erw) in enumerate(paare):
            ref = erbe(v, a)
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
