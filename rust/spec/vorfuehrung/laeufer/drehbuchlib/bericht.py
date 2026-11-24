"""Ausgabe des Laufs: Tabelle Schritt | Ergebnis | Messwerte | Gegenprobe und JSON."""
from __future__ import annotations

import json


def _kurz(d, grenze=110):
    t = "; ".join("%s=%s" % (k, json.dumps(v, ensure_ascii=False) if not isinstance(v, str) else v) for k, v in d.items())
    return t if len(t) <= grenze else t[:grenze - 3] + "..."


def tabelle(ergebnisse):
    zeilen = [("Schritt", "Ergebnis", "Messwerte", "Gegenprobe")]
    for r in ergebnisse:
        gegen = r["gegenprobe"] + ("" if not r["gegenprobe_messwerte"] else ": " + _kurz(r["gegenprobe_messwerte"], 90))
        zeilen.append(("%s %s" % (r["schritt"], r["name"]), r["ergebnis"], _kurz(r["messwerte"]), gegen))
    breiten = [max(len(z[i]) for z in zeilen) for i in range(4)]
    breiten[2] = min(breiten[2], 112)
    aus = []
    for i, z in enumerate(zeilen):
        aus.append(" | ".join(z[j].ljust(breiten[j]) for j in range(4)).rstrip())
        if i == 0:
            aus.append("-+-".join("-" * b for b in breiten))
    return "\n".join(aus)


def gruende(ergebnisse):
    """Eine Zeile je roter oder offener Teilpruefung."""
    aus = []
    for r in ergebnisse:
        if r["fehler"]:
            aus.append("%s: Fehler: %s" % (r["schritt"], r["fehler"]))
        for t in r["teile"]:
            if not t["ok"]:
                aus.append("%s rot (%s): %s" % (r["schritt"], t["art"], t["name"]))
        for o in r["offen"]:
            aus.append("%s offen: %s" % (r["schritt"], o))
    return aus


def json_text(ziel, ergebnisse, dauer_s, meldungen, seite=None):
    d = {"ziel": ziel, "gruen": all(r["ergebnis"] == "gruen" for r in ergebnisse), "dauer_s": round(dauer_s, 2),
                       "schritte": ergebnisse, "aufraeumen": meldungen}
    if seite:  # nur Namen und Pruefsummen (Attrappe mit Browser)
        d["seite"] = seite
    return json.dumps(d, ensure_ascii=False, indent=1, sort_keys=True)
