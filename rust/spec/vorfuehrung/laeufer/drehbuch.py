#!/usr/bin/env python3
"""Laeufer `drehbuch`: fuehrt S1 bis S5 und E1 bis E3 aus `rust/spec/vorfuehrung/DREHBUCH.md` nacheinander gegen eine
Testinstallation aus und schreibt je Schritt eine Zeile mit Ergebnis, Messwerten und Gegenprobe.

Aufruf (nur Standardbibliothek, Python 3.9 oder neuer):
  python3 -I -B drehbuch.py probelauf --ergebnis <datei> [--konfig <datei>] [--nur-cloud]   Probelauf auf dem Zielrechner (README);
                                                                     --nur-cloud: Messbetrieb, Exit 1 bei rotem Schritt, sonst 4, nie 0
  python3 -I -B drehbuch.py pruefe-ergebnis <datei> [--konfig <datei>] [--verboten <wert>]...
  python3 -I -B drehbuch.py selbsttest-ergebnis                                    Fall: Ergebnisdatei ohne Inhalte
  python3 -I -B drehbuch.py --ziel attrappe|echt [--konfig <datei>] [--fehler <name,...>] [--nur S3,S4]
                            [--arbeit <ordner>] [--json <datei>|-] [--behalte]
Exit: 0 alle Schritte und Gegenproben gruen; 1 mindestens ein Schritt rot, offen oder uebersprungen;
      2 Aufruf- oder Konfigurationsfehler; 3 Aufbau des Ziels gescheitert.
"""
from __future__ import annotations

import argparse
import os
import sys
import time

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, bericht, konfig, messen, probelauf  # noqa: E402

NAMEN = ablauf.NAMEN


def main(argv):
    if argv[:1] == ["probelauf"]:
        return probelauf.probelauf(argv[1:], HIER)
    if argv[:1] == ["pruefe-ergebnis"]:
        return probelauf.pruefe_ergebnis(argv[1:], HIER)
    if argv[:1] == ["selbsttest-ergebnis"]:
        return probelauf.selbsttest(argv[1:], HIER)
    ap = argparse.ArgumentParser(description="Laeufer drehbuch (Vorfuehrdrehbuch S1 bis S5, E1 bis E3)")
    ap.add_argument("--ziel", required=True, choices=["attrappe", "echt"])
    ap.add_argument("--konfig")
    ap.add_argument("--fehler", default="", help="nur gegen die Attrappen: Fehler zum Einschalten, kommagetrennt")
    ap.add_argument("--nur", default="", help="nur diese Schritte (S1 laeuft immer zuerst), kommagetrennt")
    ap.add_argument("--browser", choices=["aus", "auto", "playwright"], help="Seite der Bruecke mit Playwright bedienen (Vorgabe: Konfiguration)")
    ap.add_argument("--seite-ordner", dest="seite_ordner", help="nur Attrappe: Ordner mit index.html, ansicht.js, ansicht_bg.wasm statt seite.html (ueberschreibt `seite_ordner`)")
    ap.add_argument("--kein-selbstwechsel", dest="kein_selbstwechsel", action="store_true", help="nur Attrappe: die Seite der Attrappe wechselt nicht selbst in eine neu gestartete Sitzung (setzt `attrappe.kein_selbstwechsel`)")
    ap.add_argument("--vault", help="ueberschreibt `vault` der Konfiguration (Vorfuehr-Vault)")
    ap.add_argument("--arbeit", help="Arbeitsordner; Vorgabe: neuer Ordner unter $TMPDIR")
    ap.add_argument("--json", dest="json_pfad", help="Pfad fuer den JSON-Bericht; `-` schreibt ihn nach stdout")
    ap.add_argument("--behalte", action="store_true", help="Arbeitsordner nach dem Lauf nicht loeschen")
    a = ap.parse_args(argv)
    fehler = [f for f in a.fehler.split(",") if f]
    nur = [n for n in a.nur.split(",") if n]
    if any(n not in NAMEN for n in nur):
        print("unbekannter Schritt in --nur", file=sys.stderr)
        return 2
    if a.ziel == "echt" and fehler:
        print("--fehler gibt es nur gegen die Attrappen", file=sys.stderr)
        return 2
    kpfad = a.konfig or os.path.join(HIER, "konfig.attrappe.json" if a.ziel == "attrappe" else "konfig.probelauf.json")
    eigener_ordner = a.arbeit is None
    arbeit = os.path.realpath(a.arbeit) if a.arbeit else messen.neuer_tempordner(prefix="dbl-")
    os.makedirs(arbeit, exist_ok=True)
    try:
        k = konfig.lade(kpfad, HIER, arbeit)
        if a.vault:
            k["vault"] = os.path.realpath(a.vault)
        if a.kein_selbstwechsel:
            k.setdefault("attrappe", {})["kein_selbstwechsel"] = True
        if a.seite_ordner:
            k["seite_ordner"] = os.path.realpath(a.seite_ordner)
        elif k.get("seite_ordner"):
            k["seite_ordner"] = os.path.realpath(os.path.join(HIER, k["seite_ordner"]))
        if k.get("seite_ordner") and a.ziel != "attrappe":
            raise konfig.KonfigFehler("seite_ordner gibt es nur gegen die Attrappe (die echte Bruecke liefert ihre Seite selbst)")
        if k["ziel"] != a.ziel:
            raise konfig.KonfigFehler("Konfiguration ist fuer ziel=%s, nicht %s" % (k["ziel"], a.ziel))
        if len(arbeit) > 60 and a.ziel == "attrappe":
            raise konfig.KonfigFehler("Arbeitsordner zu tief fuer Unix-Sockets (sun_path): " + arbeit)
    except konfig.KonfigFehler as e:
        print("Konfigurationsfehler: %s" % e, file=sys.stderr)
        if eigener_ordner:
            messen.sicher_loeschen(arbeit)
        return 2
    t_start = time.monotonic()
    try:
        ergebnisse, meldungen, info = ablauf.fuehre_aus(k, arbeit, fehler, nur, a.browser)
    except ablauf.AufbauFehler as e:
        print("Aufbau des Ziels gescheitert: %s%s" % (e, " (%s)" % e.hinweis if e.hinweis else ""), file=sys.stderr)  # feste Codes
        if a.json_pfad and a.json_pfad != "-":  # Text mit veraenderlichem Inhalt nur in die Diagnosedatei neben den Bericht
            if messen.diagnose_schreibe(a.json_pfad):
                print(messen.diagnose_zeile(), file=sys.stderr)
        return 3
    seite = info.get("seite") if a.ziel == "attrappe" else None
    if seite:
        sys.path.insert(0, os.path.join(HIER, "attrappen"))  # nur die Attrappe braucht den Ordner; das Paket liefert `attrappen/` nicht aus
        import seitenordner
        print(seitenordner.zeile(seite))
    print(bericht.tabelle(ergebnisse))
    probleme = bericht.gruende(ergebnisse) + meldungen
    if probleme:
        print("\nBefunde:")
        for p in probleme:
            print("  " + p)
    text = bericht.json_text(a.ziel, ergebnisse, time.monotonic() - t_start, meldungen, seite)
    if a.json_pfad == "-":
        print(text)
    else:
        pfad = a.json_pfad or os.path.join(arbeit, "bericht.json")
        with open(pfad, "w", encoding="utf-8") as f:
            f.write(text + "\n")
        print("\nJSON: %s" % pfad)
        messen.diagnose_schreibe(pfad)  # nur wenn es veraenderlichen Text gibt: `<bericht>.diagnose`, 0600, nie gelesen, nie geloescht
        if messen.diagnose_zeile():
            print(messen.diagnose_zeile())
    gruen = bool(ergebnisse) and all(r["ergebnis"] == "gruen" for r in ergebnisse) and not meldungen
    print("Ergebnis: %s (%.1f s)" % ("GRUEN" if gruen else "NICHT GRUEN", time.monotonic() - t_start))
    if eigener_ordner and not a.behalte and (a.json_pfad or "").strip() != "":
        messen.sicher_loeschen(arbeit)  # der Bericht liegt dann ausserhalb oder auf stdout
    return 0 if gruen else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
