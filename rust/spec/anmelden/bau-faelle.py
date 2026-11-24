#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags 9 (werkbank-anmelden) in faelle/ und erwartet/, die der Übernahme
(Abschnitt 3b) in faelle-uebernehmen/ und erwartet-uebernehmen/.

Aufruf: python3 -I rust/spec/anmelden/bau-faelle.py
Die Fälle sind Daten; die Attrappe der CLI und das Wegwerf-Heim baut erst der Test (tests/faelle.rs). Das Token steht
nie als Text in einem Fall: ein Fall nennt Präfix, Füllzeichen und Länge, der Test setzt es zusammen.
"""
import json
from pathlib import Path

HIER = Path(__file__).resolve().parent

# Code -> (Exit, fester Satz). Spiegel der Tabelle in README.md Abschnitt 5; pruefen.py vergleicht beide.
CODES = {
    "anmelden.aufruf": (64, "Aufruf: werkbank-anmelden copilot [--uebernehmen] [--neu]"),
    "anmelden.cli_fehlt": (10, "Die Copilot CLI wurde nicht gefunden."),
    "anmelden.login_abgebrochen": (11, "Die Anmeldung wurde nicht abgeschlossen."),
    "anmelden.kein_token": (12, "Die Anmeldung hat kein Token geliefert."),
    "anmelden.form_falsch": (13, "Das Token hat nicht die erwartete Form."),
    "anmelden.ziel_nicht_schreibbar": (14, "Der Zielordner ist nicht schreibbar."),
    "anmelden.datei_da": (15, "Die Schluesseldatei ist schon da. Mit --neu wird sie ersetzt."),
    "anmelden.echte_anmeldung": (16, "Im Heim des Nutzers liegt eine Anmeldung der Copilot CLI. Sie muss weg, bevor eine Sitzung startet."),
    "anmelden.heim_nicht_anlegbar": (17, "Das Wegwerf-Heim ist nicht anlegbar."),
    "anmelden.signal": (None, "Abgebrochen durch ein Signal."),
    "anmelden.rest_nicht_loeschbar": (18, "Ein Rest eines frueheren Laufs ist nicht loeschbar."),
    "anmelden.quelle_ohne_token": (19, "In der Konfiguration der Copilot CLI liegt nicht genau ein Token."),
    "anmelden.quelle_unsicher": (20, "Die Konfiguration der Copilot CLI ist keine eigene regulaere Datei bis 1 MiB."),
    "anmelden.quelle_nicht_schreibbar": (21, "Die Konfiguration der Copilot CLI ist nicht neu schreibbar."),
    "anmelden.abschirmung": (5, "Der Schutz vor einem Kernabbild ist nicht setzbar."),
}
SIGNALE = {"INT": 130, "TERM": 143, "HUP": 129}
GERAETE_ZEILEN = (
    "To authenticate, visit https://github.com/login/device and enter code ABCD-1234\n"
    "Waiting for authorization...\n"
)
HINWEIS_BUND = "Hinweis: Der Schluesselbund wurde nicht geprueft, weil secret-tool und security fehlen.\n"
HINWEIS_REST = "Hinweis: Reste frueherer Anmeldungen wurden geloescht: %d\n"
OK_ZEILE = "Anmeldung gespeichert.\n"


def fall(beschreibung, **kw):
    f = {
        "beschreibung": beschreibung,
        "args": ["copilot"],
        "attrappe": "erfolg",
        "token": {"praefix": "gho_", "fuell": "A", "laenge": 40},
        "ziel": "frei",
        "echtes_heim": "leer",
        "bundtool": "leer",
        "wegwerf_wurzel": "ok",
        "signal": None,
        "reste": "keine",
    }
    f.update(kw)
    return f


FAELLE = {
    "erfolg-neu": fall("Gerätefluss, Token gespeichert; kein Schlüsselbund-Werkzeug im PATH: fester Hinweis", bundtool="keins"),
    "erfolg-bund-leer": fall("Wie erfolg-neu, ein Werkzeug sagt: kein Eintrag; kein Hinweis"),
    "erfolg-ersetzt-mit-neu": fall("Datei da, --neu ersetzt sie", args=["copilot", "--neu"], ziel="datei-da"),
    "erfolg-neu-ohne-vorhandene-datei": fall("--neu ohne vorhandene Datei legt sie an", args=["copilot", "--neu"]),
    "erfolg-token-ghu": fall("Token mit Präfix ghu_", token={"praefix": "ghu_", "fuell": "B", "laenge": 40}),
    "erfolg-token-feingranular": fall("Token mit Präfix github_pat_", token={"praefix": "github_pat_", "fuell": "C", "laenge": 93}),
    "aufruf-ohne-argument": fall("kein Argument", args=[]),
    "aufruf-unbekannte-option": fall("unbekannte Option", args=["copilot", "--alt"]),
    "aufruf-anderes-harness": fall("anderes Harness", args=["claude"]),
    "aufruf-neu-zuerst": fall("--neu vor dem Harness", args=["--neu", "copilot"]),
    "aufruf-doppelt": fall("--neu zweimal", args=["copilot", "--neu", "--neu"]),
    "cli-fehlt": fall("kein copilot im PATH", attrappe="fehlt"),
    "login-abgebrochen": fall("die CLI endet mit Exit 1 ohne Token", attrappe="abbruch"),
    "kein-token-ohne-config": fall("die CLI endet mit 0, aber ohne config.json", attrappe="ohne-config"),
    "kein-token-ohne-feld": fall("config.json ohne das Feld des Tokens", attrappe="config-ohne-feld"),
    "kein-token-zwei-tokens": fall("config.json mit zwei Tokens: keine Wahl", attrappe="zwei-tokens"),
    "kein-token-alte-form": fall("config.json in der früheren, nie gemessenen Form mit copilot_tokens: kein Token", attrappe="alte-form"),
    "kein-token-config-defekt": fall("config.json ist kein JSON", attrappe="config-defekt"),
    "form-falsch-klassisch": fall("klassisches Token ghp_", token={"praefix": "ghp_", "fuell": "D", "laenge": 40}),
    "form-falsch-platzhalter": fall("der feste Platzhalter ist nie ein Schlüssel", token={"praefix": "gho_", "fuell": "0", "laenge": 40}),
    "form-falsch-zu-kurz": fall("Token zu kurz", token={"praefix": "gho_", "fuell": "E", "laenge": 12}),
    "form-falsch-zeichen": fall("Token mit Leerzeichen", token={"praefix": "gho_", "fuell": "F G", "laenge": 40}),
    "ziel-ordner-fehlt": fall("Zielordner fehlt", ziel="ordner-fehlt"),
    "ziel-schreibgeschuetzt": fall("Zielordner ohne Schreibrecht", ziel="schreibgeschuetzt"),
    "datei-da-ohne-neu": fall("Datei da, kein --neu: die CLI startet nicht", ziel="datei-da"),
    "echte-anmeldung-config": fall("echtes Heim hat config.json mit Token", echtes_heim="config-mit-token"),
    "echte-anmeldung-config-defekt": fall("echtes Heim hat unlesbare config.json: fail closed", echtes_heim="config-defekt"),
    "echte-anmeldung-bund": fall("Schlüsselbund hat einen Eintrag copilot-cli", bundtool="gefunden"),
    "heim-nicht-anlegbar": fall("Wurzel des Wegwerf-Heims fehlt", wegwerf_wurzel="fehlt"),
    "reste-tot-geloescht": fall("Rest eines toten Laufs wird vor dem Login gelöscht; Hinweis mit der Zahl", reste="tot"),
    "reste-lebender-lauf-bleibt": fall("Rest eines lebenden Laufs bleibt; kein Hinweis", reste="lebend"),
    "reste-verweis-bleibt": fall("Verweis mit dem Namen eines Rests bleibt, sein Ziel wird nicht verfolgt", reste="verweis"),
    "reste-nicht-loeschbar": fall("Rest eines toten Laufs ist nicht löschbar: Code, kein Login", reste="nicht-loeschbar"),
    "reste-tot-und-datei-da": fall("Rest wird gelöscht, dann endet der Aufruf mit datei_da", reste="tot", ziel="datei-da"),
    "signal-int": fall("SIGINT während des Logins", attrappe="haengt", signal="INT"),
    "signal-term": fall("SIGTERM während des Logins", attrappe="haengt", signal="TERM"),
    "signal-hup": fall("SIGHUP während des Logins", attrappe="haengt", signal="HUP"),
}


def erwartet(f):
    a = f["args"]
    gueltig = a in (["copilot"], ["copilot", "--neu"])
    neu = a == ["copilot", "--neu"]
    e = {"exit": 0, "code": None, "stdout": "", "stderr": "", "ziel_datei": "nicht-angelegt", "login_gestartet": False,
         "reste_geloescht": 0, "reste_uebrig": 0}
    vor = ""
    if f["ziel"] == "datei-da":
        e["ziel_datei"] = "unveraendert"

    def fehler(code, ziel=None, login=False, exit_=None):
        e["code"] = code
        e["exit"] = CODES[code][0] if exit_ is None else exit_
        e["stderr"] = vor + "werkbank-anmelden: %s: %s\n" % (code, CODES[code][1])
        e["login_gestartet"] = login
        if ziel:
            e["ziel_datei"] = ziel
        return e

    e["reste_uebrig"] = 0 if f["reste"] == "keine" else 1
    if not gueltig:
        return fehler("anmelden.aufruf")
    if f["reste"] == "nicht-loeschbar":
        return fehler("anmelden.rest_nicht_loeschbar")
    if f["reste"] == "tot":
        e["reste_geloescht"] = 1
        e["reste_uebrig"] = 0
        vor = HINWEIS_REST % 1
    if f["ziel"] == "datei-da" and not neu:
        return fehler("anmelden.datei_da")
    if f["ziel"] in ("ordner-fehlt", "schreibgeschuetzt"):
        return fehler("anmelden.ziel_nicht_schreibbar")
    if f["attrappe"] == "fehlt":
        return fehler("anmelden.cli_fehlt")
    if f["wegwerf_wurzel"] == "fehlt":
        return fehler("anmelden.heim_nicht_anlegbar")
    e["login_gestartet"] = True
    e["stdout"] = GERAETE_ZEILEN
    if f["signal"]:
        return fehler("anmelden.signal", login=True, exit_=SIGNALE[f["signal"]])
    if f["attrappe"] == "abbruch":
        return fehler("anmelden.login_abgebrochen", login=True)
    if f["attrappe"] in ("ohne-config", "config-ohne-feld", "zwei-tokens", "config-defekt", "alte-form"):
        return fehler("anmelden.kein_token", login=True)
    if not form_ok(f["token"]):
        return fehler("anmelden.form_falsch", login=True)
    stand = "ersetzt" if f["ziel"] == "datei-da" else "neu"
    if f["echtes_heim"] != "leer" or f["bundtool"] == "gefunden":
        return fehler("anmelden.echte_anmeldung", ziel=stand, login=True)
    e["stdout"] += OK_ZEILE
    e["ziel_datei"] = stand
    e["stderr"] = vor
    if f["bundtool"] == "keins":
        e["stderr"] += HINWEIS_BUND
    return e


def form_ok(t):
    text = t["praefix"] + (t["fuell"] * t["laenge"])[: t["laenge"] - len(t["praefix"])]
    if not any(text.startswith(p) for p in ("gho_", "ghu_", "github_pat_")):
        return False  # klassisches ghp_ und alles andere
    praefix = next(p for p in ("github_pat_", "gho_", "ghu_") if text.startswith(p))
    rest = text[len(praefix):]
    platzhalter = "gho_" + "0" * 36
    return 20 <= len(text) <= 255 and rest != "" and all(c.isalnum() or c == "_" for c in rest) and text != platzhalter


# ---- Abschnitt 3b: `copilot --uebernehmen [--neu]` (eigene Fallmenge, die Fälle des Geräteflusses bleiben) ----
HINWEIS_WIEDERANLAUF = "Hinweis: Eine unterbrochene Uebernahme wurde abgeschlossen.\n"
UEB = ["copilot", "--uebernehmen"]
UEB_NEU = ["copilot", "--uebernehmen", "--neu"]


def ueb(beschreibung, **kw):
    f = {
        "beschreibung": beschreibung,
        "args": UEB,
        "quelle": "erfolg",
        "quellordner": "ok",
        "token": {"praefix": "gho_", "fuell": "U", "laenge": 40},
        "ziel": "frei",
        "bundtool": "leer",
        "reste": "keine",
        "probe": None,
    }
    f.update(kw)
    return f


FAELLE_UEB = {
    "ueb-erfolg": ueb("Token aus config.json in die Schlüsseldatei; der Eintrag in der Mitte verschwindet, alles andere bleibt byte-gleich"),
    "ueb-erfolg-letzter-eintrag": ueb("der Eintrag ist der letzte im Feld: das Komma davor geht mit", quelle="erfolg-letzter"),
    "ueb-erfolg-einziger-eintrag": ueb("der Eintrag ist der einzige im Feld: das Feld bleibt als leeres Objekt", quelle="erfolg-allein"),
    "ueb-erfolg-ohne-bundtool": ueb("kein Schlüsselbund-Werkzeug im PATH: fester Hinweis", bundtool="keins"),
    "ueb-erfolg-ghu": ueb("Token mit Präfix ghu_", token={"praefix": "ghu_", "fuell": "V", "laenge": 40}),
    "ueb-erfolg-feingranular": ueb("Token mit Präfix github_pat_", token={"praefix": "github_pat_", "fuell": "W", "laenge": 93}),
    "ueb-erfolg-ersetzt-mit-neu": ueb("Schlüsseldatei mit anderem Inhalt da, --neu ersetzt sie", args=UEB_NEU, ziel="datei-da"),
    "ueb-neu-ohne-vorhandene-datei": ueb("--neu ohne vorhandene Datei legt sie an", args=UEB_NEU),
    "ueb-wiederanlauf": ueb("Schlüsseldatei hat schon genau dieses Token (Lauf starb nach Schritt 8): nur config.json wird fertig", ziel="datei-gleich"),
    "ueb-wiederanlauf-mit-neu": ueb("wie ueb-wiederanlauf, mit --neu", args=UEB_NEU, ziel="datei-gleich"),
    "ueb-datei-da-ohne-neu": ueb("Schlüsseldatei mit anderem Inhalt da, kein --neu: nichts ändert sich", ziel="datei-da"),
    "ueb-aufruf-neu-zuerst": ueb("--neu vor --uebernehmen", args=["copilot", "--neu", "--uebernehmen"]),
    "ueb-aufruf-doppelt": ueb("--uebernehmen zweimal", args=["copilot", "--uebernehmen", "--uebernehmen"]),
    "ueb-aufruf-vor-harness": ueb("--uebernehmen vor dem Harness", args=["--uebernehmen", "copilot"]),
    "ueb-aufruf-neu-doppelt": ueb("--neu zweimal nach --uebernehmen", args=["copilot", "--uebernehmen", "--neu", "--neu"]),
    "ueb-kein-token-ohne-config": ueb("config.json fehlt", quelle="ohne-config"),
    "ueb-kein-token-kein-json": ueb("config.json ist kein JSON", quelle="kein-json"),
    "ueb-kein-token-ohne-feld": ueb("config.json ohne das Feld des Tokens", quelle="ohne-feld"),
    "ueb-kein-token-zwei-tokens": ueb("zwei Tokens im Feld: keine Wahl", quelle="zwei-tokens"),
    "ueb-kein-token-weitere-anmeldung": ueb("ein weiterer tokenartiger Text in einem anderen Feld: nach dem Entfernen bliebe eine Anmeldung", quelle="weitere-anmeldung"),
    "ueb-kein-token-ausserhalb": ueb("ein zweiter Text mit Token-Präfix außerhalb von authTokens: nicht genau ein Token", quelle="token-ausserhalb"),
    "ueb-kein-token-andere-tiefe": ueb("das Token liegt unter authTokens in der falschen Tiefe (Objekt, Text)", quelle="token-andere-tiefe"),
    "ueb-kein-token-kommentar-mitte": ueb("Kommentarzeile in der Mitte: kein JSON", quelle="kommentar-mitte"),
    "ueb-kein-token-blockkommentar": ueb("Blockkommentar am Anfang: kein JSON", quelle="blockkommentar"),
    "ueb-kein-token-komma-vor-klammer": ueb("Komma vor einer schließenden Klammer: kein JSON", quelle="komma-vor-klammer"),
    "ueb-kein-token-alte-form": ueb("die frühere, nie gemessene Form mit copilot_tokens: kein Token", quelle="alte-form"),
    "ueb-kein-token-ordner-fehlt": ueb("der Ordner der CLI fehlt", quellordner="fehlt"),
    "ueb-form-platzhalter": ueb("der feste Platzhalter ist nie ein Schlüssel", token={"praefix": "gho_", "fuell": "0", "laenge": 40}),
    "ueb-form-klassisch": ueb("klassisches Token ghp_", token={"praefix": "ghp_", "fuell": "X", "laenge": 40}),
    "ueb-form-zu-kurz": ueb("Token zu kurz", token={"praefix": "gho_", "fuell": "Y", "laenge": 12}),
    "ueb-unsicher-verweis": ueb("config.json ist ein Verweis auf eine Datei mit Token: nie verfolgt", quelle="verweis"),
    "ueb-unsicher-ordner-verweis": ueb("der Ordner der CLI ist ein Verweis: nie verfolgt", quellordner="verweis"),
    "ueb-unsicher-ordner-geteilt": ueb("der Ordner der CLI ist für die Gruppe schreibbar", quellordner="gruppe-schreibbar"),
    "ueb-unsicher-hardlink": ueb("config.json hat einen zweiten Namen: das Token bliebe dort", quelle="hardlink"),
    "ueb-unsicher-zu-gross": ueb("config.json ist größer als 1 MiB", quelle="zu-gross"),
    "ueb-unsicher-fifo": ueb("config.json ist keine reguläre Datei (FIFO); kein Hängen", quelle="fifo"),
    "ueb-unsicher-ordner-statt-datei": ueb("config.json ist ein Ordner", quelle="ordner"),
    "ueb-ziel-ordner-fehlt": ueb("Zielordner fehlt", ziel="ordner-fehlt"),
    "ueb-ziel-schreibgeschuetzt": ueb("Zielordner ohne Schreibrecht", ziel="schreibgeschuetzt"),
    "ueb-config-nicht-ersetzbar": ueb("das Ersetzen von config.json scheitert nach der Schlüsseldatei: beides zurück", probe="config-stoeren"),
    "ueb-config-nicht-ersetzbar-mit-neu": ueb("wie ueb-config-nicht-ersetzbar, mit --neu über eine alte Datei: die alte kommt zurück", args=UEB_NEU, ziel="datei-da", probe="config-stoeren"),
    "ueb-echte-anmeldung-bund": ueb("der Schlüsselbund hat einen Eintrag: Schlüsseldatei steht, config.json ist ohne Token, Code", bundtool="gefunden"),
    "ueb-reste-schluessel-temp": ueb("Temp-Datei des Schlüssels eines toten Laufs wird geräumt", reste="schluessel-temp"),
    "ueb-reste-config-temp": ueb("Temp-Datei von config.json eines toten Laufs wird geräumt", reste="config-temp"),
    "ueb-reste-beide": ueb("beide Temp-Dateien toter Läufe werden geräumt, Hinweis mit 2", reste="beide"),
    "ueb-reste-lebend-bleibt": ueb("Temp-Datei eines lebenden Laufs bleibt", reste="lebend"),
    "ueb-reste-verweis-bleibt": ueb("Verweis mit dem Namen einer Temp-Datei bleibt, sein Ziel wird nicht berührt", reste="verweis"),
}

QUELLE_UNSICHER = ("verweis", "hardlink", "zu-gross", "fifo", "ordner")
QUELLE_OHNE_TOKEN = ("ohne-config", "kein-json", "ohne-feld", "zwei-tokens", "weitere-anmeldung",
                      "token-ausserhalb", "token-andere-tiefe", "kommentar-mitte", "blockkommentar", "komma-vor-klammer", "alte-form")
RESTE_TOT = {"keine": 0, "schluessel-temp": 1, "config-temp": 1, "beide": 2, "lebend": 0, "verweis": 0}


def erwartet_ueb(f):
    a = f["args"]
    neu = a == UEB_NEU
    e = {"exit": 0, "code": None, "stdout": "", "stderr": "", "ziel_datei": "keine", "config": "unveraendert",
         "reste_geloescht": 0, "reste_uebrig": 0}
    if f["ziel"] == "datei-da":
        e["ziel_datei"] = "alt"
    if f["ziel"] == "datei-gleich":
        e["ziel_datei"] = "token"
    vor = ""

    def fehler(code):
        e["code"] = code
        e["exit"] = CODES[code][0]
        e["stderr"] = vor + "werkbank-anmelden: %s: %s\n" % (code, CODES[code][1])
        return e

    e["reste_uebrig"] = 1 if f["reste"] in ("lebend", "verweis") else (0 if f["reste"] == "keine" else RESTE_TOT[f["reste"]])
    if a not in (UEB, UEB_NEU):
        return fehler("anmelden.aufruf")
    n = RESTE_TOT[f["reste"]]
    e["reste_geloescht"] = n
    e["reste_uebrig"] = 1 if f["reste"] in ("lebend", "verweis") else 0
    if n:
        vor = HINWEIS_REST % n
    if f["quellordner"] == "fehlt":
        return fehler("anmelden.quelle_ohne_token")
    if f["quellordner"] in ("verweis", "gruppe-schreibbar"):
        return fehler("anmelden.quelle_unsicher")
    if f["quelle"] in QUELLE_UNSICHER:
        return fehler("anmelden.quelle_unsicher")
    if f["quelle"] in QUELLE_OHNE_TOKEN:
        return fehler("anmelden.quelle_ohne_token")
    if not form_ok(f["token"]):
        return fehler("anmelden.form_falsch")
    if f["ziel"] in ("ordner-fehlt", "schreibgeschuetzt"):
        return fehler("anmelden.ziel_nicht_schreibbar")
    if f["ziel"] == "datei-da" and not neu:
        return fehler("anmelden.datei_da")
    if f["probe"] == "config-stoeren":
        return fehler("anmelden.quelle_nicht_schreibbar")
    e["ziel_datei"] = "token"
    e["config"] = "ohne-token"
    if f["bundtool"] == "gefunden":
        return fehler("anmelden.echte_anmeldung")
    if f["ziel"] == "datei-gleich":
        vor += HINWEIS_WIEDERANLAUF
    e["stdout"] = OK_ZEILE
    e["stderr"] = vor
    if f["bundtool"] == "keins":
        e["stderr"] += HINWEIS_BUND
    return e


def main():
    for faelle, erw, menge, rechne in (("faelle", "erwartet", FAELLE, erwartet),
                                       ("faelle-uebernehmen", "erwartet-uebernehmen", FAELLE_UEB, erwartet_ueb)):
        for ordner in (faelle, erw):
            (HIER / ordner).mkdir(exist_ok=True)
        for name, f in menge.items():
            (HIER / faelle / (name + ".json")).write_text(json.dumps(f, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
            (HIER / erw / (name + ".json")).write_text(json.dumps(rechne(f), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print("%d Fälle, %d Fälle der Übernahme" % (len(FAELLE), len(FAELLE_UEB)))


if __name__ == "__main__":
    main()
