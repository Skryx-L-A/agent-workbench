#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags peerkanal: Paarung, JSON, Schema, Codes, Beispielkennungen.

Aufruf: <python mit jsonschema> -I rust/spec/peerkanal/pruefen.py
Geprüft werden zwei Sätze, jeder gegen sein eigenes Schema: Fassung 1 (faelle/, erwartet/, peerkanal.schema.json) und
Fassung 2 (fassung-2/faelle/, fassung-2/erwartet/, fassung-2/peerkanal-2.schema.json). Fassung 1 wird genau wie vor
Fassung 2 geprüft.
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es nur Paarung, strenges JSON-Lesen, Codes und Kennungen, meldet das Schema in der Ausgabe
als nicht geprüft und endet mit Exit 0 nur bei 0 Fehlern.
Exit 0, wenn jeder prüfbare Punkt stimmt. Die Außentests sind erst mit dem Bau prüfbar; sie werden je Ort gezählt.
"""
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent

# Feste Codes je Seite, in Prüfreihenfolge (README, Prüfreihenfolge je Seite; Seite "auskunft": Abschnitt Auskunft ohne
# Verbindung, Nachtrag 25.11.2026). Fassung 2 ändert sie nicht.
CODES = {
    "auskunft": ["pk.vorgabe_ungueltig", "pk.nicht_unterstuetzt", "pk.io"],
    "vorgabe": ["pk.vorgabe_ungueltig"],
    "anlegen": ["pk.vorgabe_ungueltig", "pk.ordner_rechte", "pk.pfad_zu_lang", "pk.name_belegt",
                "pk.anlegen_fehlgeschlagen"],
    "annehmen": ["pk.hallo_ungueltig", "pk.peer_unmessbar", "pk.prozess_gewechselt", "pk.kennung_nicht_erlaubt",
                 "pk.aufrufer_abgewiesen", "pk.io"],
    "verbinden": ["pk.vorgabe_ungueltig", "pk.nicht_erreichbar", "pk.zugriff_verweigert", "pk.server_unmessbar",
                  "pk.falscher_server", "pk.abgewiesen", "pk.io"],
}
NUR_UNIX = {"pk.ordner_rechte", "pk.pfad_zu_lang"}
# Feste Gründe des Ereignisses je Code, in Prüfreihenfolge (README, Abschnitt Annehmen).
GRUENDE = {
    "pk.hallo_ungueltig": ["pk.hallo_fehlt", "pk.hallo_falsch"],
    "pk.peer_unmessbar": ["pk.messung_gescheitert", "pk.prozess_nicht_offen", "pk.job_unbekannt"],
    "pk.prozess_gewechselt": ["pk.prozess_beendet", "pk.startzeit_weicht_ab", "pk.pidversion_weicht_ab"],
    "pk.kennung_nicht_erlaubt": ["pk.eingeschraenkt", "pk.sid_nicht_erlaubt", "pk.appcontainer_falsch", "pk.uid_nicht_erlaubt",
                                 "pk.gid_falsch"],
}
# Fassung 2 (fassung-2/README.md, Stufe 6): zwei neue Gründe, nur unter Linux.
GRUENDE_2 = dict(GRUENDE, **{"pk.kennung_nicht_erlaubt": ["pk.eingeschraenkt", "pk.sid_nicht_erlaubt",
                                                          "pk.appcontainer_falsch", "pk.uid_nicht_erlaubt",
                                                          "pk.gruppen_unlesbar", "pk.gruppe_fehlt", "pk.gid_falsch"]})
NUR_LINUX_2 = {"pk.gruppen_unlesbar", "pk.gruppe_fehlt"}
SAETZE = [
    {"fassung": 1, "ordner": HIER, "schema": HIER / "peerkanal.schema.json", "gruende": GRUENDE},
    {"fassung": 2, "ordner": HIER / "fassung-2", "schema": HIER / "fassung-2" / "peerkanal-2.schema.json",
     "gruende": GRUENDE_2},
]
# Beispielkennungen: jede Unterautorität und jede uid/gid ist eine zehnstellige Zahl, die mit 10000 beginnt; dazu die festen
# Gruppen-SIDs, die nur in Vorgaben stehen dürfen, die abgelehnt werden.
BEISPIEL_ZAHL = re.compile(r"^10000\d{5}$")
GRUPPEN = {"S-1-1-0", "S-1-5-11", "S-1-15-2-1"}
SID = re.compile(r"S-1-[0-9-]+")


def streng(pfad):
    def paare(liste):
        obj = {}
        for k, v in liste:
            if k in obj:
                raise ValueError(f"doppelter Schlüssel {k}")
            obj[k] = v
        return obj
    roh = pfad.read_bytes()
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    return json.loads(roh.decode("utf-8"), object_pairs_hook=paare)


def kennungen(obj):
    """Alle SIDs und Unix-Zahlen in Feldern sid/appcontainer_sid/uid/gid (Fassung 2 auch gruppe, ordner_gruppe,
    gruppen) und in konten."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("sid", "appcontainer_sid") and isinstance(v, str):
                yield v
            elif k in ("uid", "gid", "gruppe", "ordner_gruppe") and isinstance(v, int):
                yield v
            elif k == "gruppen" and isinstance(v, list):
                yield from (g for g in v if isinstance(g, int))
            else:
                yield from kennungen(v)
    elif isinstance(obj, list):
        for v in obj:
            yield from kennungen(v)


def peers(erw):
    if isinstance(erw.get("peer"), dict):
        yield erw["peer"]
    for e in erw.get("ereignisse", []):
        if isinstance(e.get("peer"), dict):
            yield e["peer"]


def beispiel(wert):
    if isinstance(wert, int):
        return bool(BEISPIEL_ZAHL.match(str(wert)))
    if wert in GRUPPEN or wert == "S-1-5-18":
        return True
    teile = wert.split("-")[3:]
    # S-1-5-21-a-b-c-rid: die RID (1001 ff.) ist frei, die übrigen Teile müssen Beispielzahlen sein.
    if wert.startswith("S-1-5-21-"):
        return all(BEISPIEL_ZAHL.match(t) for t in teile[1:4]) and teile[4:] in (["1001"], ["1002"], ["1003"])
    if wert.startswith(("S-1-5-80-", "S-1-15-2-")):
        return all(BEISPIEL_ZAHL.match(t) for t in teile[1:])
    return False


def pruefe_satz(satz, jsonschema):
    if jsonschema is not None:
        schema = json.loads(satz["schema"].read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)

        def pruefer(teil):
            return jsonschema.Draft202012Validator({"$ref": f"#/$defs/{teil}", "$defs": schema["$defs"]})
    else:
        pruefer = None
    gruende, fassung = satz["gruende"], satz["fassung"]
    fehler, ausstehend, orte, zahl = [], [], {}, 0
    faelle = {p.name for p in (satz["ordner"] / "faelle").glob("*.json")}
    erwartet = {p.name for p in (satz["ordner"] / "erwartet").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"{name}: ohne Gegenstück")
    for name in sorted(faelle & erwartet):
        zahl += 1
        try:
            fall = streng(satz["ordner"] / "faelle" / name)
            erw = streng(satz["ordner"] / "erwartet" / name)
        except (ValueError, UnicodeDecodeError) as art:
            fehler.append(f"{name}: JSON ungültig ({art})")
            continue
        if not re.match(r"^[a-z0-9][a-z0-9.-]{0,63}\.json$", name):
            fehler.append(f"{name}: Name nicht in Kennungsform")
        seite, code = erw.get("seite"), erw.get("code")
        if seite not in CODES:
            fehler.append(f"{name}: unbekannte Seite {seite!r}")
            continue
        if code is not None and code not in CODES[seite]:
            fehler.append(f"{name}: Code {code} gehört nicht zur Seite {seite}")
        if code in NUR_UNIX and "windows" in fall.get("ort", []):
            fehler.append(f"{name}: {code} gibt es unter Windows nicht")
        for e in erw.get("ereignisse", []):
            aufrufer = e.get("code") == "pk.aufrufer_abgewiesen" and re.match(
                r"^aufrufer:[a-z0-9][a-z0-9._-]{0,63}$", str(e.get("grund")))
            if not aufrufer and e.get("grund") not in gruende.get(e.get("code"), []):
                fehler.append(f"{name}: Grund {e.get('grund')} gehört nicht zu {e.get('code')}")
            if e.get("code") != code:
                fehler.append(f"{name}: Ereigniscode {e.get('code')} ungleich Code {code}")
            if fassung == 2 and e.get("grund") in NUR_LINUX_2 and fall.get("ort") != ["linux"]:
                fehler.append(f"{name}: Grund {e.get('grund')} gibt es nur unter Linux")
        if (seite == "annehmen" and code not in (None, "pk.io")) != bool(erw.get("ereignisse")):
            fehler.append(f"{name}: abgewiesene Annahme und Ereignis gehören zusammen")
        if fassung == 2:
            # Die Erlaubnisform {uid, gruppe} gilt nur unter Linux; anderswo muss der Fall pk.vorgabe_ungueltig erwarten.
            server = fall.get("server") if isinstance(fall.get("server"), dict) else {}
            mit_gruppe = any(isinstance(x, dict) and "gruppe" in x for x in server.get("erlaubt", []))
            if mit_gruppe and fall.get("ort") != ["linux"] and code != "pk.vorgabe_ungueltig":
                fehler.append(f"{name}: Erlaubnis mit gruppe außerhalb Linux, erwartet pk.vorgabe_ungueltig")
            for p in peers(erw):
                g = p.get("gruppen")
                if isinstance(g, list) and g != sorted(set(g)):
                    fehler.append(f"{name}: peer.gruppen nicht aufsteigend ohne Doppel")
        werte = list(kennungen(fall)) + list(kennungen(erw))
        konten = set(fall.get("konten", {}).values())
        for w in werte:
            if not beispiel(w):
                fehler.append(f"{name}: Kennung {w} ist keine Beispielkennung")
            elif w not in konten:
                fehler.append(f"{name}: Kennung {w} fehlt in konten")
        if pruefer is not None:
            for teil, obj in (("fall", fall), ("erwartet", erw)):
                for f in pruefer(teil).iter_errors(obj):
                    fehler.append(f"{name}: {teil} verletzt das Schema: {f.message}")
            vorgaben = [(t, fall[k]) for t, k in (("server_vorgabe", "server"), ("client_vorgabe", "client"))
                        if fall.get(k) is not None]
            if fall.get("art") == "vorgabe":
                abgelehnt = [t for t, v in vorgaben if not pruefer(t).is_valid(v)]
                if code == "pk.vorgabe_ungueltig" and not abgelehnt:
                    fehler.append(f"{name}: Schema nimmt die Vorgabe an, erwartet pk.vorgabe_ungueltig")
                if code is None and abgelehnt:
                    fehler.append(f"{name}: gültiger Fall, Schema lehnt {abgelehnt} ab")
                if seite != "vorgabe":
                    fehler.append(f"{name}: Vorgabefall mit Seite {seite}")
            else:
                for t, v in vorgaben:
                    for f in pruefer(t).iter_errors(v):
                        fehler.append(f"{name}: Außentest mit ungültiger {t}: {f.message}")
        if fall.get("art") == "aussentest":
            ausstehend.append(name)
            for o in fall.get("ort", []):
                orte[o] = orte.get(o, 0) + 1
    vorsilbe = "" if fassung == 1 else "Fassung 2: "
    print(f"{vorsilbe}{zahl} Fälle, {len(fehler)} Fehler, Schema "
          f"{'geprüft' if pruefer else 'NICHT geprüft (--ohne-schema)'}")
    print(f"  erst mit dem Bau prüfbar: {len(ausstehend)} Außentests "
          f"({', '.join(f'{o} {n}' for o, n in sorted(orte.items()))})")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return len(fehler)


def main():
    try:
        import jsonschema
    except ImportError:
        if "--ohne-schema" not in sys.argv[1:]:
            print("pruefen.py: das Modul jsonschema fehlt, das Schema wäre ungeprüft; Exit 2 "
                  "(mit --ohne-schema bewusst ohne Schema prüfen)", file=sys.stderr)
            return 2
        jsonschema = None
    fehler = sum(pruefe_satz(s, jsonschema) for s in SAETZE)
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
