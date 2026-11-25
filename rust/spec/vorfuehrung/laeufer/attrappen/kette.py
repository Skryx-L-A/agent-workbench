#!/usr/bin/env python3
"""Attrappe von Vertrag 5 (`rust/spec/protokollzeile/README.md`): Zeile `ausgabe.gesperrt`, Schreiber und `kette pruefe`.

Als Modul: `anhaengen_gesperrt(datei, felder, fehler=())` (Such-Attrappe).
Als Programm `kette`: `kette.py pruefe [--vorher <64 hex>] [--letzte <64 hex>] [--fehler <name>] <datei>...`
Geprueft werden die Schritte des Vertrags fuer Fassung 2 (lesbar, Fassung, vorher, kanonisch, geschlossene Form,
`--letzte`). Die Fassungen 1g und 1s kennt die Attrappe nicht.
`--fehler kette-blind`: die Attrappe prueft `--letzte` nicht (Mutationsprobe von S4).
"""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import re
import sys

NULL64 = "0" * 64
HEX64 = re.compile(r"^[0-9a-f]{64}$")
SCHLUESSEL = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
ZEIT = re.compile(r"^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$")
FELDER = ("aktionsname", "anzahl", "ergebnis", "fassung", "grund", "klasse", "kunde", "route", "sitzung", "vorher", "zeit")


def kanonisch(obj):
    # RFC 8785 fuer ASCII-Schluessel, ganze Zahlen und Texte (mehr kommt in der Zeile nicht vor).
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def zeit_text(ms):
    import datetime
    d = datetime.datetime.utcfromtimestamp(ms // 1000)
    return d.strftime("%Y-%m-%dT%H:%M:%S.") + "%03dZ" % (ms % 1000)


def anhaengen_gesperrt(datei, g, fehler=()):
    """g: route, sitzung, grund, klasse, kunde, anzahl, zeit_ms. Gibt den sha256 der Zeile zurueck."""
    fd = os.open(datei, os.O_RDWR | os.O_APPEND | os.O_CREAT, 0o600)
    with os.fdopen(fd, "r+b") as f:
        fcntl.flock(f.fileno(), fcntl.LOCK_EX)
        f.seek(0)
        inhalt = f.read()
        if inhalt and not inhalt.endswith(b"\n"):
            raise ValueError("letzte_zeile_unvollstaendig")
        vorher = sha(inhalt.split(b"\n")[-2]) if inhalt else NULL64
        zeile = {"aktionsname": "ausgabe.gesperrt", "anzahl": g["anzahl"], "ergebnis": "gesperrt", "fassung": 2,
                 "grund": g["grund"], "klasse": g["klasse"], "kunde": g["kunde"], "route": g["route"],
                 "sitzung": g["sitzung"], "vorher": vorher, "zeit": zeit_text(g["zeit_ms"])}
        if "pfad-im-protokoll" in fehler:
            zeile["pfad"] = g.get("pfad", "kunden/beispiel/notiz.md")
        roh = kanonisch(zeile).encode("utf-8")
        f.write(roh + b"\n")
        f.flush()
        os.fsync(f.fileno())
        fcntl.flock(f.fileno(), fcntl.LOCK_UN)
    return sha(roh)


def pruefe(dateien, vorher=None, letzte=None, blind=False):
    """Gibt (exit, ausgabe) wie `kette pruefe`."""
    erwartet = vorher or NULL64
    zeilen = 0
    letzte_hash = None
    letzte_ref = (None, None)

    def bruch(datei, grund, zeile):
        return 1, {"bruch": {"datei": os.path.basename(datei) if datei else None, "grund": grund, "zeile": zeile},
                   "letzte": None, "ok": False, "zeilen": zeilen}

    for datei in dateien:
        try:
            with open(datei, "rb") as f:
                inhalt = f.read()
        except OSError:
            return bruch(datei, "datei_nicht_lesbar", None)
        if not inhalt:
            continue
        if not inhalt.endswith(b"\n"):
            return bruch(datei, "letzte_zeile_unvollstaendig", len(inhalt.split(b"\n")))
        for nr, roh in enumerate(inhalt[:-1].split(b"\n"), 1):
            try:
                text = roh.decode("utf-8")
                wert = json.loads(text)
                if not isinstance(wert, dict) or not roh or len(roh) > 4194304:
                    raise ValueError
            except ValueError:
                return bruch(datei, "zeile_unlesbar", nr)
            if wert.get("fassung") != 2 or isinstance(wert.get("fassung"), bool):
                return bruch(datei, "form_falsch" if "fassung" in wert else "fassung_fehlt", nr)
            if "vorher" not in wert:
                return bruch(datei, "vorher_fehlt", nr)
            if wert["vorher"] != erwartet:
                return bruch(datei, "kette_gebrochen", nr)
            if kanonisch(wert).encode("utf-8") != roh:
                return bruch(datei, "nicht_kanonisch", nr)
            if not isinstance(wert.get("zeit"), str) or not ZEIT.match(wert["zeit"]) \
                    or not isinstance(wert.get("aktionsname"), str) or not isinstance(wert.get("ergebnis"), str):
                return bruch(datei, "form_falsch", nr)
            if wert["aktionsname"] == "ausgabe.gesperrt" and not _form_gesperrt(wert):
                return bruch(datei, "form_falsch", nr)
            zeilen += 1
            erwartet = sha(roh)
            letzte_hash = erwartet
            letzte_ref = (datei, nr)
    if letzte is not None and not blind and letzte != letzte_hash:
        zeilen_ohne = zeilen
        exit_, aus = bruch(letzte_ref[0], "letzte_falsch", letzte_ref[1])
        aus["zeilen"] = zeilen_ohne
        return exit_, aus
    return 0, {"bruch": None, "letzte": letzte_hash, "ok": True, "zeilen": zeilen}


def _form_gesperrt(w):
    if tuple(sorted(w)) != FELDER:
        return False
    if w["aktionsname"] != "ausgabe.gesperrt" or w["ergebnis"] != "gesperrt" or w["fassung"] != 2:
        return False
    a = w["anzahl"]
    if not isinstance(a, int) or isinstance(a, bool) or not 1 <= a <= 4294967295:
        return False
    for k in ("grund", "route", "sitzung"):
        if not isinstance(w[k], str) or not SCHLUESSEL.match(w[k]):
            return False
    for k in ("klasse", "kunde"):
        if w[k] is not None and (not isinstance(w[k], str) or not SCHLUESSEL.match(w[k])):
            return False
    return bool(HEX64.match(w["vorher"]))


QUELLEN = ("gateway", "stelle", "brain", "halter", "ordnermarke")


def main(argv):
    args = list(argv)
    if not args or args[0] not in ("pruefe", "pruefe-quelle"):
        return 64
    quelle_modus = args[0] == "pruefe-quelle"
    args = args[1:]
    vorher = letzte = None
    fehler = []
    dateien = []
    gesehen = set()
    i = 0
    while i < len(args):
        a = args[i]
        if a.startswith("--"):
            if a in gesehen or i + 1 >= len(args):
                return 64
            gesehen.add(a)
            w = args[i + 1]
            if a == "--fehler":
                fehler = w.split(",")
            elif a in ("--vorher", "--letzte"):
                if not HEX64.match(w):
                    return 64
                if a == "--vorher":
                    vorher = w
                else:
                    letzte = w
            else:
                return 64
            i += 2
        else:
            dateien.append(a)
            i += 1
    if quelle_modus:
        # `pruefe-quelle <ordner> <quelle>`: die Monatsdateien `<quelle>-<JJJJ-MM>.jsonl` in aufsteigender Folge als eine Kette
        # (Anker kennt die Attrappe nicht). Die Quelle `ordnermarke` steht noch nicht in Vertrag 5 (siehe README, OPEN).
        if len(dateien) != 2 or dateien[1] not in QUELLEN:
            return 64
        ordner, quelle = dateien
        try:
            namen = sorted(n for n in os.listdir(ordner) if re.match(r"^%s-[0-9]{4}-[0-9]{2}\.jsonl$" % quelle, n))
        except OSError:
            sys.stdout.write(kanonisch({"bruch": {"datei": os.path.basename(ordner), "grund": "datei_nicht_lesbar", "zeile": None},
                                        "letzte": None, "ok": False, "zeilen": 0}) + "\n")
            return 1
        dateien = [os.path.join(ordner, n) for n in namen]
        if not dateien:
            exit_, aus = pruefe([], vorher, letzte)
            sys.stdout.write(kanonisch(aus) + "\n")
            return exit_
    if not dateien:
        return 64
    exit_, aus = pruefe(dateien, vorher, letzte, blind="kette-blind" in fehler)
    sys.stdout.write(kanonisch(aus) + "\n")
    return exit_


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
