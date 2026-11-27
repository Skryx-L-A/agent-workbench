#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Start-Struktur gegen Paarung, JSON-Lesen und Schema.

Aufruf: <python mit jsonschema> -I rust/spec/start/pruefen.py
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es nur Paarung, Größe und JSON-Lesen und meldet das Schema in der Ausgabe als nicht geprüft.
Exit 0, wenn jeder prüfbare Punkt stimmt. Codes außerhalb des Schemas sind erst mit dem Bau prüfbar.
"""
import base64
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
PRAEFIX = "start"
GRENZE = 4096
SCHEMA_DATEI = HIER / "start.schema.json"
# Codes, die das Schema allein erkennt; die übrigen prüft erst die Umsetzung.
SCHEMA_CODES = {
    "sitzung_nicht_erlaubt", "marker_nicht_erlaubt", "marker_form", "schluessel_unbekannt", "schluessel_fehlt", "typ_falsch", "fassung_unbekannt",
    "kennung_form", "modell_form", "ordner_ungueltig",
}
# Fälle, die das Schema nicht trennen kann: maxLength zählt Zeichen, die Grenze zählt Byte.
SCHEMA_BLIND = set()


TIEFE = 127


def tiefe_pruefen(text):
    """Mehr als 127 Ebenen sind ungültig; Klammern in Texten zählen nicht. Läuft vor json.loads (Stackschutz)."""
    tiefe, im_text, maskiert = 0, False, False
    for zeichen in text:
        if im_text:
            if maskiert:
                maskiert = False
            elif zeichen == "\\":
                maskiert = True
            elif zeichen == '"':
                im_text = False
        elif zeichen == '"':
            im_text = True
        elif zeichen in "[{":
            tiefe += 1
            if tiefe > TIEFE:
                raise ValueError("Verschachtelung über 127 Ebenen")
        elif zeichen in "]}":
            tiefe -= 1


def streng_lesen(roh):
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    text = roh.decode("utf-8")
    tiefe_pruefen(text)

    def paare(liste):
        obj = {}
        for k, v in liste:
            if k in obj:
                raise ValueError("doppelter Schlüssel")
            obj[k] = v
        return obj

    obj = json.loads(text, object_pairs_hook=paare)
    if not isinstance(obj, dict):
        raise ValueError("kein Objekt")
    return obj


def main():
    try:
        import jsonschema
        schema = json.loads(SCHEMA_DATEI.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        pruefer = {form: jsonschema.Draft202012Validator(dict(schema, **{"$ref": f"#/$defs/{form}"}))
                   for form in ("anfrage", "datei")}
    except ImportError:
        if "--ohne-schema" not in sys.argv[1:]:
            print("pruefen.py: das Modul jsonschema fehlt, das Schema wäre ungeprüft; Exit 2 "
                  "(mit --ohne-schema bewusst ohne Schema prüfen)", file=sys.stderr)
            return 2
        pruefer = None
    fehler, nur_bau, zahl = [], [], 0
    faelle = {p.name for p in (HIER / "faelle").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"{name}: ohne Gegenstück")
    for name in sorted(faelle & erwartet):
        zahl += 1
        fall = json.loads((HIER / "faelle" / name).read_text(encoding="utf-8"))
        erw = json.loads((HIER / "erwartet" / name).read_text(encoding="utf-8"))
        if fall["form"] == "aufruf":
            if "exit" not in erw and "exec" not in erw:
                fehler.append(f"{name}: Aufruf ohne exit oder exec")
            nur_bau.append(f"{name} (Aufruf)")
            continue
        roh = fall["text"].encode("utf-8") if "text" in fall else base64.b64decode(fall["b64"])
        code = None if erw["gueltig"] else erw["code"].removeprefix(PRAEFIX + ".")
        if code is not None and not erw["code"].startswith(PRAEFIX + "."):
            fehler.append(f"{name}: Code ohne Präfix {PRAEFIX}.")
        if len(roh) > GRENZE:
            if code != "zu_gross":
                fehler.append(f"{name}: {len(roh)} Byte, erwartet aber {erw}")
            continue
        if code == "zu_gross":
            fehler.append(f"{name}: erwartet zu_gross bei {len(roh)} Byte")
            continue
        try:
            obj = streng_lesen(roh)
        except (ValueError, UnicodeDecodeError) as fehlerart:
            if code != "json_ungueltig":
                fehler.append(f"{name}: JSON ungültig ({fehlerart}), erwartet {erw}")
            continue
        if code == "json_ungueltig":
            fehler.append(f"{name}: JSON lesbar, erwartet aber json_ungueltig")
            continue
        if pruefer is None:
            continue
        nimmt_an = pruefer[fall["form"]].is_valid(obj)
        if code is None and not nimmt_an:
            fehler.append(f"{name}: gültiger Fall, Schema lehnt ab: {next(pruefer[fall['form']].iter_errors(obj)).message}")
        elif code in SCHEMA_CODES and nimmt_an and name not in SCHEMA_BLIND:
            fehler.append(f"{name}: Schema nimmt an, erwartet {erw['code']}")
        elif code is not None and (code not in SCHEMA_CODES or name in SCHEMA_BLIND):
            nur_bau.append(f"{name} ({erw['code']}, Schema {'nimmt an' if nimmt_an else 'lehnt ab'})")
    # Ausstehende Fälle (Nachtrag 26.11.2026, Fassung 2 mit effort und name): Paarung, Größe, JSON-Lesen, Präfix.
    # Das Schema der Fassung 1 kennt die Felder nicht, und kein Crate liest den Ordner, bis die Regel gebaut ist.
    aus_f = {p.name for p in (HIER / "faelle-ausstehend").glob("*.json")}
    aus_e = {p.name for p in (HIER / "erwartet-ausstehend").glob("*.json")}
    for name in sorted(aus_f ^ aus_e):
        fehler.append(f"ausstehend {name}: ohne Gegenstück")
    for name in sorted(aus_f & aus_e):
        fall = json.loads((HIER / "faelle-ausstehend" / name).read_text(encoding="utf-8"))
        erw = json.loads((HIER / "erwartet-ausstehend" / name).read_text(encoding="utf-8"))
        roh = fall["text"].encode("utf-8")
        if len(roh) > GRENZE:
            fehler.append(f"ausstehend {name}: über {GRENZE} Byte")
        try:
            streng_lesen(roh)
        except (ValueError, UnicodeDecodeError) as fehlerart:
            fehler.append(f"ausstehend {name}: JSON ungültig ({fehlerart})")
        if not erw["gueltig"] and not erw["code"].startswith(PRAEFIX + "."):
            fehler.append(f"ausstehend {name}: Code ohne Präfix {PRAEFIX}.")
        if (HIER / "faelle" / name).exists():
            fehler.append(f"ausstehend {name}: steht auch in faelle/")
    print(f"{len(aus_f & aus_e)} ausstehende Fälle (Fassung 2 mit effort und name, erst mit dem Bau prüfbar)")
    print(f"{zahl} Fälle, {len(fehler)} Fehler, Schema {'geprüft' if pruefer else 'NICHT geprüft (--ohne-schema, jsonschema fehlt)'}")
    for zeile in nur_bau:
        print(f"  erst mit dem Bau prüfbar: {zeile}")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
