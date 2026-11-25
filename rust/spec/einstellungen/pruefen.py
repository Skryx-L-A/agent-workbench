#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Einstellungsdatei gegen Paarung, JSON-Lesen und Schema.

Aufruf: <python mit jsonschema> -I rust/spec/einstellungen/pruefen.py
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es nur Paarung, Größe und JSON-Lesen und meldet das Schema in der Ausgabe als nicht geprüft.
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


# Gemessene Pfade am Wert von endpoints.api (Copilot CLI 1.0.88): GET /models (Vertrag 8), GET /responses (zweiter
# angemeldeter Start, WebSocket), die fünf POST-Pfade (Messung zum Token der CLI vom 20.11.2026, Vertrag 2).
GEMESSEN_MODELL_HOST = [("GET", "/models"), ("GET", "/responses"), ("POST", "/auto"), ("POST", "/models/session"),
                        ("POST", "/models/session/intent"), ("POST", "/chat/completions"), ("POST", "/agents/sessions")]
# Pfade außerhalb der Liste (Gegenprobe zu N1 und F1): das abgeleitete Token an api.github.com, ein beliebiger Pfad und ein
# Unterpfad am Modell-Host, eine falsche Methode.
AUSSERHALB = [("api.github.com", "GET", "/copilot_internal/v2/token"),
              ("api.individual.githubcopilot.com", "GET", "/copilot_internal/v2/token"),
              ("api.individual.githubcopilot.com", "POST", "/v1/fremd"),
              ("api.individual.githubcopilot.com", "GET", "/models/x"),
              ("api.individual.githubcopilot.com", "POST", "/models"),
              ("api.individual.githubcopilot.com", "GET", "/chat/completions")]


def vorfuehrung_pruefen(pruefer, fehler):
    """Die Vorführdatei besteht das Schema, lässt die gemessenen Pfade am Modell-Host zu (auch GET /responses, das
    WebSocket-Upgrade) und hat an Zielen mit Schlüssel nur genaue Pfade (N1); Pfade außerhalb der Liste sind abgelehnt."""
    obj = json.loads((HIER / "vorfuehrung.json").read_text(encoding="utf-8"))
    if pruefer is not None and not pruefer.is_valid(obj):
        fehler.append(f"vorfuehrung.json: Schema lehnt ab: {next(pruefer.iter_errors(obj)).message}")
    ziele = [z for r in obj["routen"] if r["schluessel"] == "copilot" for z in r["netzziele"]
             if z["host"] == "api.individual.githubcopilot.com"]
    if not any(q["methode"] == "GET" and pfad_trifft(q["pfad"], "/responses") for z in ziele for q in z["pfade"]):
        fehler.append("vorfuehrung.json: GET /responses an api.individual.githubcopilot.com nicht zugelassen")
    # N1 (Entscheid der Leitung, 25.11.2026): an einem Ziel mit Schlüssel nur genaue Pfade, kein Teilbaum; ein Pfad, der
    # ein Token ausgibt, steht nie in einer Regel (Halter-Vertrag, Abschnitt 4)
    gebunden = [z for r in obj["routen"] for z in r["netzziele"] if z.get("schluessel")]
    for z in gebunden:
        for q in z["pfade"]:
            if q["pfad"].endswith("*"):
                fehler.append(f"vorfuehrung.json: Teilbaum {q['methode']} {q['pfad']} an {z['host']} mit Schlüssel")

    def zugelassen(host, methode, pfad):
        return any(q["methode"] == methode and pfad_trifft(q["pfad"], pfad)
                   for z in gebunden if z["host"] == host for q in z["pfade"])

    modell = "api.individual.githubcopilot.com"
    # Fall: die gemessenen Pfade gehen durch
    for methode, pfad in GEMESSEN_MODELL_HOST:
        if not zugelassen(modell, methode, pfad):
            fehler.append(f"vorfuehrung.json: gemessener Pfad {methode} {pfad} an {modell} nicht zugelassen")
    # Gegenprobe: ein Pfad außerhalb der Liste ist abgelehnt, auch ein Unterpfad eines genauen Pfads
    for host, methode, pfad in AUSSERHALB:
        if zugelassen(host, methode, pfad):
            fehler.append(f"vorfuehrung.json: {methode} {pfad} an {host} zugelassen, steht aber in keiner Regel")


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
    print(f"{zahl} Fälle, {len(fehler)} Fehler, Schema {'geprüft' if pruefer else 'NICHT geprüft (--ohne-schema, jsonschema fehlt)'}")
    for zeile in nur_bau:
        print(f"  erst mit dem Bau prüfbar: {zeile}")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
