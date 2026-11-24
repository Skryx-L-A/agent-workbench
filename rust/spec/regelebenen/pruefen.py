#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Regelebenen gegen Paarung, Codes und Schema.

Aufruf: <python mit jsonschema> -I rust/spec/regelebenen/pruefen.py
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es nur Paarung, strenges JSON-Lesen, Codes und Zeilenform, meldet das Schema in der Ausgabe
als nicht geprüft und endet mit Exit 0 nur bei 0 Fehlern.
Exit 0, wenn jeder prüfbare Punkt stimmt. Querverweise, Lockerung und Befugnis sind erst mit dem Bau prüfbar.
"""
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
SCHEMA = HIER / "regelebenen.schema.json"
SCHEMA_V2 = HIER.parent / "einstellungen" / "einstellungen.schema.json"
# Codes, die das Schema allein erkennt (Stufen 3 bis 11); einst.* prüft das Schema von Vertrag 2.
SCHEMA_CODES = {"ebenen.schluessel_unbekannt", "ebenen.schluessel_fehlt", "ebenen.typ_falsch",
                "ebenen.fassung_unbekannt", "ebenen.anzahl", "ebenen.kennung_form", "ebenen.wert_ungueltig"}
CODES = SCHEMA_CODES | {"ebenen.zu_gross", "ebenen.json_ungueltig", "ebenen.ebene_falsch", "ebenen.kennung_doppelt",
                        "ebenen.route_unbekannt", "ebenen.nutzer_mehrfach", "ebenen.nicht_befugt",
                        "ebenen.gruppe_unbekannt", "ebenen.kunde_unbekannt", "ebenen.klasse_unbekannt",
                        "ebenen.lockerung", "ebenen.keine_route"}
HINWEISE = {"ebenen.lockerung_ignoriert", "ebenen.route_unbekannt_ignoriert", "ebenen.kunde_unbekannt_ignoriert",
            "ebenen.klasse_unbekannt_gesperrt"}
ZEILE = {"aktionsname", "alt", "antrag", "code", "ebene", "ergebnis", "handelnd", "neu", "pflicht", "ziel", "zweiter"}  # Vertrag 5, Abschnitt 6.6
HEX64 = re.compile(r"^[0-9a-f]{64}$")
FALL_FELDER = {"name", "anforderungen", "beschreibung", "stand", "schritte"}


def streng(text):
    def paare(liste):
        obj = {}
        for k, v in liste:
            if k in obj:
                raise ValueError(f"doppelter Schlüssel {k}")
            obj[k] = v
        return obj
    return json.loads(text, object_pairs_hook=paare)


def gueltiger_code(c):
    return c in CODES or (isinstance(c, str) and c.startswith("einst."))


def main():
    try:
        import jsonschema
        p_ebenen = jsonschema.Draft202012Validator(json.loads(SCHEMA.read_text(encoding="utf-8")))
        p_v2 = jsonschema.Draft202012Validator(json.loads(SCHEMA_V2.read_text(encoding="utf-8")))
        jsonschema.Draft202012Validator.check_schema(p_ebenen.schema)
    except ImportError:
        if "--ohne-schema" not in sys.argv[1:]:
            print("pruefen.py: das Modul jsonschema fehlt, das Schema wäre ungeprüft; Exit 2 "
                  "(mit --ohne-schema bewusst ohne Schema prüfen)", file=sys.stderr)
            return 2
        p_ebenen = p_v2 = None
    fehler, zahl, schritte_zahl, nur_bau = [], 0, 0, 0
    faelle = {p.name for p in (HIER / "faelle").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"{name}: ohne Gegenstück")

    def datei_pruefen(wo, datei, soll_gueltig):
        if p_ebenen is None:
            return
        ok = p_ebenen.is_valid(datei)
        if ok and datei.get("ebene") == "firma":
            ok = p_v2.is_valid(datei["einstellung"])
        if soll_gueltig and not ok:
            fehler.append(f"{wo}: Datei soll formgültig sein, Schema lehnt ab")
        if soll_gueltig is False and ok:
            fehler.append(f"{wo}: Schema nimmt an, erwartet Formfehler")

    for name in sorted(faelle & erwartet):
        zahl += 1
        try:
            fall = streng((HIER / "faelle" / name).read_text(encoding="utf-8"))
            erw = streng((HIER / "erwartet" / name).read_text(encoding="utf-8"))
        except ValueError as e:
            fehler.append(f"{name}: {e}")
            continue
        if set(fall) != FALL_FELDER or fall["name"] + ".json" != name:
            fehler.append(f"{name}: Felder des Falls oder Name falsch")
            continue
        if len(erw["schritte"]) != len(fall["schritte"]):
            fehler.append(f"{name}: Zahl der Ergebnisse passt nicht zu den Schritten")
            continue
        form_fehler_beim_laden = any(e.get("fehler") in SCHEMA_CODES or str(e.get("fehler")).startswith("einst.")
                                     for e in erw["schritte"])
        if not form_fehler_beim_laden:
            st = fall["stand"]
            datei_pruefen(f"{name} stand.firma", st["firma"], True)
            for platz, d in list(st.get("gruppen", {}).items()) + list(st.get("nutzer", {}).items()):
                datei_pruefen(f"{name} stand {platz}", d, True)
        for i, (s, e) in enumerate(zip(fall["schritte"], erw["schritte"])):
            schritte_zahl += 1
            wo = f"{name} Schritt {i}"
            if len(s) != 1 or next(iter(s)) not in ("zeige", "aendern"):
                fehler.append(f"{wo}: Schritt hat nicht genau zeige oder aendern")
                continue
            if "zeige" in s:
                if "fehler" in e:
                    if not gueltiger_code(e["fehler"]):
                        fehler.append(f"{wo}: unbekannter Code {e['fehler']}")
                    else:
                        nur_bau += 1
                    continue
                w = e["wirksam"]
                if p_v2 is not None and not p_v2.is_valid(w["einstellung"]):
                    fehler.append(f"{wo}: wirksame Einstellung verletzt Vertrag 2: {next(p_v2.iter_errors(w['einstellung'])).message}")
                for h in w["hinweise"]:
                    if h["code"] not in HINWEISE or h["ebene"] not in ("firma", "gruppe", "nutzer"):
                        fehler.append(f"{wo}: Hinweis falsch {h}")
                for h in w["herkunft"]:
                    if h["ebene"] not in ("firma", "gruppe", "nutzer"):
                        fehler.append(f"{wo}: Herkunft falsch {h}")
                continue
            a = s["aendern"]
            if set(a) != {"wer", "datei"}:
                fehler.append(f"{wo}: aendern hat nicht genau wer und datei")
            z = e["zeile"]
            if set(z) != ZEILE or not HEX64.match(z["neu"]) or (z["alt"] is not None and not HEX64.match(z["alt"])):
                fehler.append(f"{wo}: Zeile hat nicht die Form aus Vertrag 5, Abschnitt 6.6")
            if z["ergebnis"] != ("geaendert" if e["angenommen"] else "abgelehnt") or any(z[k] is not None for k in ("antrag", "pflicht", "zweiter")):
                fehler.append(f"{wo}: ergebnis, antrag, pflicht oder zweiter falsch")
            if not (isinstance(a["wer"], str) and re.match(r"^ps_[0-9a-f]{32}$", a["wer"])) or z["handelnd"] != a["wer"]:
                fehler.append(f"{wo}: wer ist kein Pseudonym oder nicht handelnd")
            if (z["code"] is None) != e["angenommen"] or z["code"] != e["code"]:
                fehler.append(f"{wo}: angenommen, code und Zeile widersprechen sich")
            if z["aktionsname"] != ("regel.geaendert" if e["angenommen"] else "regel.abgelehnt"):
                fehler.append(f"{wo}: aktionsname passt nicht")
            if e["angenommen"]:
                datei_pruefen(wo, a["datei"], True)
            elif not gueltiger_code(e["code"]):
                fehler.append(f"{wo}: unbekannter Code {e['code']}")
            elif e["code"] in SCHEMA_CODES or e["code"].startswith("einst."):
                datei_pruefen(wo, a["datei"], False)
            else:
                datei_pruefen(wo, a["datei"], True)
                nur_bau += 1
    print(f"{zahl} Fälle, {schritte_zahl} Schritte, {len(fehler)} Fehler, "
          f"Schema {'geprüft' if p_ebenen else 'NICHT geprüft (--ohne-schema, jsonschema fehlt)'}; "
          f"{nur_bau} Ablehnungen erst mit dem Bau prüfbar (Bezug, Befugnis, Lockerung, Ladefehler)")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
