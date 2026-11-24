#!/usr/bin/env python3
"""Prueft einen Mutationskatalog gegen den Quelltext (Vertrag Mutationskatalog, Teil A).

Aufruf:
    /usr/bin/python3 -I katalog-pruefen.py <katalog.json> [--wurzel <ordner>]

Ohne --wurzel ist die Wurzel drei Ebenen ueber diesem Skript (rust/spec/mutationskatalog -> Repo).
Das Werkzeug liest nur. Es braucht kein Linux und keine fremde Bibliothek.

Rueckgabecodes: 0 kein Befund; 2 Aufruf oder Form falsch; 3 mindestens ein Befund (Zeile "FUND:").
"""

import json
import re
import sys
from pathlib import Path

ID_FORM = re.compile(r"^K[0-9]{2}$")
AN_FORM = re.compile(r"^A[0-9]{2,3}$")
REGEL_FORM = re.compile(r"^R-[A-Z0-9-]{3,40}$")
FN_FORM = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")
ANKER_MIN = 12
FELDER = ("id", "regel", "anforderung", "datei", "funktion", "anker", "mutant", "roter_test")


def fehler(text):
    """Aufruf- oder Formfehler: Code 2, kein Befund ueber den Quelltext."""
    print(f"katalog-pruefen: {text}", file=sys.stderr)
    raise SystemExit(2)


def argumente(argv):
    katalog = None
    wurzel = None
    rest = list(argv)
    while rest:
        a = rest.pop(0)
        if a == "--wurzel":
            if not rest:
                fehler("--wurzel braucht einen Ordner")
            wurzel = rest.pop(0)
        elif a in ("-h", "--hilfe", "--help"):
            print(__doc__)
            raise SystemExit(0)
        elif a.startswith("-"):
            fehler(f"unbekannte Option {a}")
        elif katalog is None:
            katalog = a
        else:
            fehler("mehr als ein Katalog genannt")
    if katalog is None:
        fehler("Aufruf: katalog-pruefen.py <katalog.json> [--wurzel <ordner>]")
    hier = Path(__file__).resolve().parent
    return Path(katalog), Path(wurzel).resolve() if wurzel else hier.parent.parent.parent


def katalog_lesen(pfad):
    try:
        rohtext = pfad.read_text(encoding="utf-8")
    except OSError as e:
        fehler(f"Katalog nicht lesbar: {e}")
    try:
        daten = json.loads(rohtext)
    except ValueError as e:
        fehler(f"Katalog ist kein JSON: {e}")
    if not isinstance(daten, dict):
        fehler("Katalog ist kein Objekt")
    if daten.get("fassung") != 1:
        fehler("Katalog ohne \"fassung\": 1")
    regeln = daten.get("regeln")
    eintraege = daten.get("eintraege")
    if not isinstance(regeln, list) or not regeln:
        fehler("\"regeln\" fehlt oder ist leer")
    if not isinstance(eintraege, list) or not eintraege:
        fehler("\"eintraege\" fehlt oder ist leer")
    return regeln, eintraege


def regeln_pruefen(regeln):
    """Die Regeltabelle; gibt die Menge der Regel-Kennungen."""
    kennungen = set()
    for i, r in enumerate(regeln):
        if not isinstance(r, dict):
            fehler(f"Regel {i} ist kein Objekt")
        for feld in ("id", "titel", "anforderung"):
            if not isinstance(r.get(feld), str) or not r[feld].strip():
                fehler(f"Regel {i}: Feld \"{feld}\" fehlt oder ist leer")
        if not REGEL_FORM.match(r["id"]):
            fehler(f"Regel {r['id']}: Kennung nicht in der Form R-NAME")
        if r["id"] in kennungen:
            fehler(f"Regel {r['id']}: Kennung zweimal")
        if not AN_FORM.match(r["anforderung"]):
            fehler(f"Regel {r['id']}: \"anforderung\" nicht in der Form A23")
        kennungen.add(r["id"])
    return kennungen


def funktion_ueber(text, stelle):
    """Der Name der naechsten Funktion oberhalb von `stelle`, oder None."""
    letzte = None
    for treffer in FN_FORM.finditer(text, 0, stelle):
        letzte = treffer.group(1)
    return letzte


def eintrag_pruefen(e, nummer, regelkennungen, wurzel, gesehen_ids, gesehen_anker):
    """Prueft einen Eintrag. Gibt die Liste seiner Befunde (leer heisst gruen)."""
    funde = []
    if not isinstance(e, dict):
        fehler(f"Eintrag {nummer} ist kein Objekt")
    for feld in FELDER:
        if not isinstance(e.get(feld), str) or not e[feld].strip():
            fehler(f"Eintrag {nummer}: Feld \"{feld}\" fehlt oder ist leer")
    kennung = e["id"]
    if not ID_FORM.match(kennung):
        fehler(f"Eintrag {nummer}: \"id\" {kennung} nicht in der Form K01")
    if kennung in gesehen_ids:
        fehler(f"Eintrag {kennung}: Kennung zweimal")
    gesehen_ids.add(kennung)
    if not AN_FORM.match(e["anforderung"]):
        fehler(f"{kennung}: \"anforderung\" nicht in der Form A23")
    if e["regel"] not in regelkennungen:
        funde.append(f"{kennung}: die Regel {e['regel']} steht nicht in \"regeln\"")

    anker = e["anker"]
    if "\n" in anker:
        fehler(f"{kennung}: der Anker hat einen Zeilenumbruch")
    if len(anker.encode("utf-8")) < ANKER_MIN:
        fehler(f"{kennung}: der Anker ist kuerzer als {ANKER_MIN} Byte")
    if anker in gesehen_anker:
        funde.append(f"{kennung}: derselbe Anker steht schon in {gesehen_anker[anker]}")
    else:
        gesehen_anker[anker] = kennung

    datei = e["datei"]
    if datei.startswith("/") or ".." in Path(datei).parts:
        fehler(f"{kennung}: \"datei\" {datei} ist nicht relativ oder enthaelt ..")
    pfad = wurzel / datei
    try:
        text = pfad.read_text(encoding="utf-8")
    except OSError:
        funde.append(f"{kennung}: Mutant ohne Ziel, die Datei {datei} ist nicht lesbar")
        return funde

    treffer = text.count(anker)
    if treffer == 0:
        funde.append(f"{kennung}: Mutant ohne Ziel, der Anker steht nicht in {datei}")
    elif treffer > 1:
        funde.append(f"{kennung}: der Anker steht {treffer}-mal in {datei}, nicht genau einmal")
    else:
        hat = funktion_ueber(text, text.index(anker))
        if hat != e["funktion"]:
            funde.append(
                f"{kennung}: der Anker steht in der Funktion {hat}, der Eintrag nennt {e['funktion']}"
            )
    return funde


def main(argv):
    katalogpfad, wurzel = argumente(argv)
    if not wurzel.is_dir():
        fehler(f"Wurzel {wurzel} ist kein Ordner")
    regeln, eintraege = katalog_lesen(katalogpfad)
    regelkennungen = regeln_pruefen(regeln)

    funde = []
    benutzt = set()
    gesehen_ids = set()
    gesehen_anker = {}
    for nummer, e in enumerate(eintraege, 1):
        eigene = eintrag_pruefen(e, nummer, regelkennungen, wurzel, gesehen_ids, gesehen_anker)
        funde.extend(eigene)
        benutzt.add(e["regel"])
        stand = "FUND" if eigene else "ok"
        print(f"{stand} {e['id']} {e['regel']} {e['datei']}:{e['funktion']}")

    for r in sorted(regelkennungen - benutzt):
        funde.append(f"{r}: die Regel hat keinen Eintrag")

    for zeile in funde:
        print(f"FUND: {zeile}")
    print(
        f"katalog: {len(eintraege)} Eintraege, {len(regelkennungen)} Regeln, "
        f"{len(funde)} Funde, Wurzel {wurzel}"
    )
    return 3 if funde else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
