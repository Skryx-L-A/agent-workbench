#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Regelebenen gegen Paarung, Codes und Schema.

Aufruf: <python mit jsonschema> -I rust/spec/regelebenen/pruefen.py
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es nur Paarung, strenges JSON-Lesen, Codes und Zeilenform, meldet das Schema in der Ausgabe
als nicht geprüft und endet mit Exit 0 nur bei 0 Fehlern.
Exit 0, wenn jeder prüfbare Punkt stimmt. Querverweise, Lockerung und Befugnis sind erst mit dem Bau prüfbar.
"""
import hashlib
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


# Fassung 2 (Nachtrag 26.11.2026, README Abschnitt 15): setzen() und sicht() in faelle-setzen/ und erwartet-setzen/.
CODES_SETZEN = {"ebenen.einstellung_unbekannt", "ebenen.bereich_falsch", "ebenen.ebene_unbekannt", "ebenen.gruppe_unbekannt",
                "ebenen.ebene_gesperrt", "ebenen.nicht_befugt", "ebenen.wert_ungueltig", "ebenen.lockerung",
                "ebenen.ausser_grenze"}
SICHT = {"wert", "herkunft", "aenderbar", "grund"}
HERKUNFT = re.compile(r"^(firma|nutzer|gruppe [a-z0-9][a-z0-9._-]{0,63})$")
STAND_LAUF = {"klassen", "admins", "gruppen", "werte"}
PSEUDONYM = re.compile(r"^ps_[0-9a-f]{32}$")


def nur_pseudonyme(liste):
    """S10 für leseklassen: bei ebene nutzer steht nur ein Pseudonym, nie ein Konto (Abschnitt 15.11)."""
    return isinstance(liste, list) and all(not (isinstance(x, dict) and x.get("ebene") == "nutzer")
                                           or (isinstance(x.get("wer"), str) and PSEUDONYM.match(x["wer"])) for x in liste)


def pruefen_setzen(fehler):
    faelle = {p.name for p in (HIER / "faelle-setzen").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet-setzen").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"setzen {name}: ohne Gegenstück")
    zahl, codes = 0, set()
    for name in sorted(faelle & erwartet):
        zahl += 1
        try:
            fall = streng((HIER / "faelle-setzen" / name).read_text(encoding="utf-8"))
            erw = streng((HIER / "erwartet-setzen" / name).read_text(encoding="utf-8"))
        except ValueError as e:
            fehler.append(f"setzen {name}: {e}")
            continue
        if "konto-" in (HIER / "erwartet-setzen" / name).read_text(encoding="utf-8"):
            fehler.append(f"setzen {name}: ein Ergebnis nennt ein Konto (S10)")
        if set(fall) != FALL_FELDER or fall["name"] + ".json" != name or set(fall["stand"]) != STAND_LAUF:
            fehler.append(f"setzen {name}: Felder des Falls, des Stands oder Name falsch")
            continue
        if any(not re.match(r"^ps_[0-9a-f]{32}$", a) for a in fall["stand"]["admins"]):
            fehler.append(f"setzen {name}: admins enthält etwas anderes als Pseudonyme")
        if len(erw["schritte"]) != len(fall["schritte"]):
            fehler.append(f"setzen {name}: Zahl der Ergebnisse passt nicht")
            continue
        for i, (s, e) in enumerate(zip(fall["schritte"], erw["schritte"])):
            wo = f"setzen {name} Schritt {i}"
            if len(s) != 1 or next(iter(s)) not in ("setzen", "sicht"):
                fehler.append(f"{wo}: Schritt hat nicht genau setzen oder sicht")
                continue
            if "sicht" in s:
                for k, v in e["einstellungen"].items():
                    if set(v) != SICHT or not HERKUNFT.match(v["herkunft"]) or not isinstance(v["aenderbar"], bool) \
                            or v["grund"] not in (None, "nur_admin", "nur_strenger", "fest") \
                            or (v["grund"] in ("nur_admin", "fest")) == v["aenderbar"]:
                        fehler.append(f"{wo}: Sicht von {k} hat nicht die Form aus Abschnitt 15.5")
                if "leseklassen" in e["einstellungen"] and not nur_pseudonyme(e["einstellungen"]["leseklassen"]["wert"]):
                    fehler.append(f"{wo}: Sicht von leseklassen nennt etwas anderes als ein Pseudonym (S10)")
                continue
            a = s["setzen"]
            if set(a) != {"wer", "bereich", "ebene", "schluessel", "wert"}:
                fehler.append(f"{wo}: setzen hat nicht genau wer, bereich, ebene, schluessel, wert")
            z = e["zeile"]
            if set(z) != ZEILE or not HEX64.match(z["neu"]) or (z["alt"] is not None and not HEX64.match(z["alt"])):
                fehler.append(f"{wo}: Zeile hat nicht die Form aus Vertrag 5, Abschnitt 6.6")
            if z["handelnd"] != a["wer"] or z["code"] != e["code"] or (e["code"] is None) != e["angenommen"]:
                fehler.append(f"{wo}: handelnd, code und angenommen widersprechen sich")
            if a.get("schluessel") == "leseklassen" and e["angenommen"] and not nur_pseudonyme(a["wert"]):
                fehler.append(f"{wo}: angenommene leseklassen mit Konto statt Pseudonym (S10)")
            if e["code"] is not None:
                if e["code"] not in CODES_SETZEN:
                    fehler.append(f"{wo}: unbekannter Code {e['code']}")
                codes.add(e["code"])
    for c in sorted(CODES_SETZEN - codes):
        fehler.append(f"setzen: kein Fall für den Code {c}")
    return zahl

# Schreibweg der Firmenvorgabe und Konto in der Ansicht der Admins (README Abschnitte 15.12 und 15.13).
HELFER = "werkbank-vorgabe"
CODES_HELFER = {"firmenvorgabe.partner_unbekannt", "firmenvorgabe.partner_eingeschlossen", "firmenvorgabe.datei_unsicher",
                "firmenvorgabe.ladefehler", "firmenvorgabe.partner_nicht_admin", "firmenvorgabe.anfrage_zu_gross",
                "firmenvorgabe.anfrage_frist", "firmenvorgabe.anfrage_ungueltig", "firmenvorgabe.veraltet",
                "firmenvorgabe.ausser_leseklassen", "firmenvorgabe.temp_fehlgeschlagen", "firmenvorgabe.fsync_fehlgeschlagen",
                "firmenvorgabe.rename_fehlgeschlagen", "firmenvorgabe.ordner_fsync_fehlgeschlagen", "firmenvorgabe.zustand_unklar"}
CODES_SCHREIBEN = CODES_HELFER | {"firmenvorgabe.helfer_nicht_erreichbar"}
FALL_SCHREIBEN = {"name", "anforderungen", "beschreibung", "firma", "konten", "zuordnung", "firmenvorgabe", "ordner", "eltern",
                  "schritte"}
FEHLER_HELFER = {"eingeschlossen", "zu_gross", "frist", "temp", "fsync", "rename", "ordner_fsync", "rueckweg"}
FEHLER_EIN = FEHLER_HELFER | {"fremd_geaendert", "helfer_weg", "antwort_fehlt"}
DATEI_ERG = {"eigentuemer", "modus", "sha256", "inhalt"}
SCHRITT_ARTEN = ("schreiben", "anfrage", "nachricht", "neuladen")


def konten_in(obj, konten, pfad=""):
    """Alle Stellen in obj, an denen ein Konto aus der Zuordnung steht, als Pfade."""
    if isinstance(obj, dict):
        return [p for k, v in obj.items() for p in konten_in(v, konten, f"{pfad}/{k}")]
    if isinstance(obj, list):
        return [p for i, v in enumerate(obj) for p in konten_in(v, konten, f"{pfad}/{i}")]
    return [pfad] if isinstance(obj, str) and (obj in konten or obj.startswith("konto-")) else []


def sha_inhalt(inhalt):
    return hashlib.sha256((json.dumps(inhalt, ensure_ascii=False, indent=1, sort_keys=True) + "\n").encode("utf-8")).hexdigest()


def admins_aus(inhalt, konten):
    """uids der Admins nach der gerade gültigen Datei; ohne lesbare Kontenliste niemand (H4, H5)."""
    a = inhalt.get("admins") if isinstance(inhalt, dict) else None
    return {konten[k] for k in a if isinstance(k, str) and k in konten} if isinstance(a, list) else set()


def pruefen_datei(wo, e, sha_vorher, fehler):
    """Form der Datei im Ergebnis und S12. Gibt die sha256 danach zurück oder None bei Formfehler."""
    d = e.get("datei")
    if not isinstance(d, dict) or set(d) != DATEI_ERG or not HEX64.match(d["sha256"]) or d["sha256"] != sha_inhalt(d["inhalt"]):
        fehler.append(f"{wo}: Datei im Ergebnis hat nicht die Form aus Abschnitt 15.12 oder sha256 passt nicht zum Inhalt")
        return None
    gleich = d["sha256"] == sha_vorher
    if e["unveraendert"] != gleich or e["temp_bleibt"] is not False:
        fehler.append(f"{wo}: unveraendert widerspricht der sha256, oder die Temp-Datei bleibt (S12)")
    return d["sha256"]


def pruefen_schreiben(fehler):
    faelle = {p.name for p in (HIER / "faelle-schreiben").glob("*.json")}
    erwartet = {p.name for p in (HIER / "erwartet-schreiben").glob("*.json")}
    for name in sorted(faelle ^ erwartet):
        fehler.append(f"schreiben {name}: ohne Gegenstück")
    zahl, codes, arten, wege = 0, set(), set(), set()
    for name in sorted(faelle & erwartet):
        zahl += 1
        try:
            fall = streng((HIER / "faelle-schreiben" / name).read_text(encoding="utf-8"))
            erw = streng((HIER / "erwartet-schreiben" / name).read_text(encoding="utf-8"))
        except ValueError as e:
            fehler.append(f"schreiben {name}: {e}")
            continue
        if set(fall) != FALL_SCHREIBEN or fall["name"] + ".json" != name or len(erw["schritte"]) != len(fall["schritte"]):
            fehler.append(f"schreiben {name}: Felder des Falls, Name oder Zahl der Ergebnisse falsch")
            continue
        zu, uids = fall["zuordnung"], fall["konten"]
        konten = set(zu) | set(uids)
        if any(not PSEUDONYM.match(p) for p in zu.values()) or len(set(zu.values())) != len(zu):
            fehler.append(f"schreiben {name}: zuordnung bildet nicht eindeutig auf Pseudonyme ab")
        if any(type(u) is not int or u < 1 for u in uids.values()) or len(set(uids.values())) != len(uids) or set(uids) != set(zu):
            fehler.append(f"schreiben {name}: konten bildet nicht eindeutig auf uids ab oder passt nicht zur zuordnung")
        konto_von = {u: k for k, u in uids.items()}
        inhalt = fall["firmenvorgabe"]["inhalt"]
        admins = admins_aus(inhalt, uids)
        sha_vorher = sha_inhalt(inhalt)
        gesperrt = False
        for i, (s, e) in enumerate(zip(fall["schritte"], erw["schritte"])):
            wo = f"schreiben {name} Schritt {i}"
            if len(s) != 1 or next(iter(s)) not in SCHRITT_ARTEN:
                fehler.append(f"{wo}: Schritt hat nicht genau schreiben, anfrage, nachricht oder neuladen")
                continue
            art = next(iter(s))
            if art == "neuladen":
                if s["neuladen"] != {} or e != {"geladen": sha_vorher, "gesperrt": False}:
                    fehler.append(f"{wo}: neuladen lädt nicht den Inhalt der Datei oder hebt die Sperre nicht auf")
                gesperrt = False
                continue
            if art == "nachricht":
                n = e["einstellungen"]
                wer = zu.get(konto_von.get(s["nachricht"]["partner"]))
                if set(n) != {"art", "fassung", "admin", "werte"} or n["admin"] != (s["nachricht"]["partner"] in admins) or wer is None:
                    fehler.append(f"{wo}: Nachricht Einstellungen hat nicht die Form aus Abschnitt 15.10")
                    continue
                arten.add("admin" if n["admin"] else "nutzer")
                stellen = konten_in(n, konten)
                erlaubt = [p for p in stellen if n["admin"] and re.match(r"^/werte/leseklassen/wert/\d+/wer$", p)
                           and n["werte"]["leseklassen"]["wert"][int(p.split("/")[4])]["ebene"] == "nutzer"]
                if stellen != erlaubt:
                    fehler.append(f"{wo}: Konto außerhalb des einen Wegs (S10, Abschnitt 15.13): {sorted(set(stellen) - set(erlaubt))}")
                continue
            a = s[art]
            wege.add(art)
            if art == "anfrage":
                if set(a) != {"partner", "anfrage", "fehler"} or not set(a["fehler"]) <= FEHLER_HELFER:
                    fehler.append(f"{wo}: anfrage hat nicht genau partner, anfrage, fehler oder einen Fehler des Dienstes")
                ant = e.get("antwort")
                if set(e) != {"antwort", "datei", "unveraendert", "temp_bleibt"} or not isinstance(ant, dict) \
                        or set(ant) != {"code", "sha256"}:
                    fehler.append(f"{wo}: Ergebnis der Anfrage hat nicht die Form aus Abschnitt 15.12")
                    continue
                c = ant["code"]
                if c is not None and c not in CODES_HELFER:
                    fehler.append(f"{wo}: Code {c} ist kein Code des Helfers")
                if (c is None) != (ant["sha256"] is not None) or (c is None and ant["sha256"] != e["datei"]["sha256"]):
                    fehler.append(f"{wo}: sha256 der Antwort steht nur beim Ersetzen und nennt dann die neue Datei")
                angenommen = c is None
            else:
                if set(a) != {"partner", "wert", "fehler"} or not set(a["fehler"]) <= FEHLER_EIN:
                    fehler.append(f"{wo}: schreiben hat nicht genau partner, wert, fehler")
                z = e["zeile"]
                c = e["code"]
                if set(z) != ZEILE or not HEX64.match(z["neu"]) or (z["alt"] is not None and not HEX64.match(z["alt"])):
                    fehler.append(f"{wo}: Zeile hat nicht die Form aus Vertrag 5, Abschnitt 6.6")
                if konten_in(z, konten) or not PSEUDONYM.match(z["handelnd"]) or z["handelnd"] != zu.get(konto_von.get(a["partner"])):
                    fehler.append(f"{wo}: Zeile nennt ein Konto oder handelnd ist nicht das Pseudonym des Partners (S10)")
                if (c is None) != e["angenommen"] or z["code"] != c \
                        or z["aktionsname"] != ("regel.geaendert" if e["angenommen"] else "regel.abgelehnt"):
                    fehler.append(f"{wo}: angenommen, code und Zeile widersprechen sich")
                if c is not None and c not in CODES_SCHREIBEN | CODES_SETZEN:
                    fehler.append(f"{wo}: unbekannter Code {c}")
                if gesperrt and c != "firmenvorgabe.zustand_unklar":
                    fehler.append(f"{wo}: nach zustand_unklar nimmt der Dienst vor dem Neuladen eine Änderung an (Sperre)")
                angenommen = c is None
            if c is not None:
                codes.add(c)
            sha_nach = pruefen_datei(wo, e, sha_vorher, fehler)
            if sha_nach is None:
                continue
            d = e["datei"]
            # S12: Jeder Fehler lässt die Datei byte-gleich, außer zustand_unklar (der Ausgang ist unbekannt).
            if c not in (None, "firmenvorgabe.zustand_unklar") and sha_nach != sha_vorher:
                fehler.append(f"{wo}: Datei nach einem Fehler nicht byte-gleich (S12)")
            if c in ("ebenen.nicht_befugt", "firmenvorgabe.partner_nicht_admin") and a["partner"] in admins:
                fehler.append(f"{wo}: nicht befugt für einen Admin der gültigen Datei")
            # S11: Nur ein Admin der gerade gültigen Datei ersetzt sie, nie einer, den erst die neue Datei nennt.
            if (angenommen or sha_nach != sha_vorher) and a["partner"] not in admins:
                fehler.append(f"{wo}: Datei ersetzt für einen Partner, der in der gültigen Datei kein Admin ist (S11)")
            if sha_nach != sha_vorher:
                alt_ohne = {k: v for k, v in inhalt.items() if k != "leseklassen"} if isinstance(inhalt, dict) else None
                neu_ohne = {k: v for k, v in d["inhalt"].items() if k != "leseklassen"}
                if alt_ohne != neu_ohne:
                    fehler.append(f"{wo}: der Helfer ändert mehr als leseklassen, etwa die Kontenliste der Admins (S13)")
                if (d["eigentuemer"], d["modus"]) != (HELFER, "0644"):
                    fehler.append(f"{wo}: die ersetzte Datei gehört nicht {HELFER} mit Modus 0644")
            if c == "firmenvorgabe.zustand_unklar" and art == "schreiben":
                gesperrt = True
            sha_vorher, inhalt = sha_nach, d["inhalt"]
            admins = admins_aus(inhalt, uids)
    for c in sorted(CODES_SCHREIBEN - codes):
        fehler.append(f"schreiben: kein Fall für den Code {c}")
    for art in sorted({"admin", "nutzer"} - arten):
        fehler.append(f"schreiben: keine Nachricht an einen Betrachter der Art {art}")
    for weg in sorted({"schreiben", "anfrage"} - wege):
        fehler.append(f"schreiben: kein Fall für den Weg {weg}")
    return zahl


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
    zahl_setzen = pruefen_setzen(fehler)
    zahl_schreiben = pruefen_schreiben(fehler)
    print(f"{zahl} Fälle, {zahl_setzen} Fälle zu setzen und {zahl_schreiben} Fälle zum Schreibweg, {schritte_zahl} Schritte, {len(fehler)} Fehler, "
          f"Schema {'geprüft' if p_ebenen else 'NICHT geprüft (--ohne-schema, jsonschema fehlt)'}; "
          f"{nur_bau} Ablehnungen erst mit dem Bau prüfbar (Bezug, Befugnis, Lockerung, Ladefehler)")
    for zeile in fehler:
        print(f"  FEHLER {zeile}")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
