#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Einstellungsdatei gegen Paarung, JSON-Lesen und Schema.

Aufruf: <python mit jsonschema> -I rust/spec/einstellungen/pruefen.py
Ohne jsonschema prüft es nur Paarung, Größe und JSON-Lesen und meldet das Schema als nicht geprüft.
Exit 0, wenn jeder prüfbare Punkt stimmt. Codes außerhalb des Schemas sind erst mit dem Bau prüfbar.
"""
import base64
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
PRAEFIX = "einst"
GRENZE = 65536
SCHEMA_DATEI = HIER / "einstellungen.schema.json"
# Codes, die das Schema allein erkennt; die übrigen prüft erst die Umsetzung.
SCHEMA_CODES = {
    "ziel_art", "ziel_schema", "ziel_form", "modell_form", "netzziel_lokal", "netzziel_form", "netzziel_port", "zugriff_unbekannt",
    "zugriff_felder", "pfad_form", "verbinden_mit_schluessel", "schluessel_unterdomaene",
    "schluessel_unbekannt", "route_ohne_klasse", "schluessel_fehlt", "typ_falsch", "fassung_unbekannt",
    "anzahl", "kennung_form", "art_unbekannt", "vorgabe_wert", "frist_bereich", "werkzeug_wert",
}
# Fälle, die das Schema nicht trennen kann: JSON Schema hält 1.0 und 1 für gleich; IPv4-Teile über 255 sehen aus wie Namen.
SCHEMA_BLIND = {"typ-fassung-bruch.json", "ziel-form-ipv4-256.json"}


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


def pfad_trifft(muster, pfad):
    """Pfadregel des Vertrags: genau dieser Pfad; `/x/*` trifft alles darunter; `/*` trifft jeden Pfad."""
    if muster.endswith("/*"):
        return pfad.startswith(muster[:-1]) and len(pfad) > len(muster) - 1
    return muster == pfad


def vorfuehrung_pruefen(pruefer, fehler):
    """Die Vorführdatei besteht das Schema und lässt GET /responses (WebSocket-Upgrade, gemessen im echten Lauf) zu."""
    obj = json.loads((HIER / "vorfuehrung.json").read_text(encoding="utf-8"))
    if pruefer is not None and not pruefer.is_valid(obj):
        fehler.append(f"vorfuehrung.json: Schema lehnt ab: {next(pruefer.iter_errors(obj)).message}")
    ziele = [z for r in obj["routen"] if r["schluessel"] == "copilot" for z in r["netzziele"]
             if z["host"] == "api.individual.githubcopilot.com"]
    if not any(q["methode"] == "GET" and pfad_trifft(q["pfad"], "/responses") for z in ziele for q in z["pfade"]):
        fehler.append("vorfuehrung.json: GET /responses an api.individual.githubcopilot.com nicht zugelassen")


def main():
    try:
        import jsonschema
        pruefer = jsonschema.Draft202012Validator(json.loads(SCHEMA_DATEI.read_text(encoding="utf-8")))
        jsonschema.Draft202012Validator.check_schema(pruefer.schema)
    except ImportError:
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
        nimmt_an = pruefer.is_valid(obj)
        if code is None and not nimmt_an:
            fehler.append(f"{name}: gültiger Fall, Schema lehnt ab: {next(pruefer.iter_errors(obj)).message}")
        elif code in SCHEMA_CODES and nimmt_an and name not in SCHEMA_BLIND:
            fehler.append(f"{name}: Schema nimmt an, erwartet {erw['code']}")
        elif code is not None and (code not in SCHEMA_CODES or name in SCHEMA_BLIND):
            nur_bau.append(f"{name} ({erw['code']}, Schema {'nimmt an' if nimmt_an else 'lehnt ab'})")
    vorfuehrung_pruefen(pruefer, fehler)
    print(f"{zahl} Fälle, {len(fehler)} Fehler, Schema {'geprüft' if pruefer else 'NICHT geprüft (jsonschema fehlt)'}")
    for zeile in nur_bau:
        print(f"  erst mit dem Bau prüfbar: {zeile}")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
