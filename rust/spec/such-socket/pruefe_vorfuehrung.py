#!/usr/bin/env python3
"""Prüft den Vorführ-Vault gegen das Drehbuch (`rust/spec/vorfuehrung/DREHBUCH.md`, S3, S4, S5, E2).

Ohne Programm des Baus: Die Liste ohne Tor kommt aus der Referenz des Vertrags `rangkern`, das Tor aus `bauen.py`.
Aufruf: REF_SICHER=1 <python mit braincli> -I -B pruefe_vorfuehrung.py <ordner mit rangkern_ref.py>
Exit 0 grün, 1 rot.
"""
from __future__ import annotations

import contextlib
import io
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
VAULT = HIER / "vorfuehr-vault"
FRAGE_F = "Wann ist das nächste Wartungsfenster der Presslinie?"
N_K = "kunden/k-nord/MK-7Q2-liefertermin-presslinie.md"
N_S = "kunden/k-sued/MS-3P8-abnahme-presslinie.md"
N_I = "betrieb/MI-4F9-wartungsfenster-presslinie.md"
MARKER = {"MK-7Q2": N_K, "MI-4F9": N_I, "MS-3P8": N_S}
# Einstellungen der Vorführung nach der Antwort der Firma (Entscheidung vom 20.11.2026): Klassen der Firma als
# Kennungen, Route copilot (cloud, internal), Route lokal (lokal, customer). k-nord hat die Freigabe für lokal,
# k-sued steht ohne Freigabe drin, `erlaubt: []` (Kundenachse: auch die lokale Route sperrt; Entscheidung, Drehbuch N-S).
EINSTELLUNGEN = {
    "fassung": 1,
    "klassen": ["public", "internal", "confidential", "customer"],
    "vorgabestufe": "customer",
    "routen": [{"schluessel": "copilot", "art": "cloud", "klasse": "internal"},
               {"schluessel": "lokal", "art": "lokal", "klasse": "customer"}],
    "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []},
               {"schluessel": "k-sued", "erlaubt": [], "verboten": []}],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
}
# Je Route: im ganzen Vault gesperrte Notizen; davon die, die auf F passen und deshalb gezählt werden; Grund je Notiz.
# Keine Notiz trägt `confidential`: 19 von 21 Notizen passen über 4-Gramme lexikalisch auf F, eine weitere gesperrte Notiz
# machte die Zahlen in S3 und S5 von der Rangfolge mit Einbettung abhängig (gemessen 20.11.).
SOLL = {
    "copilot": {"gesperrt_alle": {N_K, N_S}, "gesperrt_f": {N_K, N_S},
                "gruende": {N_K: ("klasse-ueber-route", "customer"), N_S: ("klasse-ueber-route", "customer")},
                "treffer_f": {N_I}},
    "lokal": {"gesperrt_alle": {N_S}, "gesperrt_f": {N_S},
              "gruende": {N_S: ("route-fuer-kunde-nicht-erlaubt", "customer")}, "treffer_f": {N_I, N_K}},
}
PROTOKOLLWERTE = ["ausgabe.gesperrt", "gesperrt", "klasse-ueber-route", "route-fuer-kunde-nicht-erlaubt", "customer",
                  "k-nord", "k-sued", "copilot", "lokal"]

fehler: list[str] = []


def frontmatter(text: str) -> dict:
    teile = text.split("---\n")
    out = {}
    for z in teile[1].splitlines() if len(teile) > 2 else []:
        k, _, v = z.partition(": ")
        try:
            out[k] = json.loads(v)
        except ValueError:
            out[k] = v
    return out


def main() -> int:
    sys.path.insert(0, str(Path(sys.argv[1]).resolve()))
    sys.path.insert(0, str(HIER))
    import bauen  # noqa: E402
    import rangkern_ref as ref  # noqa: E402

    notizen = {str(p.relative_to(VAULT)): p.read_text(encoding="utf-8") for p in sorted(VAULT.rglob("*.md"))}
    print(f"{len(notizen)} Notizen")
    fm = {rel: frontmatter(t) for rel, t in notizen.items()}

    # Marker: genau in einer Notiz, dort in Pfad, Titel und Text (Drehbuch, Abschnitt 2)
    for marke, rel in MARKER.items():
        mit = [r for r, t in notizen.items() if marke in t or marke in r]
        if mit != [rel]:
            fehler.append(f"Marker {marke} steht in {mit}, erwartet nur {rel}")
        titel = str(fm[rel].get("title", ""))
        rumpf = notizen[rel].split("---\n", 2)[2]
        if marke not in rel or marke not in titel or marke not in rumpf:
            fehler.append(f"Marker {marke} fehlt in Pfad, Titel oder Text von {rel}")

    # Rangliste ohne Tor für F über den Rückfallweg: Sie ist die lexikalische Liste (BM25)
    roh = io.BytesIO()
    aus = io.TextIOWrapper(roh, encoding="utf-8")
    with contextlib.redirect_stdout(aus):
        code = ref.main(["search", str(VAULT), "-k", "50", "--today", "2026-11-20", "--", FRAGE_F])
        aus.flush()
    liste = [h["rel"] for h in json.loads(roh.getvalue().decode())["hits"]] if code == 0 else []
    print("Liste ohne Tor für F:", liste[:6])
    if len(liste) < 5:
        fehler.append(f"Nur {len(liste)} lexikalische Treffer für F; mit Einbettung könnte eine andere Notiz in die ersten 5")

    for route, soll in SOLL.items():
        urteil = {}
        for rel in notizen:
            klasse, kunde, _ = bauen.eingaben(fm[rel])
            urteil[rel] = bauen.darf(EINSTELLUNGEN, klasse, kunde, route)
        gesperrt = {r for r, u in urteil.items() if not u[0]}
        if gesperrt != soll["gesperrt_alle"]:
            fehler.append(f"Route {route}: im Vault gesperrt {sorted(gesperrt)}, erwartet {sorted(soll['gesperrt_alle'])}")
        for rel, (grund, klasse) in soll["gruende"].items():
            if urteil[rel][1:] != (grund, klasse):
                fehler.append(f"Route {route}: {rel} mit {urteil[rel][1:]}, erwartet {(grund, klasse)}")
        # Gezählt werden die Gesperrten unter den ersten 5 der Liste ohne Tor (Vertrag 4).
        gesperrt_f = {r for r in liste[:5] if r in gesperrt}
        if gesperrt_f != soll["gesperrt_f"]:
            fehler.append(f"Route {route}: unter den ersten 5 für F gesperrt {sorted(gesperrt_f)}, erwartet {sorted(soll['gesperrt_f'])}")
        # Robust mit Einbettung: Eine gesperrte Notiz, die nicht lexikalisch auf F passt, kann bei mindestens 5
        # lexikalischen Treffern nicht unter die ersten 5 der Fusion (sie hat nur den Beitrag der Vektorliste, hergeleitet).
        lexikalisch_gesperrt = {r for r in liste if r in gesperrt}
        if lexikalisch_gesperrt != soll["gesperrt_f"]:
            fehler.append(f"Route {route}: lexikalisch passende Gesperrte {sorted(lexikalisch_gesperrt)}")
        erlaubt5 = [r for r in liste if r not in gesperrt][:5]
        for rel in soll["treffer_f"]:
            if rel not in erlaubt5:
                fehler.append(f"Route {route}: {rel} kommt für F nicht unter die ersten 5 Treffer")
        print(f"Route {route}: gesperrt für F {len(gesperrt_f)}, Treffer {erlaubt5}")

    # S4: kein Wort der Frage F in einem Protokollwert, der von der Frage unabhängig ist
    woerter = {w for w in re.findall(r"[a-zäöüß]+", FRAGE_F.lower()) if len(w) >= 3}
    for w in woerter:
        for wert in PROTOKOLLWERTE:
            if w in wert:
                fehler.append(f"Wort {w!r} der Frage F steckt im Protokollwert {wert!r}")
    if re.search(r"\d", FRAGE_F):
        fehler.append("Frage F enthält eine Ziffer; sie könnte in zeit oder vorher vorkommen")

    for f in fehler:
        print("ROT ", f)
    print("grün" if not fehler else f"{len(fehler)} Fehler")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
