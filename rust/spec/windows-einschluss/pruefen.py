#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Einschluss unter Windows.

Aufruf: <python mit jsonschema> -I rust/spec/windows-einschluss/pruefen.py
Geprüft wird: Paarung faelle/erwartet, Pflichtfelder je Form, Größe und JSON-Lesen, das Schema gegen die Fälle
der Formen konfig und steuer, die Helferzeilen gegen eine Referenz der Stufen 2 bis 5 und je Eigenschaft W1 bis
W14 ein Fall und eine Gegenprobe. Erst mit dem Bau prüfbar: Stufen, die Dateisystem, Person, Dienst oder Lage
brauchen, sowie alle Fälle der Formen aufruf und vm. Ohne jsonschema meldet es das Schema als nicht geprüft.
Exit 0, wenn jeder prüfbare Punkt stimmt.
"""
import base64
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
GRENZE = 4096
EIGENSCHAFTEN = [f"W{i}" for i in range(1, 15)]
# Codes, die das Schema allein erkennt; die übrigen prüft erst die Umsetzung.
SCHEMA_CODES = {
    "konfig": {"schluessel_unbekannt", "schluessel_fehlt", "typ_falsch", "fassung_unbekannt", "liste_grenze",
               "pfad_form", "shell_unbekannt"},
    "steuer": {"op_unbekannt", "schluessel_unbekannt", "schluessel_fehlt", "typ_falsch", "form", "grenze"},
}
PRAEFIX = {"konfig": "einschluss.", "steuer": "dienst."}
# Fälle, die das Schema nicht trennen kann (4242.0 gilt dem Schema als Ganzzahl).
SCHEMA_BLIND = {"steuer-typ-pid-bruch.json"}
SID_TEIL = r"(?:0|[1-9][0-9]{0,9})"
SID_RE = re.compile(r"S-1-15-2(?:-" + SID_TEIL + r"){7}")


def streng_lesen(roh):
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    text = roh.decode("utf-8")

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


def helfer_referenz(roh):
    """Stufen 2 bis 5 des Helfers; None heißt: Zeile ist wohlgeformt, Antwort hängt an der Lage."""
    if b"\n" not in roh[:512]:
        return "ABGELEHNT zu_lang" if len(roh) >= 512 else "keine Antwort"
    zeile, _, rest = roh.partition(b"\n")
    if rest:
        return "ABGELEHNT anfrage_form"
    if any(not (0x41 <= b <= 0x5A or 0x30 <= b <= 0x39 or b in b"- ") for b in zeile):
        return "ABGELEHNT anfrage_form"
    for verb in (b"SETZE ", b"ENTFERNE "):
        if zeile.startswith(verb):
            sid = zeile[len(verb):].decode("ascii")
            if not SID_RE.fullmatch(sid) or any(int(t) > 4294967295 for t in sid.split("-")[3:]):
                return "ABGELEHNT sid_form"
            return None
    return "ABGELEHNT anfrage_unbekannt"


def main():
    try:
        import jsonschema
        schema = json.loads((HIER / "einschluss.schema.json").read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        pruefer = {form: jsonschema.Draft202012Validator(dict(schema, **{"$ref": f"#/$defs/{form}"}))
                   for form in ("konfig", "steuer")}
    except ImportError:
        pruefer = None
    fehler, nur_bau, zahl = [], [], 0
    gesehen = {(w, art): [] for w in EIGENSCHAFTEN for art in ("fall", "gegenprobe")}
    faelle = {p.name for p in (HIER / "faelle").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"{name}: ohne Gegenstück")
    for name in sorted(faelle & erwartet):
        zahl += 1
        fall = json.loads((HIER / "faelle" / name).read_text(encoding="utf-8"))
        erw = json.loads((HIER / "erwartet" / name).read_text(encoding="utf-8"))
        form = fall.get("form")
        if not name.startswith(f"{form}-"):
            fehler.append(f"{name}: Name passt nicht zur Form {form}")
        if not fall.get("beschreibung"):
            fehler.append(f"{name}: ohne beschreibung")
        if form == "vm":
            for feld in ("eigenschaft", "art", "ausgangslage", "handlung", "beleg"):
                if not fall.get(feld):
                    fehler.append(f"{name}: vm ohne {feld}")
            if not erw.get("erwartet") or not erw.get("messwerte"):
                fehler.append(f"{name}: vm ohne erwartet oder messwerte")
            if not (fall.get("beleg", "").startswith("gemessen") or fall.get("beleg", "").startswith("nicht gemessen")):
                fehler.append(f"{name}: beleg beginnt nicht mit 'gemessen' oder 'nicht gemessen'")
            key = (fall.get("eigenschaft"), fall.get("art"))
            if key in gesehen:
                gesehen[key].append(name)
            else:
                fehler.append(f"{name}: Eigenschaft oder Art unbekannt {key}")
            nur_bau.append(f"{name} (Außentest VM)")
            continue
        if form == "aufruf":
            if "exit" not in erw and "agent" not in erw:
                fehler.append(f"{name}: Aufruf ohne exit oder agent")
            nur_bau.append(f"{name} (Aufruf)")
            continue
        roh = fall["text"].encode("utf-8") if "text" in fall else base64.b64decode(fall["b64"])
        if form == "helfer":
            if "antwort" not in erw:
                fehler.append(f"{name}: helfer ohne antwort")
                continue
            if "aufrufer" in fall.get("lage", {}):
                nur_bau.append(f"{name} (Aufrufer)")
                continue
            ref = helfer_referenz(roh)
            if ref == "keine Antwort":
                if erw["antwort"] is not None:
                    fehler.append(f"{name}: ohne Zeilenende erwartet keine Antwort, steht {erw['antwort']}")
            elif ref is None:
                if not (str(erw["antwort"]).startswith("OK ") or erw["antwort"] in {"ABGELEHNT ohne_sperre"}):
                    fehler.append(f"{name}: wohlgeformte Zeile, erwartet aber {erw['antwort']}")
                nur_bau.append(f"{name} ({erw['antwort']}, hängt an der Lage)")
            elif ref != erw["antwort"]:
                fehler.append(f"{name}: Referenz {ref}, erwartet {erw['antwort']}")
            continue
        if form not in PRAEFIX:
            fehler.append(f"{name}: Form {form} unbekannt")
            continue
        pre = PRAEFIX[form]
        code = None if erw["gueltig"] else erw["code"]
        if code is not None and not code.startswith(pre):
            fehler.append(f"{name}: Code ohne Präfix {pre}")
            continue
        code = code.removeprefix(pre) if code else None
        if len(roh) > GRENZE:
            if code != "zu_gross":
                fehler.append(f"{name}: {len(roh)} Byte, erwartet aber {erw}")
            continue
        if code == "zu_gross":
            fehler.append(f"{name}: erwartet zu_gross bei {len(roh)} Byte")
            continue
        try:
            obj = streng_lesen(roh)
        except (ValueError, UnicodeDecodeError) as art:
            if code != "json_ungueltig":
                fehler.append(f"{name}: JSON ungültig ({art}), erwartet {erw}")
            continue
        if code == "json_ungueltig":
            fehler.append(f"{name}: JSON lesbar, erwartet aber json_ungueltig")
            continue
        if form == "steuer" and code is None and "lage" not in fall and fall["text"].count("\n") != 1:
            fehler.append(f"{name}: Steuerzeile ohne genau ein LF am Ende")
        if pruefer is None:
            continue
        nimmt_an = pruefer[form].is_valid(obj)
        if code is None and not nimmt_an:
            fehler.append(f"{name}: gültiger Fall, Schema lehnt ab: {next(pruefer[form].iter_errors(obj)).message}")
        elif code in SCHEMA_CODES[form] and nimmt_an and name not in SCHEMA_BLIND:
            fehler.append(f"{name}: Schema nimmt an, erwartet {erw['code']}")
        elif code is not None and (code not in SCHEMA_CODES[form] or name in SCHEMA_BLIND):
            if code not in SCHEMA_CODES[form] and not nimmt_an:
                fehler.append(f"{name}: Schema lehnt ab, obwohl {erw['code']} erst eine spätere Stufe ist")
            nur_bau.append(f"{name} ({erw['code']}, Schema {'nimmt an' if nimmt_an else 'lehnt ab'})")
    for (w, art), namen in gesehen.items():
        if len(namen) != 1:
            fehler.append(f"{w}: {len(namen)} Fälle der Art {art}, erwartet genau 1")
    print(f"{zahl} Fälle, {len(fehler)} Fehler, Schema {'geprüft' if pruefer else 'NICHT geprüft (jsonschema fehlt)'}")
    for zeile in nur_bau:
        print(f"  erst mit dem Bau prüfbar: {zeile}")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
