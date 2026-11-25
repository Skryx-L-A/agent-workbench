#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Halter-Vertrags.

Aufruf: <python mit jsonschema> -I rust/spec/halter/pruefen.py
Ohne jsonschema prüft es alles außer dem Schema und meldet das Schema als nicht geprüft (Exit 0 nur mit Schema).

Geprüft wird: Paarung faelle/erwartet, Größe, strenges JSON-Lesen, RFC 8785 der Bytes, die Rollentabelle,
Stufe 7a gegen den offenen Strom der Lage, das Schema (Stufen 8 bis 12 als Gruppe) und die Stufen 13 bis 27 gegen
die Lage des Falls (mit Pfadprüfung und Teilbaum-Regeln aus dem Nachtrag B05), einschließlich
Ed25519-Signatur, digest, Frist und Einmal-Kennung des Scheins. Fälle der Form `konfig` (Abschnitt 2.3a, Nachtrag Rolle
aus der Gruppe): strenges Lesen, Größe, `$defs/konfig`, eigene uid in `konten` und die Regeln der Rollengruppen gegen
`/etc/group` und `/etc/passwd` der `umgebung`. Welcher Code innerhalb der Schema-Stufen
fällt, die Ausführung (Stufen 28 bis 31 mit 28a) und alles an Pipe, Socket und Strom sind erst mit dem Bau prüfbar.
"""
import base64
import binascii
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HIER))
import werkzeug as w  # noqa: E402

GRENZE = 33554432
SCHEMA_DATEI = HIER / "halter.schema.json"
ROLLEN = {
    "hub": {"schluessel.vorhanden", "mcp.aufruf"},
    "bruecke": {"schluessel.vorhanden", "modell.aufruf", "modell.strom", "strom.teil", "strom.ende", "modell.suche"},
    "oberflaeche": {"schluessel.vorhanden", "schluessel.setzen", "schluessel.loeschen"},
}
OPS = set().union(*ROLLEN.values())
SCHEMA_CODES = {"halter.schluessel_unbekannt", "halter.schluessel_fehlt", "halter.typ_falsch", "halter.wert_form",
                "halter.kopf_verboten"}
SCHEIN_SPANNE = 86400000
SCHEIN_VORLAUF = 5000


def rohbytes(fall):
    if "erzeugt" in fall:
        e = fall["erzeugt"]
        vorne, hinten = e["vorne"].encode(), e["hinten"].encode()
        return vorne + e["fuellbyte"].encode() * (e["laenge"] - len(vorne) - len(hinten)) + hinten
    if "text" in fall:
        return fall["text"].encode("utf-8")
    return base64.b64decode(fall["b64"])


def schein_stufen(anfrage, lage, eintrag):
    """Stufen 19 bis 26. Gibt den Code oder None zurück."""
    schein = anfrage["schein"]
    daten = schein["daten"]
    oeff = lage["kids"].get(daten["kid"])
    if oeff is None:
        return "halter.schein_kid"
    try:
        sig = base64.b64decode(schein["signatur"], validate=True)
    except (binascii.Error, ValueError):
        return "halter.schein_signatur"
    if not w.pruefen_signatur(base64.b64decode(oeff), w.kanonisch_bytes(daten), sig):
        return "halter.schein_signatur"
    if daten["art"] != "mcp-schein" or daten["fassung"] != 1:
        return "halter.schein_art"
    if daten["lauf"] != anfrage["lauf"]:
        return "halter.schein_lauf"
    eigen = w.digest(anfrage["server"], anfrage["werkzeug"], anfrage["argumente"], anfrage["person"],
                     anfrage["vorschau_sha256"])
    if daten["digest"] != eigen:
        return "halter.schein_digest"
    jetzt = lage["jetzt_ms"]
    spanne = daten["ablauf"] - daten["ausgestellt"]
    if not 1 <= spanne <= SCHEIN_SPANNE or daten["ausgestellt"] > jetzt + SCHEIN_VORLAUF:
        return "halter.schein_zeit"
    if daten["ausgestellt"] < lage["start_ms"] or jetzt >= daten["ablauf"]:
        return "halter.schein_abgelaufen"
    if daten["einmal"] in lage["benutzt"]:
        return "halter.schein_benutzt"
    return None


def geheimnis(lage, name, konto):
    for g in lage["geheimnisse"]:
        if g["name"] == name and g["konto"] == konto:
            return g
    return None


def pfad_unsicher(pfad):
    """Pfadprüfung vor Stufe 15 (Nachtrag B05): //, Segment . oder .., kodierte Punkte und Trenner."""
    klein = pfad.lower()
    if "//" in pfad or "\\" in pfad or any(k in klein for k in ("%2e", "%2f", "%5c")):
        return True
    return any(seg in (".", "..") for seg in pfad.split("/")[1:])


def regel_passt(regel, methode, pfad):
    """Regel `<METHODE> <pfad>`: `/*` passt immer, `/x/*` deckt den Teilbaum `/x/...`, sonst genau."""
    r_methode, r_pfad = regel.split(" ", 1)
    if r_methode != methode:
        return False
    if r_pfad == "/*":
        return True
    if r_pfad.endswith("/*"):
        return pfad.startswith(r_pfad[:-1])
    return pfad == r_pfad


def modell_geheimnis(lage, name, konto):
    """modell.* liegt im Bereich maschine (ohne Konto), anmeldung.* gehört dem Konto des Aufrufers (Stufen 16, 17)."""
    return geheimnis(lage, name, None if name.startswith("modell.") else konto)


def nach_schema(a, lage):
    """Stufen 13 bis 27 für eine schemagültige Anfrage."""
    op, konfig, konto = a["op"], lage["konfig"], lage["aufrufer"]["konto"]
    if op in ("schluessel.setzen", "schluessel.loeschen"):
        if a["name"].startswith("modell."):
            return "halter.bereich"
        return None if lage["nachweis_gueltig"] else "halter.nachweis"
    if op == "schluessel.vorhanden":
        # nach ziel (Nachtrag 25.11.2026): das Ziel muss in halter.json stehen; ohne Geheimnis ist die Antwort false
        if "ziel" in a and a["ziel"] not in konfig["ziele"]:
            return "halter.ziel_unbekannt"
        return None
    if op in ("strom.teil", "strom.ende"):
        return None
    if op in ("modell.aufruf", "modell.strom", "modell.suche"):
        ziel = konfig["ziele"].get(a["ziel"])
        if ziel is None:
            return "halter.ziel_unbekannt"
        if op != "modell.suche":
            methode = a["methode"] if op == "modell.aufruf" else "GET"
            if pfad_unsicher(a["pfad"]) or not any(regel_passt(r, methode, a["pfad"]) for r in ziel["pfade"]):
                return "halter.pfad_verboten"
        return None if modell_geheimnis(lage, ziel["schluessel"], konto) else "halter.kein_schluessel"
    server = konfig["server"].get(a["server"])
    if server is None:
        return "halter.ziel_unbekannt"
    eintrag = None
    for g in lage["geheimnisse"]:
        if g["name"] == server["schluessel"] and g["person"] == a["person"]:
            eintrag = g
    if eintrag is None:
        return "halter.kein_schluessel"
    if eintrag["konto"] != konto and not lage["aufrufer"].get("vertreter", False):
        return "halter.person"
    braucht = server["werkzeuge"].get(a["werkzeug"], "schreibend") == "schreibend" and server["freigabe"]
    if braucht and "schein" not in a:
        return "halter.schein_fehlt"
    if not braucht and "schein" in a:
        return "halter.schein_unerwartet"
    return schein_stufen(a, lage, eintrag) if braucht else None


KONFIG_GRENZE = 1048576
GRUPPE_PASSWORT_OK = ("x", "!", "*", "")


def felder(text, n):
    """Zeilen einer Datei im Format von /etc/group oder /etc/passwd mit genau n Feldern; andere Zeilen zählen nicht."""
    return [z.split(":") for z in text.split("\n") if z and len(z.split(":")) == n]


def rollengruppen(k, umgebung):
    """Abschnitt 2.3a, Rollengruppen beim Start: None, wenn alle Regeln halten, sonst der Text der verletzten Regel."""
    gruppen = felder(umgebung["group"], 4)
    hauptgruppen = {}
    for f in felder(umgebung["passwd"], 7):
        if f[2].isdigit() and f[3].isdigit():
            hauptgruppen[int(f[2])] = int(f[3])
    konten = {int(u) for u in k["konten"]}
    gids = set()
    for rolle, name in k["rollen"].items():
        treffer = [f for f in gruppen if f[0] == name]
        if len(treffer) != 1:
            return f"{rolle}: Gruppe nicht genau einmal"
        _, passwort, gid, mitglieder = treffer[0]
        if not gid.isdigit() or not gid.isascii() or int(gid) == 0 or int(gid) > 4294967295:
            return f"{rolle}: gid"
        gid = int(gid)
        if gid in gids:
            return f"{rolle}: gid doppelt"
        gids.add(gid)
        if mitglieder != "":
            return f"{rolle}: Mitglied"
        if passwort not in GRUPPE_PASSWORT_OK:
            return f"{rolle}: Passwort"
        if any(hauptgruppen.get(u) == gid for u in konten):
            return f"{rolle}: Hauptgruppe eines Kontos"
    return None


def konfig_referenz(fall, roh, pruefer):
    """Fall der Form `konfig`: None für gültig, 'halter.konfig' für Exit 78, 'BAU' ohne jsonschema."""
    if len(roh) > KONFIG_GRENZE:
        return "halter.konfig"
    try:
        k = w.streng_lesen(roh)
    except (ValueError, UnicodeDecodeError):
        return "halter.konfig"
    if pruefer is None:
        return "BAU"
    if not pruefer["konfig"].is_valid(k):
        return "halter.konfig"
    if len(set(k["rollen"].values())) != len(k["rollen"]):
        return "halter.konfig"
    if str(fall["umgebung"]["uid"]) not in k["konten"]:
        return "halter.konfig"
    if rollengruppen(k, fall["umgebung"]) is not None:
        return "halter.konfig"
    return None


def referenz(fall, roh, pruefer):
    """Code nach der Prüfreihenfolge, 'SCHEMA' für die Stufen 8 bis 12, 'BAU' wenn nicht prüfbar, None für gültig."""
    if fall["form"] == "konfig":
        return konfig_referenz(fall, roh, pruefer)
    if len(roh) > GRENZE:
        return "halter.zu_gross"
    lage = fall.get("lage")
    if fall["form"] == "anfrage" and lage["aufrufer"]["rolle"] is None:
        return "halter.peer"
    try:
        obj = w.streng_lesen(roh)
    except (ValueError, UnicodeDecodeError):
        return "halter.json_ungueltig" if fall["form"] == "anfrage" else "antwort.form"
    try:
        if w.kanonisch_bytes(obj) != roh:
            return "halter.nicht_kanonisch" if fall["form"] == "anfrage" else "antwort.form"
    except w.NichtKanonisierbar:
        return "BAU"
    if fall["form"] == "antwort":
        if pruefer is None:
            return "BAU"
        return None if pruefer["antwort"].is_valid(obj) else "antwort.form"
    f = obj.get("fassung")
    if type(f) is not int or f != 1:
        return "halter.fassung_unbekannt"
    if not isinstance(obj.get("op"), str) or obj["op"] not in OPS:
        return "halter.op_unbekannt"
    if obj["op"] not in ROLLEN[lage["aufrufer"]["rolle"]]:
        return "halter.rolle"
    if obj["op"] == "schluessel.vorhanden" and "ziel" in obj and lage["aufrufer"]["rolle"] != "bruecke":
        return "halter.rolle"
    offen = lage.get("strom")
    if obj["op"] in ("strom.teil", "strom.ende"):
        if offen is None or obj.get("id") != offen:
            return "halter.strom"
    elif offen is not None:
        return "halter.strom"
    if pruefer is None:
        return "BAU"
    if not pruefer["anfrage"].is_valid(obj):
        return "SCHEMA"
    return nach_schema(obj, lage)


def main():
    w.selbsttest()
    try:
        import jsonschema
        schema = json.loads(SCHEMA_DATEI.read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)
        pruefer = {form: jsonschema.Draft202012Validator(dict(schema, **{"$ref": f"#/$defs/{form}"}))
                   for form in ("anfrage", "antwort", "konfig")}
    except ImportError:
        pruefer = None
    fehler, nur_bau, zahl = [], [], 0
    faelle = {p.name for p in (HIER / "faelle").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"{name}: ohne Gegenstück")
    codes = {}
    for name in sorted(faelle & erwartet):
        zahl += 1
        fall = json.loads((HIER / "faelle" / name).read_text(encoding="utf-8"))
        erw = json.loads((HIER / "erwartet" / name).read_text(encoding="utf-8"))
        soll = None if erw["gueltig"] else erw["code"]
        if soll is not None:
            codes[soll] = codes.get(soll, 0) + 1
        ist = referenz(fall, rohbytes(fall), pruefer)
        if ist == "BAU":
            nur_bau.append(f"{name} ({soll})")
        elif ist == "SCHEMA":
            if soll not in SCHEMA_CODES:
                fehler.append(f"{name}: Schema lehnt ab, erwartet {soll}")
            else:
                nur_bau.append(f"{name}: welcher Schema-Code ({soll})")
        elif ist != soll:
            fehler.append(f"{name}: Referenz {ist}, erwartet {soll}")
    print(f"{zahl} Fälle, {len(fehler)} Fehler, {len(codes)} Codes, Schema "
          f"{'geprüft' if pruefer else 'NICHT geprüft (jsonschema fehlt)'}")
    for zeile in nur_bau:
        print(f"  erst mit dem Bau prüfbar: {zeile}")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler or pruefer is None else 0


if __name__ == "__main__":
    sys.exit(main())
