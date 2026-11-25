#!/usr/bin/env python3
"""Attrappe des Tors `darf(einheit, route, kunde)` nach Vertrag 3 (`rust/spec/datentor/VERTRAG.md`, Abschnitte 2 bis 5).

Gleiche Regeln D2 bis D11 wie der Vertrag, nur Standardbibliothek. Zwei Nutzungen:
  - als Modul: `darf(vorgabe, einheit, route, kunde)` fuer die Such-Attrappe,
  - als Programm `datentor`: `darf.py [--fehler d11-fehlt] < fall.json` mit der Form aus Abschnitt 5.
Mit `--fehler d11-fehlt` laesst die Attrappe die Regel D11 aus (Mutationsprobe von Schritt S3, Gegenprobe).
"""
from __future__ import annotations

import json
import sys

MELDUNG = {
    "route-fehlt": "Anfrage ohne Route",
    "route-ohne-klasse": "Route {route} hat keine Klasse",
    "einheit-ohne-klasse": "Einheit ohne gültige Klasse",
    "klasse-ueber-route": "Klasse der Einheit liegt über Route {route}",
    "kunde-fehlt": "Kundendaten ohne Kunde",
    "kunde-ohne-eintrag": "Kunde ohne Eintrag in der Kundenvorgabe",
    "route-fuer-kunde-verboten": "Route {route} ist für diesen Kunden verboten",
    "route-fuer-kunde-nicht-erlaubt": "Route {route} ist für diesen Kunden nicht erlaubt",
    "kundendaten-an-cloud": "Kundendaten an Cloud-Route {route} ohne Freigabe",
    "frei": "frei",
}


def _antwort(grund, klasse, route):
    return {"darf": grund == "frei", "grund": grund, "klasse": klasse,
            "meldung": MELDUNG[grund].replace("{route}", str(route))}


def darf(vorgabe, einheit, route, kunde, fehler=()):
    klassen = vorgabe["klassen"]
    hoechste = klassen[-1]
    vorgabestufe = vorgabe.get("vorgabestufe", hoechste)
    roh = einheit.get("klasse") if isinstance(einheit, dict) else None
    gueltig = roh is None or (isinstance(roh, str) and roh in klassen)
    if not gueltig:
        wirksam = None
    elif kunde is not None:
        wirksam = hoechste
    else:
        wirksam = vorgabestufe if roh is None else roh
    if route is None or route == "":
        return _antwort("route-fehlt", wirksam, route)
    r = next((x for x in vorgabe["routen"] if x["schluessel"] == route), None)
    if r is None or not r.get("klasse"):
        return _antwort("route-ohne-klasse", wirksam, route)
    if not gueltig:
        return _antwort("einheit-ohne-klasse", None, route)
    if klassen.index(wirksam) > klassen.index(r["klasse"]):
        return _antwort("klasse-ueber-route", wirksam, route)
    if wirksam == hoechste or kunde is not None:
        if kunde is None:
            return _antwort("kunde-fehlt", wirksam, route)
        k = next((x for x in vorgabe["kunden"] if x["schluessel"] == kunde), None)
        if k is None:
            return _antwort("kunde-ohne-eintrag", wirksam, route)
        if route in k.get("verboten", []):
            return _antwort("route-fuer-kunde-verboten", wirksam, route)
        if route not in k.get("erlaubt", []):
            return _antwort("route-fuer-kunde-nicht-erlaubt", wirksam, route)
        frei = vorgabe.get("kundendaten_an_cloud", {}).get("freigaben", [])
        if r["art"] == "cloud" and "d11-fehlt" not in fehler \
                and not any(f["route"] == route and f["kunde"] == kunde for f in frei):
            return _antwort("kundendaten-an-cloud", wirksam, route)
    return _antwort("frei", wirksam, route)


def main(argv):
    fehler = []
    args = list(argv)
    if args[:1] == ["--fehler"] and len(args) >= 2:
        fehler = args[1].split(",")
        args = args[2:]
    if args not in ([], ["-"]):
        print("Aufruf: datentor [-] < fall.json", file=sys.stderr)
        return 2
    try:
        fall = json.loads(sys.stdin.read(1048577))
        vorgabe, anfragen = fall["vorgabe"], fall["anfragen"]
        antworten = [darf(vorgabe, a["einheit"], a.get("route"), a.get("kunde"), fehler) for a in anfragen]
    except (ValueError, KeyError, TypeError):
        print("ungueltiger Fall", file=sys.stderr)
        return 2
    sys.stdout.write(json.dumps({"antworten": antworten}, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
