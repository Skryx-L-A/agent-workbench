#!/usr/bin/env python3
"""Attrappe des Befehls, mit dem ein Mensch die Ordnermarke loest (A156, ausserhalb des Einschlusses).

Aufruf: marke_loeschen.py --zustand <ordner> --protokoll <ordner> [--fehler <name>] <arbeitsordner>
Entfernt den Eintrag des aufgeloesten Pfads aus `<zustand>/ordnermarken.json` (atomar unter flock auf `ordnermarken.lock`) und
haengt eine Protokollzeile `ordnermarke.geloest` an `<protokoll>/ordnermarke-<JJJJ-MM>.jsonl` an (Fassung 2, Kette wie bei den
anderen Zeilen). Die Zeile traegt kein Pfad und keinen Hash des Pfads, sondern das Feld `marke` mit der Kennung der Marke.
Exit 0 gelungen, 1 keine Marke, 64 Aufruffehler, 78 Datei kaputt.
`--fehler e6-loeschzeile-mit-pfad`: die Zeile traegt den Pfad (Mutationsprobe von E6).
"""
from __future__ import annotations

import fcntl
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kette  # noqa: E402


def main(argv):
    a = list(argv)
    fehler = ""
    if len(a) == 7 and a[4] == "--fehler":
        fehler = a[5]
        a = a[:4] + a[6:]
    if len(a) != 5 or a[0] != "--zustand" or a[2] != "--protokoll":
        return 64
    zustand, protokoll, ordner = a[1], a[3], os.path.realpath(a[4])
    datei = os.path.join(zustand, "ordnermarken.json")
    with open(os.path.join(zustand, "ordnermarken.lock"), "a") as sperre:
        fcntl.flock(sperre.fileno(), fcntl.LOCK_EX)
        try:
            with open(datei, "rb") as f:
                d = json.loads(f.read().decode("utf-8"))
            marken = d["marken"]
        except FileNotFoundError:
            return 1
        except (OSError, ValueError, KeyError, TypeError):
            return 78
        eintrag = next((m for m in marken if m.get("ordner") == ordner), None)
        if eintrag is None:
            return 1
        rest = [m for m in marken if m is not eintrag]
        roh = json.dumps({"fassung": 1, "marken": rest}, sort_keys=True, separators=(",", ":")).encode("utf-8")
        fd = os.open(datei + ".neu", os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(roh)
            f.flush()
            os.fsync(f.fileno())
        os.replace(datei + ".neu", datei)
    jetzt = int(time.time() * 1000)
    pdatei = os.path.join(protokoll, "ordnermarke-%s.jsonl" % kette.zeit_text(jetzt)[:7])
    vorher = kette.NULL64
    if os.path.exists(pdatei):
        zeilen = open(pdatei, "rb").read().split(b"\n")[:-1]
        if zeilen:
            vorher = kette.sha(zeilen[-1])
    z = {"aktionsname": "ordnermarke.geloest", "ergebnis": "geloest", "fassung": 2, "marke": eintrag["kennung"],
         "vorher": vorher, "zeit": kette.zeit_text(jetzt)}
    if fehler == "e6-loeschzeile-mit-pfad":
        z["ordner"] = ordner
    with open(pdatei, "ab") as f:
        f.write(kette.kanonisch(z).encode("utf-8") + b"\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
