#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags 9 (werkbank-anmelden): Paarung, Felder, Codes und feste Sätze gegen README.md.

Aufruf: python3 -I rust/spec/anmelden/pruefen.py
Exit 0, wenn jeder Punkt stimmt. Die Fälle selbst (Läufe des Programms) prüft tests/faelle.rs des Crates.
Jeder Code der Tabelle in README.md Abschnitt 5 braucht mindestens einen Fall; jeder Fall nennt einen Code der Tabelle oder keinen.
Die Ordner faelle/, erwartet/, faelle-uebernehmen/ und erwartet-uebernehmen/ liest der Code des Crates. Ausstehende Ordner gibt es seit
dem Zug des Nachtrags Halter nicht mehr; liegt wieder einer da, ist das ein Fehler.
"""
import importlib.util
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
FELDER_FALL = {"beschreibung", "args", "attrappe", "token", "ziel", "echtes_heim", "bundtool", "wegwerf_wurzel", "signal", "reste"}
FELDER_ERWARTET = {"exit", "code", "stdout", "stderr", "ziel_datei", "login_gestartet", "reste_geloescht", "reste_uebrig"}
WERTE = {
    "attrappe": {"erfolg", "abbruch", "ohne-config", "config-ohne-feld", "zwei-tokens", "config-defekt", "alte-form", "haengt", "fehlt"},
    "ziel": {"frei", "datei-da", "ordner-fehlt", "schreibgeschuetzt", "heim-fehlt", "ebene-verweis", "ebene-geteilt", "ebene-privat", "ebene-privat-mitglied", "ebene-privat-sss",
             "ebene-privat-ohne-nss", "alte-datei"},
    "echtes_heim": {"leer", "config-mit-token", "config-defekt"},
    "bundtool": {"keins", "leer", "gefunden"},
    "wegwerf_wurzel": {"ok", "fehlt"},
    "reste": {"keine", "tot", "lebend", "verweis", "nicht-loeschbar"},
}
# Ordner der ausstehenden Fälle des Nachtrags Halter; seit dessen Zug gibt es sie nicht mehr.
AUSSTEHEND_ORDNER = ("faelle-ausstehend", "erwartet-ausstehend", "faelle-uebernehmen-ausstehend", "erwartet-uebernehmen-ausstehend")
SIGNAL_EXIT = {"INT": 130, "TERM": 143, "HUP": 129}
ZIEL_DATEI = {"neu", "ersetzt", "unveraendert", "nicht-angelegt"}
# Muster eines echten Tokens: kein Fall und kein Text des Vertrags darf eines als Text tragen.
TOKEN_MUSTER = re.compile(r"(gho|ghu|ghp)_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}")


def tabelle():
    """Liest die Codetabelle aus README.md: Code -> (Exit oder None, Satz)."""
    text = (HIER / "README.md").read_text(encoding="utf-8")
    codes = {}
    for zeile in text.splitlines():
        m = re.match(r"\| \d+ \| `(anmelden\.[a-z_]+)` \| ([^|]+?) \| `([^`]+)` \|", zeile)
        if m:
            exit_ = None if "n" in m.group(2) else int(m.group(2))
            codes[m.group(1)] = (exit_, m.group(3))
    return codes


def bauer():
    spec = importlib.util.spec_from_file_location("bau_faelle", HIER / "bau-faelle.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# Codes ohne sichtbaren Fall: ihr Fehler lässt sich ohne Eingriff ins System nicht herbeiführen; die Bibliothek prüft sie
# (tests/uebernehmen.rs, a_refused_protection_is_its_own_code_and_fail_closed).
OHNE_FALL = {"anmelden.abschirmung"}

FELDER_UEB = {"beschreibung", "args", "quelle", "quellordner", "token", "ziel", "bundtool", "reste", "probe"}
FELDER_UEB_ERWARTET = {"exit", "code", "stdout", "stderr", "ziel_datei", "config", "reste_geloescht", "reste_uebrig"}
WERTE_UEB = {
    "quelle": {"erfolg", "erfolg-letzter", "erfolg-allein", "ohne-config", "kein-json", "ohne-feld", "zwei-tokens",
               "weitere-anmeldung", "token-ausserhalb", "token-andere-tiefe", "kommentar-mitte", "blockkommentar",
               "komma-vor-klammer", "alte-form", "verweis", "hardlink", "zu-gross", "fifo", "ordner"},
    "quellordner": {"ok", "fehlt", "verweis", "gruppe-schreibbar"},
    "ziel": {"frei", "datei-da", "datei-gleich", "ordner-fehlt", "schreibgeschuetzt", "heim-fehlt", "ebene-verweis", "ebene-privat", "ebene-privat-mitglied",
             "ebene-privat-sss"},
    "bundtool": {"keins", "leer", "gefunden"},
    "reste": {"keine", "schluessel-temp", "config-temp", "beide", "lebend", "verweis"},
    "probe": {None, "config-stoeren"},
}
ZIEL_UEB = {"token", "alt", "keine"}
CONFIG_UEB = {"ohne-token", "unveraendert"}


def uebernahme_pruefen(mod, codes, fehler, f_ordner="faelle-uebernehmen", e_ordner="erwartet-uebernehmen", menge=None, kaputt=None, ziel_mehr=frozenset()):
    """Prüft die Fälle der Übernahme (Abschnitt 3b) wie die des Geräteflusses. Gibt die benutzten Codes und die Namen zurück."""
    menge = mod.FAELLE_UEB if menge is None else menge
    kaputt = mod.ZIEL_KAPUTT if kaputt is None else kaputt
    benutzt = set()
    namen_f = {p.stem for p in (HIER / f_ordner).glob("*.json")}
    namen_e = {p.stem for p in (HIER / e_ordner).glob("*.json")}
    if namen_f != namen_e:
        fehler.append("%s, Paarung: nur in faelle %s, nur in erwartet %s" % (f_ordner, sorted(namen_f - namen_e), sorted(namen_e - namen_f)))
    if namen_f != set(menge):
        fehler.append("%s: die Fälle weichen von bau-faelle.py ab (neu erzeugen)" % f_ordner)
    for name in sorted(namen_f & namen_e):
        roh_f = (HIER / f_ordner / (name + ".json")).read_text(encoding="utf-8")
        roh_e = (HIER / e_ordner / (name + ".json")).read_text(encoding="utf-8")
        if TOKEN_MUSTER.search(roh_f) or TOKEN_MUSTER.search(roh_e):
            fehler.append("%s: ein Text hat die Form eines Tokens" % name)
        f, e = json.loads(roh_f), json.loads(roh_e)
        if set(f) != FELDER_UEB or set(e) != FELDER_UEB_ERWARTET:
            fehler.append("%s: Felder des Falls oder der Antwort" % name)
            continue
        for feld, erlaubt in WERTE_UEB.items():
            if f[feld] not in (erlaubt | ziel_mehr if feld == "ziel" else erlaubt):
                fehler.append("%s: %s=%r" % (name, feld, f[feld]))
        if "--uebernehmen" not in f["args"]:
            fehler.append("%s: ein Fall der Übernahme ruft --uebernehmen" % name)
        if e["ziel_datei"] not in ZIEL_UEB or e["config"] not in CONFIG_UEB:
            fehler.append("%s: ziel_datei oder config" % name)
        code = e["code"]
        if code is None:
            if e["exit"] != 0 or e["stdout"] != "Anmeldung gespeichert.\n" or e["ziel_datei"] != "token" or e["config"] != "ohne-token":
                fehler.append("%s: Erfolg heißt Exit 0, Token in der Schlüsseldatei, config.json ohne Token" % name)
        else:
            benutzt.add(code)
            if code not in codes or e["exit"] != codes[code][0]:
                fehler.append("%s: Code oder Exit" % name)
                continue
            if not e["stderr"].endswith("werkbank-anmelden: %s: %s\n" % (code, codes[code][1])):
                fehler.append("%s: stderr endet nicht mit dem festen Satz" % name)
            if code != "anmelden.echte_anmeldung" and e["config"] != "unveraendert":
                fehler.append("%s: bei einem Fehler bleibt config.json unverändert" % name)
            if code not in ("anmelden.echte_anmeldung",) and e["ziel_datei"] == "token" and f["ziel"] != "datei-gleich":
                fehler.append("%s: bei einem Fehler entsteht keine Schlüsseldatei" % name)
        if name in menge and (menge[name] != f or mod.erwartet_ueb(f, kaputt) != e):
            fehler.append("%s: weicht von bau-faelle.py ab (neu erzeugen)" % name)
    return benutzt, namen_f


def geraet_pruefen(mod, codes, fehler, f_ordner="faelle", e_ordner="erwartet", menge=None, kaputt=None, ziel_mehr=frozenset()):
    """Prüft die Fälle des Geräteflusses in einem Ordnerpaar. Gibt die benutzten Codes und die Namen zurück."""
    menge = mod.FAELLE if menge is None else menge
    kaputt = mod.ZIEL_KAPUTT if kaputt is None else kaputt
    namen_f = {p.stem for p in (HIER / f_ordner).glob("*.json")}
    namen_e = {p.stem for p in (HIER / e_ordner).glob("*.json")}
    if namen_f != namen_e:
        fehler.append("%s, Paarung: nur in faelle %s, nur in erwartet %s" % (f_ordner, sorted(namen_f - namen_e), sorted(namen_e - namen_f)))
    if namen_f != set(menge):
        fehler.append("%s: die Fälle weichen von bau-faelle.py ab (neu erzeugen)" % f_ordner)
    benutzt = set()
    for name in sorted(namen_f & namen_e):
        roh_f = (HIER / f_ordner / (name + ".json")).read_text(encoding="utf-8")
        roh_e = (HIER / e_ordner / (name + ".json")).read_text(encoding="utf-8")
        for roh in (roh_f, roh_e):
            if TOKEN_MUSTER.search(roh):
                fehler.append("%s: ein Text hat die Form eines Tokens" % name)
        f, e = json.loads(roh_f), json.loads(roh_e)
        if set(f) != FELDER_FALL:
            fehler.append("%s: Felder des Falls %s" % (name, sorted(set(f) ^ FELDER_FALL)))
            continue
        if set(e) != FELDER_ERWARTET:
            fehler.append("%s: Felder der Antwort %s" % (name, sorted(set(e) ^ FELDER_ERWARTET)))
            continue
        for feld, erlaubt in WERTE.items():
            if f[feld] not in (erlaubt | ziel_mehr if feld == "ziel" else erlaubt):
                fehler.append("%s: %s=%r" % (name, feld, f[feld]))
        if f["signal"] not in (None, "INT", "TERM", "HUP"):
            fehler.append("%s: signal=%r" % (name, f["signal"]))
        if not (isinstance(f["token"], dict) and set(f["token"]) == {"praefix", "fuell", "laenge"}):
            fehler.append("%s: token" % name)
        if e["ziel_datei"] not in ZIEL_DATEI:
            fehler.append("%s: ziel_datei=%r" % (name, e["ziel_datei"]))
        code = e["code"]
        if code is None:
            if e["exit"] != 0 or not e["stdout"].endswith("Anmeldung gespeichert.\n"):
                fehler.append("%s: Erfolg braucht Exit 0 und die Zeile am Ende von stdout" % name)
        else:
            benutzt.add(code)
            if code not in codes:
                fehler.append("%s: Code %s nicht in der Tabelle" % (name, code))
                continue
            exit_, satz = codes[code]
            if code == "anmelden.signal":
                if f["signal"] is None or e["exit"] != SIGNAL_EXIT[f["signal"]]:
                    fehler.append("%s: Signal und Exit passen nicht" % name)
            elif e["exit"] != exit_:
                fehler.append("%s: Exit %s statt %s" % (name, e["exit"], exit_))
            vor = "Hinweis: Reste frueherer Anmeldungen wurden geloescht: %d\n" % e["reste_geloescht"] if e["reste_geloescht"] else ""
            if code == "anmelden.ziel_nicht_schreibbar":
                vor += mod.hinweis_gruppe(f["ziel"])  # Nachtrag 1 (28.11.2026): der Hinweis mit der Abhilfe
            if code == "anmelden.rest_nicht_loeschbar" and e["reste_geloescht"]:
                fehler.append("%s: bei einem nicht löschbaren Rest gibt es keinen Hinweis" % name)
            if e["stderr"] != vor + "werkbank-anmelden: %s: %s\n" % (code, satz):
                fehler.append("%s: stderr weicht vom festen Satz ab" % name)
        if not all(isinstance(e[k], int) and e[k] >= 0 for k in ("reste_geloescht", "reste_uebrig")):
            fehler.append("%s: reste_geloescht und reste_uebrig sind Zahlen" % name)
        if f["reste"] == "tot" and e["code"] != "anmelden.aufruf" and (e["reste_geloescht"] != 1 or e["reste_uebrig"] != 0):
            fehler.append("%s: ein toter Rest wird gelöscht" % name)
        if f["reste"] in ("lebend", "verweis", "nicht-loeschbar") and (e["reste_geloescht"] != 0 or e["reste_uebrig"] != 1):
            fehler.append("%s: dieser Rest bleibt" % name)
        if f["signal"] and f["attrappe"] != "haengt":
            fehler.append("%s: ein Signal braucht die Attrappe haengt" % name)
        # Die Antwort stimmt mit dem Erzeuger überein (kein von Hand geänderter Fall).
        if name in menge and (menge[name] != f or mod.erwartet(f, kaputt) != e):
            fehler.append("%s: weicht von bau-faelle.py ab (neu erzeugen)" % name)
    return benutzt, namen_f


def main():
    fehler = []
    codes = tabelle()
    if len(codes) != 15:
        fehler.append("Tabelle in README.md hat %d statt 15 Codes" % len(codes))
    mod = bauer()
    if {k: v for k, v in mod.CODES.items()} != codes:
        fehler.append("CODES in bau-faelle.py weicht von der Tabelle in README.md ab")
    benutzt, namen_f = geraet_pruefen(mod, codes, fehler)
    b2, namen_u = uebernahme_pruefen(mod, codes, fehler)
    # Die ausstehenden Fälle des Nachtrags Halter sind verschoben: keine Menge, kein Ordner.
    if mod.FAELLE_AUSSTEHEND or mod.FAELLE_UEB_AUSSTEHEND:
        fehler.append("bau-faelle.py: FAELLE_AUSSTEHEND und FAELLE_UEB_AUSSTEHEND sind leer")
    for ordner in AUSSTEHEND_ORDNER:
        if (HIER / ordner).exists():
            fehler.append("%s: der Ordner ist entfernt, seine Fälle stehen in den gelesenen Ordnern" % ordner)
    benutzt = benutzt | b2
    for code in codes:
        if code not in benutzt and code not in OHNE_FALL:
            fehler.append("kein Fall für %s" % code)
    readme = (HIER / "README.md").read_text(encoding="utf-8")
    if TOKEN_MUSTER.search(readme):
        fehler.append("README.md: ein Text hat die Form eines Tokens")
    if fehler:
        for z in fehler:
            sys.stderr.write("FEHLER: %s\n" % z)
        sys.exit(1)
    sys.stdout.write("anmelden: %d Fälle, %d Fälle der Übernahme, %d Codes, alles stimmt\n"
                     % (len(namen_f), len(namen_u), len(codes)))


if __name__ == "__main__":
    main()
