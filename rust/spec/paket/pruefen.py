#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Paketbau und Prüfbefehl.

Aufruf: <python mit jsonschema> -I rust/spec/paket/pruefen.py
Geprüft wird: Paarung von faelle/ und erwartet/ sowie von faelle-ausstehend/ und erwartet-ausstehend/ (kein Crate liest diese), die Form jedes Falls, der erwartete Code oder die erwarteten Zeilen
gegen den Referenzprüfer (referenz.py) und, für Manifeste, das Schema gegen die Fälle. Ohne jsonschema endet es mit Exit 2
und einer Zeile auf stderr; mit --ohne-schema bleibt das Schema bewusst ungeprüft und die Ausgabe meldet es. Exit 0, wenn jeder prüfbare Punkt stimmt.
Nicht prüfbar vor dem Bau: die Rust-Umsetzung, der Windows-Installer und jeder Außentest auf Windows (README.md).
"""
import base64
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER))
import referenz  # noqa: E402

SCHEMA_DATEI = HIER / "manifest.schema.json"
PAARE = (("faelle", "erwartet"), ("faelle-ausstehend", "erwartet-ausstehend"))
PRAEFIX = {"manifest": "paket.", "liste": "paket.", "stueckliste": "paket.", "steuerdatei": "paket.", "entfernen": "paket.", "remove": "paket.", "einrichten": "einricht.", "profil": "paket.", "bauwerkzeuge": "paket.", "statisch": "paket.", "laufkonfig": "paket.", "dienstkonfig": "paket.", "kerneinheit": "paket.", "systemnutzer": "paket.", "umzug": "paket."}
# Codes, die das Schema allein erkennt; die übrigen prüft erst die Umsetzung (und hier die Referenz).
SCHEMA_CODES = {
    "schluessel_unbekannt", "schluessel_fehlt", "typ_falsch", "fassung_unbekannt", "anzahl", "version_form",
    "wert_unbekannt", "kennung_form", "pfad_form", "pruefsumme_form", "groesse_bereich",
}
# Fälle, die das Schema nicht trennen kann (Zahlenbereich, Segmentregeln, 1.0 gleich 1).
SCHEMA_BLIND = {
    "manifest-typ-fassung-bruch", "manifest-version-bereich-256", "manifest-version-bereich-65536",
    "manifest-pfad-form-punkt-punkt", "manifest-pfad-form-punkt-ende", "manifest-pfad-form-reserviert",
    "manifest-pfad-form-manifest", "manifest-pfad-form-manifest-linux", "manifest-pfad-form-manifest-macos", "manifest-pfad-form-gross",
}
FORMEN = {"manifest", "liste", "stueckliste", "steuerdatei", "entfernen", "remove", "einrichten", "profil", "bauwerkzeuge", "statisch", "laufkonfig", "dienstkonfig", "kerneinheit", "systemnutzer", "umzug", "pruefung"}


def manifest_bytes(fall):
    roh = fall["text"].encode("utf-8") if "text" in fall else base64.b64decode(fall["b64"])
    return roh + b" " * fall.get("auffuellen", 0)


def main():
    try:
        import jsonschema
        pruefer = jsonschema.Draft202012Validator(json.loads(SCHEMA_DATEI.read_text(encoding="utf-8")))
        jsonschema.Draft202012Validator.check_schema(pruefer.schema)
    except ImportError:
        if "--ohne-schema" not in sys.argv[1:]:
            print("pruefen.py: das Modul jsonschema fehlt, das Schema wäre ungeprüft; Exit 2 "
                  "(mit --ohne-schema bewusst ohne Schema prüfen)", file=sys.stderr)
            return 2
        pruefer = None
    fehler, zahl, je_form = [], 0, {}
    # Zwei Ordnerpaare: die gelesenen (faelle/, erwartet/) und die ausstehenden (faelle-ausstehend/, erwartet-ausstehend/, README.md dort).
    # Beide prüft derselbe Referenzprüfer; kein Fallname steht in beiden Paaren.
    eintraege, ausstehend = [], 0
    for f_ordner, e_ordner in PAARE:
        faelle = {p.name for p in (HIER / f_ordner).glob("*.json")}
        erwartet = {p.name for p in (HIER / e_ordner).glob("*.json")}
        for name in sorted(faelle ^ erwartet):
            fehler.append(f"{f_ordner}/{name}: ohne Gegenstück")
        eintraege += [(name, f_ordner, e_ordner) for name in sorted(faelle & erwartet)]
        if f_ordner != "faelle":
            ausstehend += len(faelle & erwartet)
            if not (HIER / f_ordner / "README.md").is_file() or not (HIER / e_ordner / "README.md").is_file():
                fehler.append(f"{f_ordner}, {e_ordner}: die README.md fehlt (welcher Code die Fälle aktiv macht)")
    namen_alle = [n for n, _, _ in eintraege]
    for name in sorted({n for n in namen_alle if namen_alle.count(n) > 1}):
        fehler.append(f"{name}: steht in den gelesenen und den ausstehenden Ordnern")
    for name, f_ordner, e_ordner in eintraege:
        zahl += 1
        stamm = name[:-5]
        fall = json.loads((HIER / f_ordner / name).read_text(encoding="utf-8"))
        erw = json.loads((HIER / e_ordner / name).read_text(encoding="utf-8"))
        form = fall.get("form")
        je_form[form] = je_form.get(form, 0) + 1
        if form not in FORMEN or not stamm.startswith(form + "-") or "beschreibung" not in fall:
            fehler.append(f"{name}: Form oder Name oder Beschreibung falsch")
            continue
        if form == "pruefung":
            rc, strom, zeilen = referenz.pruefbefehl(fall["aufruf"], fall["lage"])
            if (rc, strom, zeilen) != (erw["rc"], erw["strom"], erw["zeilen"]):
                fehler.append(f"{name}: erwartet rc {erw['rc']} {erw['strom']} {erw['zeilen']}, Referenz {rc} {strom} {zeilen}")
            if any(len(z.encode("ascii")) > 200 for z in erw["zeilen"]):
                fehler.append(f"{name}: Zeile über 200 Byte oder nicht ASCII")
            if erw["rc"] not in (0, 1, 20, 64, 70):
                fehler.append(f"{name}: rc {erw['rc']} steht nicht in der Tabelle")
            continue
        if form == "laufkonfig":
            if referenz.laufkonfig(fall) != erw["datei"]:
                fehler.append(f"{name}: erwartet {erw['datei']}, Referenz {referenz.laufkonfig(fall)}")
            if (erw.get("postinst_code") == 19) != (erw["datei"] is None and bool(fall.get("pruefer_verboten"))):
                fehler.append(f"{name}: der Code 19 von postinst passt nicht zu pruefer_verboten und datei")
            if erw["datei"] is not None and (erw["modus"], erw["eigentuemer"]) != ("0644", "root:root"):
                fehler.append(f"{name}: Modus oder Eigentuemer falsch")
            continue
        if form == "dienstkonfig":
            if referenz.dienstkonfig(fall) != erw["dateien"]:
                fehler.append(f"{name}: erwartet {erw['dateien']}, Referenz {referenz.dienstkonfig(fall)}")
            if erw["dateien"] is not None and (erw["modus"], erw["eigentuemer"]) != ("0644", "root:root"):
                fehler.append(f"{name}: Modus oder Eigentuemer falsch")
            if erw.get("journal", []) != referenz.dienst_admin(fall)[2]:
                fehler.append(f"{name}: erwartete Journalzeilen {erw.get('journal', [])}, Referenz {referenz.dienst_admin(fall)[2]}")
            if erw["dateien"] is not None and referenz.dienstkonfig_socket_laenge(fall["uid"]) > referenz.SOCKET_PFAD_MAX:
                fehler.append(f"{name}: Socket-Pfad zu lang")
            if erw["dateien"] is not None:
                lauf = erw["dateien"]["kern"]["lauf"].get("sitzungen")
                if lauf is None or lauf != referenz.laufkonfig({"nutzer": fall["nutzer"], "uid": fall["uid"], "heim": "/srv/beispiel/heim", "harness": {}})["sitzungen"]:
                    fehler.append(f"{name}: kern.json lauf.sitzungen fehlt oder ist nicht der Wert von sitzungen in lauf.json")
                zl = erw["dateien"]["kern"]["lauf"].get("ziellisten")
                if zl is None or zl != referenz.laufkonfig({"nutzer": fall["nutzer"], "uid": fall["uid"], "heim": "/srv/beispiel/heim", "harness": {}})["zustand"] + "/sitzungen":
                    fehler.append(f"{name}: kern.json lauf.ziellisten fehlt oder ist nicht zustand/sitzungen von lauf.json")
                lz = erw["dateien"]["kern"]["lauf"].get("laufzeit")
                if lz is None or lz != referenz.laufkonfig({"nutzer": fall["nutzer"], "uid": fall["uid"], "heim": "/srv/beispiel/heim", "harness": {}})["laufzeit"]:
                    fehler.append(f"{name}: kern.json lauf.laufzeit fehlt oder ist nicht laufzeit von lauf.json")
                if zl == lauf:
                    fehler.append(f"{name}: lauf.ziellisten darf nicht lauf.sitzungen sein")
                run = f"/run/user/{fall['uid']}"
                kl = erw["dateien"]["kern"]["lauf"]
                if kl.get("marker_datei") != f"{run}/werkbank-probelauf/marker.json" or not kl.get("marker_datei", "").startswith(run + "/"):
                    fehler.append(f"{name}: kern.json lauf.marker_datei fehlt oder liegt nicht unter /run/user/<uid> desselben Nutzers")
                if kl.get("marker_ordner") != f"/var/lib/werkbank/z/{fall['uid']}":
                    fehler.append(f"{name}: kern.json lauf.marker_ordner fehlt oder ist nicht /var/lib/werkbank/z/<uid> desselben Nutzers")
            continue
        code = None if erw["gueltig"] else erw["code"].removeprefix(PRAEFIX[form])
        if code is not None and not erw["code"].startswith(PRAEFIX[form]):
            fehler.append(f"{name}: Code ohne Präfix {PRAEFIX[form]}")
            continue
        if form == "manifest":
            roh = manifest_bytes(fall)
            ref = referenz.manifest_code(roh)
        elif form == "liste":
            ref = referenz.liste_code(fall)
        elif form == "steuerdatei":
            ref = referenz.steuer_code(fall)
        elif form == "entfernen":
            ref = referenz.entfernen_code(fall)
            # Gegenprobe der Suche nach Resten: `alte_regel` sagt, wie die ersetzte Regel (`find / -xdev`) den Fall urteilt, und das muss stimmen.
            if "alte_regel" in fall and ref == code:
                alt_gruen = referenz.entfernen_code(fall, suche="wurzel") is None
                if fall["alte_regel"] not in ("gruen", "rot") or alt_gruen != (fall["alte_regel"] == "gruen"):
                    fehler.append(f"{name}: die alte Regel urteilt {'gruen' if alt_gruen else 'rot'}, der Fall sagt {fall['alte_regel']}")
        elif form == "remove":
            ref = referenz.remove_code(fall)
        elif form == "einrichten":
            ref = referenz.einricht_code(fall)
        elif form == "profil":
            ref = referenz.profil_code(fall)
        elif form == "bauwerkzeuge":
            ref = referenz.bauwerkzeuge_code(fall)
        elif form == "statisch":
            ref = referenz.statisch_code(fall)
        elif form == "kerneinheit":
            ref = referenz.kern_einheit_code(fall)
        elif form == "systemnutzer":
            ref = referenz.systemnutzer_code(fall)
        elif form == "umzug":
            ref = referenz.umzug_code(fall)
        else:
            ref = referenz.stueckliste_code(fall)
        if ref != code:
            fehler.append(f"{name}: erwartet {erw}, Referenz {ref}")
            continue
        if form != "manifest" or pruefer is None or code in ("zu_gross", "json_ungueltig"):
            continue
        obj = referenz.streng_lesen(roh)
        nimmt_an = pruefer.is_valid(obj)
        if code is None and not nimmt_an:
            fehler.append(f"{name}: gültiger Fall, Schema lehnt ab: {next(pruefer.iter_errors(obj)).message}")
        elif code in SCHEMA_CODES and stamm not in SCHEMA_BLIND and nimmt_an:
            fehler.append(f"{name}: Schema nimmt an, erwartet {erw['code']}")
        elif code in SCHEMA_CODES and stamm in SCHEMA_BLIND and not nimmt_an:
            fehler.append(f"{name}: als schema-blind geführt, das Schema lehnt aber ab")
        elif code is not None and code not in SCHEMA_CODES and not nimmt_an:
            fehler.append(f"{name}: Code {erw['code']} liegt jenseits des Schemas, das Schema lehnt aber ab")
    # Jeder Code der README-Tabellen kommt in mindestens einem Fall vor.
    gesehen = set()
    for p in [q for _, e_ordner in PAARE for q in (HIER / e_ordner).glob("*.json")]:
        e = json.loads(p.read_text(encoding="utf-8"))
        if "code" in e:
            gesehen.add(e["code"])
        for z in e.get("zeilen", []):
            gesehen.add(z.split(":", 1)[0])
    tabelle = set(codes_der_readme())
    for c in sorted(tabelle - gesehen - NICHT_SICHTBAR):
        fehler.append(f"Code {c} steht in der README, aber in keinem Fall")
    for c in sorted(gesehen - tabelle):
        fehler.append(f"Code {c} kommt in einem Fall vor, aber nicht in der README")
    teile = ", ".join(f"{k} {v}" for k, v in sorted(je_form.items()))
    print(f"{zahl} Fälle ({teile}; davon {ausstehend} ausstehend), {len(fehler)} Fehler, Schema {'geprüft' if pruefer else 'NICHT geprüft (--ohne-schema, jsonschema fehlt)'}")
    print("  erst mit dem Bau prüfbar: Rust-Umsetzung, Windows-Installer und alle Außentests auf Windows (README, Abschnitt Außentests)")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


# Codes der README, die kein sichtbarer Fall erreicht: `pruef.intern` entsteht nur durch einen Fehler im Programm.
NICHT_SICHTBAR = {"pruef.intern"}


def codes_der_readme():
    """Alle Codes `paket.*` und `pruef.*`, die in der README als erstes Codewort einer Tabellenzeile stehen."""
    text = (HIER / "README.md").read_text(encoding="utf-8")
    codes = []
    for zeile in text.split("\n"):
        if zeile.startswith("|"):
            treffer = re.search(r"`((?:paket|pruef|einricht)\.[a-z0-9_]+)`", zeile)
            if treffer:
                codes.append(treffer.group(1))
    return codes


if __name__ == "__main__":
    sys.exit(main())
