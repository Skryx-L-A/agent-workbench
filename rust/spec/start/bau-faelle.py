#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags Start-Struktur (faelle/, erwartet/).

Aufruf: python3 -I rust/spec/start/bau-faelle.py
Formen: `anfrage` (Ansicht an Kern-Dienst, ohne sitzung), `datei` (Kern-Dienst an "Lauf starten", mit sitzung)
und `aufruf` (das Programm "Lauf starten" als Ganzes; erst mit dem Bau prüfbar).
"""
import base64
import copy
import json
import re
from pathlib import Path

HIER = Path(__file__).resolve().parent

# Die erste Zeile jeder Ablehnung des Starters (Exit 3 und 5 bis 9); dazu kommt die zweite Zeile mit dem Teilcode.
STARTER_ZEILE = "lauf.einschluss_fehlt: Einschluss nicht verfuegbar"

# Einstellungsdatei nach Vertrag 2 (rust/spec/einstellungen/vorfuehrung.json), hier eingebettet,
# damit dieser Vertrag allein prüfbar ist.
EINSTELLUNGEN = {
    "fassung": 1,
    "klassen": ["oeffentlich", "intern", "kundendaten"],
    "vorgabestufe": "kundendaten",
    "routen": [
        {"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern"},
        {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten"},
    ],
    "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
}

# Fassung 2 derselben Basis (Vertrag 2, Nachtrag 20.11.): Netzziele an der Cloud-Route, Ziel und Modell an der lokalen Route.
# Abgeleitet, nicht die Vorführdatei; nur die Aufruf-Fälle mit Netzgrenze brauchen sie.
EINSTELLUNGEN_2 = copy.deepcopy(EINSTELLUNGEN)
EINSTELLUNGEN_2["fassung"] = 2
EINSTELLUNGEN_2["routen"][0]["netzziele"] = [
    {"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
     "pfade": [{"methode": "GET", "pfad": "/copilot_internal/" + "*"}]}]
EINSTELLUNGEN_2["routen"][1]["ziel"] = "http://127.0.0.1:11434/v1"
EINSTELLUNGEN_2["routen"][1]["modell"] = "qwen3:8b"
EINSTELLUNGEN_2["routen"][1]["netzziele"] = []

CLOUD = {
    "fassung": 1,
    "harness": "claude",
    "modell": "claude-sonnet-5-5",
    "route": "cloud-beispiel",
    "kunde": "k-beispiel",
    "arbeitsordner": "/home/beispiel/projekt",
}
COPILOT = {
    "fassung": 1,
    "harness": "copilot",
    "modell": "claude-sonnet-5.5",
    "route": "cloud-beispiel",
    "kunde": "k-beispiel",
    "arbeitsordner": "/home/beispiel/projekt",
}
LOKAL = {
    "fassung": 1,
    "harness": "pi",
    "modell": "qwen3:8b",
    "route": "lokal-beispiel",
    "kunde": "k-beispiel",
    "arbeitsordner": "/home/beispiel/projekt",
}


def text(obj):
    return json.dumps(obj, ensure_ascii=False, indent=1) + "\n"


def datei(start, sitzung="s-1"):
    obj = {"fassung": start["fassung"], "sitzung": sitzung}
    obj.update({k: v for k, v in start.items() if k != "fassung"})
    return obj


def mit(basis, aendern):
    obj = copy.deepcopy(basis)
    aendern(obj)
    return obj


FAELLE = []


def fall(name, form, beschreibung, inhalt, code=None, einstellungen=None, route_art=None):
    eintrag = {"beschreibung": beschreibung, "form": form}
    if isinstance(inhalt, bytes):
        eintrag["b64"] = base64.b64encode(inhalt).decode("ascii")
    elif isinstance(inhalt, str):
        eintrag["text"] = inhalt
    else:
        eintrag["text"] = text(inhalt)
    eintrag["einstellungen"] = einstellungen or EINSTELLUNGEN
    if code is None:
        erwartet = {"gueltig": True, "route_art": route_art}
    else:
        erwartet = {"gueltig": False, "code": code}
    FAELLE.append((name, eintrag, erwartet))


A, D = "anfrage", "datei"

# Gültige Beispiele: die zwei Einträge des Durchstichs in beiden Formen
fall("anfrage-gueltig-cloud", A, "claude mit Cloud-Route", CLOUD, route_art="cloud")
fall("anfrage-gueltig-lokal", A, "lokales Modell mit lokaler Route", LOKAL, route_art="lokal")
fall("datei-gueltig-cloud", D, "Startdatei zu claude mit Cloud-Route, Sitzung s-1", datei(CLOUD), route_art="cloud")
fall("datei-gueltig-lokal", D, "Startdatei zum lokalen Modell, Sitzung s-2", datei(LOKAL, "s-2"), route_art="lokal")
fall("anfrage-gueltig-ohne-kunde", A, "kunde null: Sitzung ohne Kundenbezug",
     mit(CLOUD, lambda o: o.update(kunde=None)), route_art="cloud")
fall("datei-gueltig-sitzung-64", D, "Sitzungskennung mit 64 Zeichen ist die Grenze", datei(CLOUD, "s" * 64),
     route_art="cloud")
fall("anfrage-gueltig-ordner-umlaut", A, "Arbeitsordner mit Umlaut und Leerzeichen",
     mit(CLOUD, lambda o: o.update(arbeitsordner="/home/beispiel/Übung 1")), route_art="cloud")
basis = text(CLOUD)
fall("anfrage-gueltig-4096", A, "genau 4096 Byte (aufgefüllt mit Leerzeichen)",
     basis + " " * (4096 - len(basis.encode())), route_art="cloud")

# Nachtrag 20.11.: copilot auf Cloud-Routen, Modellnamen aus Copilot und Ollama
fall("anfrage-gueltig-copilot", A, "copilot mit Cloud-Route", COPILOT, route_art="cloud")
fall("datei-gueltig-copilot", D, "Startdatei zu copilot, Sitzung s-3", datei(COPILOT, "s-3"), route_art="cloud")
fall("anfrage-gueltig-copilot-auto", A, "copilot mit Modell auto (Copilot wählt selbst)",
     mit(COPILOT, lambda o: o.update(modell="auto")), route_art="cloud")
fall("anfrage-gueltig-pi-ollama-name", A, "Ollama-Name mit Namensraum, Unterstrich, Doppelpunkt und Großbuchstaben",
     mit(LOKAL, lambda o: o.update(modell="beispiel_ns/qwen3:4b-instruct-q4_K_M")), route_art="lokal")

# start.modell_route (Stufe 13, nur mit Einstellungen der Fassung 2)
fall("anfrage-gueltig-lokal-fassung-2", A, "pi auf lokaler Route der Fassung 2, Modell gleich dem der Route", LOKAL,
     route_art="lokal", einstellungen=EINSTELLUNGEN_2)
fall("anfrage-modell-nicht-route", A, "pi verlangt ein anderes Modell als die lokale Route",
     mit(LOKAL, lambda o: o.update(modell="qwen3:4b")), "start.modell_route", einstellungen=EINSTELLUNGEN_2)
fall("anfrage-modell-vor-vertraeglichkeit", A, "das Modell der Route geht vor der Verträglichkeit (Stufe 13 vor 15)",
     mit(COPILOT, lambda o: o.update(route="lokal-beispiel")), "start.modell_route", einstellungen=EINSTELLUNGEN_2)
fall("anfrage-route-vor-modell", A, "unbekannte Route geht vor dem Modell der Route",
     mit(LOKAL, lambda o: o.update(route="lokal-anders", modell="qwen3:4b")), "start.route_unbekannt",
     einstellungen=EINSTELLUNGEN_2)
cloud_mit_modell = mit(EINSTELLUNGEN_2, lambda o: o["routen"][0].update(modell="gpt-5.6-sol"))
fall("anfrage-copilot-modell-der-route", A, "Cloud-Route legt das Modell fest, copilot nennt dasselbe",
     mit(COPILOT, lambda o: o.update(modell="gpt-5.6-sol")), route_art="cloud", einstellungen=cloud_mit_modell)
fall("anfrage-copilot-anderes-modell", A, "Cloud-Route legt das Modell fest, copilot nennt ein anderes",
     COPILOT, "start.modell_route", einstellungen=cloud_mit_modell)

# start.zu_gross
fall("anfrage-zu-gross-4097", A, "4097 Byte", basis + " " * (4097 - len(basis.encode())), "start.zu_gross")

# start.json_ungueltig
fall("anfrage-json-bom", A, "UTF-8 mit BOM", b"\xef\xbb\xbf" + basis.encode(), "start.json_ungueltig")
fall("anfrage-json-doppelter-schluessel", A, "route zweimal im Objekt",
     basis.replace('"route": "cloud-beispiel",', '"route": "lokal-beispiel",\n "route": "cloud-beispiel",', 1),
     "start.json_ungueltig")
fall("anfrage-json-syntax", A, "abgeschnittenes JSON", basis[:30], "start.json_ungueltig")
fall("anfrage-json-kein-utf8", A, "Byte 0xFF ist kein UTF-8",
     basis.encode().replace(b"projekt", b"projekt\xff", 1), "start.json_ungueltig")
fall("anfrage-json-kein-objekt", A, "oberste Ebene ist ein Text", '"claude"\n', "start.json_ungueltig")


# Verschachtelungstiefe (Nachtrag 20.11.): das Objekt der obersten Ebene ist Ebene 1; mehr als 127 Ebenen sind
# start.json_ungueltig (Stackschutz, fail closed). Die Form kennt keinen verschachtelten Wert; die Tiefe steckt
# darum unter einem unbekannten Schlüssel.
CLOUD_N = mit(CLOUD, lambda o: o.update(arbeitsordner="/srv/beispiel/projekt"))  # Nachtrag: neue Fälle nutzen /srv/beispiel
basis_n = text(CLOUD_N)


def tief(ebenen, innen="["):
    """Text eines Startobjekts, dessen unbekannter Schlüssel x den Wert bis zur Gesamttiefe `ebenen` verschachtelt."""
    if innen == "{":
        kern = '{"a":' * (ebenen - 1) + "0" + "}" * (ebenen - 1)
    else:
        kern = "[" * (ebenen - 1) + "]" * (ebenen - 1)
    return basis_n.replace('"fassung": 1,', '"fassung": 1,\n "x": ' + kern + ",", 1)


fall("anfrage-json-tiefe-127", A, "Tiefe 127 (Objekt plus 126 Listen) ist die Grenze: JSON gelesen, erst die Form lehnt x ab",
     tief(127), "start.schluessel_unbekannt")
fall("anfrage-json-tiefe-128", A, "Tiefe 128 (Objekt plus 127 Listen)", tief(128), "start.json_ungueltig")
fall("anfrage-json-tiefe-objekte-127", A, "Tiefe 127 aus Objekten: JSON gelesen, erst die Form lehnt x ab",
     tief(127, "{"), "start.schluessel_unbekannt")
fall("anfrage-json-tiefe-objekte-128", A, "Tiefe 128 aus Objekten", tief(128, "{"), "start.json_ungueltig")
fall("anfrage-json-tiefe-1500", A, "1500 Ebenen, das Lesen darf nicht an der Stapeltiefe scheitern", tief(1500),
     "start.json_ungueltig")
fall("datei-json-tiefe-128", D, "Startdatei mit Tiefe 128", tief(128).replace('"fassung": 1,', '"fassung": 1,\n "sitzung": "s-1",', 1),
     "start.json_ungueltig")
fall("anfrage-json-tiefe-vor-sitzung", A, "Tiefe 128 geht vor sitzung_nicht_erlaubt (Stufe 2 vor 3)",
     tief(128).replace('"fassung": 1,', '"fassung": 1,\n "sitzung": "s-1",', 1), "start.json_ungueltig")
fall("anfrage-json-tiefe-in-text", A, "Klammern in einem Text zählen nicht als Tiefe",
     mit(CLOUD, lambda o: o.update(arbeitsordner="/srv/beispiel/" + "[" * 200)), route_art="cloud")

# start.sitzung_nicht_erlaubt
fall("anfrage-sitzung-nicht-erlaubt", A, "die Ansicht schickt eine Sitzungskennung mit", datei(CLOUD),
     "start.sitzung_nicht_erlaubt")
fall("anfrage-sitzung-vor-unbekannt", A, "sitzung geht vor einem unbekannten Schlüssel",
     mit(datei(CLOUD), lambda o: o.update(klasse="intern")), "start.sitzung_nicht_erlaubt")

# start.marker_nicht_erlaubt (Stufe 4) und start.marker_form (Stufe 12): Marker nur in der Startdatei
MARKER = {"liste": ["MK-7Q2", "MK-INTERN"], "ordner": "/home/beispiel/zaehlung"}


def mit_marker(start, marker=MARKER):
    obj = copy.deepcopy(start)
    obj["marker"] = copy.deepcopy(marker)
    return obj


def marker_liste(liste):
    return mit_marker(datei(LOKAL, "s-2"), {"liste": liste, "ordner": "/home/beispiel/zaehlung"})


fall("datei-gueltig-marker", D, "Startdatei mit Markerliste und Zählordner (nur der Kern-Dienst setzt das Feld)",
     mit_marker(datei(LOKAL, "s-2")), route_art="lokal")
fall("datei-gueltig-marker-16", D, "16 Marker zu je 32 Zeichen sind die Grenze",
     marker_liste([f"M{i:02d}" + "X" * 29 for i in range(16)]), route_art="lokal")
fall("anfrage-marker-nicht-erlaubt", A, "die Ansicht schickt marker mit", mit_marker(LOKAL), "start.marker_nicht_erlaubt")
fall("anfrage-sitzung-vor-marker", A, "sitzung geht vor marker", mit_marker(datei(LOKAL, "s-2")),
     "start.sitzung_nicht_erlaubt")
fall("anfrage-marker-vor-unbekannt", A, "marker geht vor einem unbekannten Schlüssel",
     mit(mit_marker(LOKAL), lambda o: o.update(klasse="intern")), "start.marker_nicht_erlaubt")
fall("datei-marker-schluessel-unbekannt", D, "marker mit unbekanntem Schlüssel",
     mit_marker(datei(LOKAL, "s-2"), {**MARKER, "zaehlen": True}), "start.schluessel_unbekannt")
fall("datei-marker-ohne-ordner", D, "marker ohne ordner", mit_marker(datei(LOKAL, "s-2"), {"liste": ["MK-7Q2"]}),
     "start.schluessel_fehlt")
fall("datei-marker-typ", D, "marker als Liste statt Objekt", mit(datei(LOKAL, "s-2"), lambda o: o.update(marker=["MK-7Q2"])),
     "start.typ_falsch")
fall("datei-marker-leer", D, "0 Marker", marker_liste([]), "start.marker_form")
fall("datei-marker-17", D, "17 Marker", marker_liste([f"M{i}" for i in range(17)]), "start.marker_form")
fall("datei-marker-33", D, "Marker mit 33 Zeichen", marker_liste(["M" * 33]), "start.marker_form")
fall("datei-marker-klein", D, "Kleinbuchstabe im Marker", marker_liste(["mk-7q2"]), "start.marker_form")
fall("datei-marker-zeichen", D, "Komma im Marker (die Option trennt mit Komma)", marker_liste(["MK,7Q2"]),
     "start.marker_form")
fall("datei-marker-leerer-marker", D, "leerer Marker", marker_liste([""]), "start.marker_form")
fall("datei-marker-doppelt", D, "derselbe Marker zweimal", marker_liste(["MK-7Q2", "MK-7Q2"]), "start.marker_form")
fall("datei-marker-ordner-relativ", D, "Zählordner relativ",
     mit_marker(datei(LOKAL, "s-2"), {"liste": ["MK-7Q2"], "ordner": "zaehlung"}), "start.marker_form")
fall("datei-marker-ordner-c1", D, "Zählordner mit C1-Steuerzeichen U+0085",
     mit_marker(datei(LOKAL, "s-2"), {"liste": ["MK-7Q2"], "ordner": "/home/bei\u0085spiel"}), "start.marker_form")
fall("datei-ordner-vor-marker", D, "ungültiger Arbeitsordner geht vor der Form der Marker",
     mit(marker_liste([]), lambda o: o.update(arbeitsordner="relativ")), "start.ordner_ungueltig")

# start.schluessel_unbekannt
fall("anfrage-schluessel-unbekannt", A, "Klassen gehören nicht in den Start",
     mit(CLOUD, lambda o: o.update(klasse="intern")), "start.schluessel_unbekannt")
fall("datei-schluessel-unbekannt", D, "Umgebung gehört nicht in den Start",
     mit(datei(CLOUD), lambda o: o.update(umgebung={"ROUTE": "lokal-beispiel"})), "start.schluessel_unbekannt")

# start.schluessel_fehlt
fall("datei-sitzung-fehlt", D, "Startdatei ohne sitzung", CLOUD, "start.schluessel_fehlt")
fall("anfrage-route-fehlt", A, "ohne route (A43)", mit(CLOUD, lambda o: o.pop("route")), "start.schluessel_fehlt")
fall("anfrage-kunde-fehlt", A, "kunde fehlt; ohne Kundenbezug steht null",
     mit(CLOUD, lambda o: o.pop("kunde")), "start.schluessel_fehlt")
fall("anfrage-fassung-fehlt", A, "fassung fehlt", mit(CLOUD, lambda o: o.pop("fassung")), "start.schluessel_fehlt")

# start.typ_falsch
fall("anfrage-typ-route-null", A, "route null", mit(CLOUD, lambda o: o.update(route=None)), "start.typ_falsch")
fall("anfrage-typ-fassung-text", A, "fassung als Text", mit(CLOUD, lambda o: o.update(fassung="1")),
     "start.typ_falsch")
fall("anfrage-typ-modell-zahl", A, "modell als Zahl", mit(CLOUD, lambda o: o.update(modell=5)), "start.typ_falsch")

# start.fassung_unbekannt
fall("anfrage-fassung-2", A, "fassung 2", mit(CLOUD, lambda o: o.update(fassung=2)), "start.fassung_unbekannt")

# start.kennung_form
fall("datei-sitzung-65", D, "Sitzungskennung mit 65 Zeichen", datei(CLOUD, "s" * 65), "start.kennung_form")
fall("anfrage-route-gross", A, "Großbuchstabe in route", mit(CLOUD, lambda o: o.update(route="Cloud-beispiel")),
     "start.kennung_form")
fall("anfrage-kunde-leer", A, "leerer Kunde (ohne Kundenbezug steht null)", mit(CLOUD, lambda o: o.update(kunde="")),
     "start.kennung_form")
fall("anfrage-harness-pfad", A, "harness als Pfad", mit(CLOUD, lambda o: o.update(harness="/usr/bin/claude")),
     "start.kennung_form")

# start.modell_form
fall("anfrage-modell-leer", A, "leeres Modell", mit(CLOUD, lambda o: o.update(modell="")), "start.modell_form")
fall("anfrage-modell-129", A, "Modell mit 129 Zeichen", mit(CLOUD, lambda o: o.update(modell="m" * 129)),
     "start.modell_form")
fall("anfrage-modell-leerzeichen", A, "Modell mit Leerzeichen und Option",
     mit(CLOUD, lambda o: o.update(modell="claude --dangerously")), "start.modell_form")
fall("anfrage-modell-128", A, "Modell mit 128 Zeichen ist die Grenze",
     mit(CLOUD, lambda o: o.update(modell="m" * 128)), route_art="cloud")

# start.ordner_ungueltig
for name, pfad, was in [
    ("anfrage-ordner-doppelstrich", "/home//beispiel", "Pfad mit //"),
    ("anfrage-ordner-punktpunkt", "/home/beispiel/../anders", "Pfad mit .."),
    ("anfrage-ordner-punkt", "/home/./beispiel", "Pfad mit ."),
    ("anfrage-ordner-relativ", "home/beispiel", "relativer Pfad"),
    ("anfrage-ordner-schraegstrich-ende", "/home/beispiel/", "/ am Ende"),
    ("anfrage-ordner-wurzel", "/", "die Wurzel selbst"),
    ("anfrage-ordner-steuerzeichen", "/home/bei\nspiel", "Zeilenumbruch im Pfad"),
    ("anfrage-ordner-zu-lang", "/" + "a" * 1024, "1025 Byte"),
]:
    fall(name, A, was, mit(CLOUD, lambda o, p=pfad: o.update(arbeitsordner=p)), "start.ordner_ungueltig")
fall("anfrage-ordner-1024", A, "Pfad mit 1024 Byte ist die Grenze",
     mit(CLOUD, lambda o: o.update(arbeitsordner="/" + "a" * 1023)), route_art="cloud")
fall("anfrage-ordner-punkte-im-namen", A, "...x und .versteckt sind gewöhnliche Namen",
     mit(CLOUD, lambda o: o.update(arbeitsordner="/home/.versteckt/...x")), route_art="cloud")

# start.harness_unbekannt
fall("anfrage-harness-unbekannt", A, "Harness, den Fassung 1 nicht kennt",
     mit(CLOUD, lambda o: o.update(harness="codex")), "start.harness_unbekannt")

# start.route_unbekannt
fall("anfrage-route-unbekannt", A, "Route steht nicht in der Einstellungsdatei",
     mit(CLOUD, lambda o: o.update(route="cloud-anders")), "start.route_unbekannt")
ohne_cloud = mit(EINSTELLUNGEN, lambda o: o.update(routen=[o["routen"][1]]))
fall("datei-route-unbekannt-andere-einstellungen", D, "Route fehlt in dieser Einstellungsdatei",
     datei(CLOUD), "start.route_unbekannt", einstellungen=ohne_cloud)

# start.kunde_unbekannt
fall("anfrage-kunde-unbekannt", A, "Kunde steht nicht in der Einstellungsdatei",
     mit(CLOUD, lambda o: o.update(kunde="k-anders")), "start.kunde_unbekannt")

# start.harness_route_unvertraeglich
fall("anfrage-claude-lokale-route", A, "claude mit lokaler Route",
     mit(CLOUD, lambda o: o.update(route="lokal-beispiel")), "start.harness_route_unvertraeglich")
fall("anfrage-pi-cloud-route", A, "lokaler Harness mit Cloud-Route",
     mit(LOKAL, lambda o: o.update(route="cloud-beispiel")), "start.harness_route_unvertraeglich")
fall("anfrage-copilot-lokale-route", A, "copilot mit lokaler Route",
     mit(COPILOT, lambda o: o.update(route="lokal-beispiel")), "start.harness_route_unvertraeglich")
fall("anfrage-route-vor-vertraeglichkeit", A, "unbekannte Route geht vor der Verträglichkeit",
     mit(LOKAL, lambda o: o.update(route="cloud-anders")), "start.route_unbekannt")

# Aufruf "Lauf starten": Ausgangslage und erwartetes Ende; erst mit dem Bau prüfbar.
MARKE_LOKAL = {"ordner": "/home/beispiel/projekt", "kennung": "a1b2c3d4e5f60718293a4b5c6d7e8f90", "kunde": "k-beispiel",
               "klasse_hoch": "kundendaten"}
ABGEWIESEN_HOEHER = {"exit": 77, "stderr": "lauf.ordner_hoeher_markiert: Ordner hoeher markiert als die Route",
                     "suche_gestartet": False, "modellweg_gestartet": False}
ABGEWIESEN_KUNDE = {"exit": 77, "stderr": "lauf.ordner_anderer_kunde: Ordner gehoert einem anderen Kunden",
                    "suche_gestartet": False, "modellweg_gestartet": False}
ABGEWIESEN_KAPUTT = {"exit": 78, "stderr": "lauf.ordner_marke_kaputt: Ordnermarke unlesbar oder beschaedigt",
                     "suche_gestartet": False, "modellweg_gestartet": False}

COP_ARGV = ["copilot", "--model", "claude-sonnet-5.5", "--disable-builtin-mcps", "--additional-mcp-config", "@<heim>/.copilot/wb-mcp.json"]

AUFRUFE = [
    ("aufruf-ein-argument", "argv ohne Pfad", {"argv": ["<programm>"]},
     {"exit": 64, "stderr": "lauf.aufruf: genau ein Pfad erwartet", "suche_gestartet": False}),
    ("aufruf-drei-argumente", "argv mit zwei Pfaden", {"argv": ["<programm>", "<start>", "<start>"]},
     {"exit": 64, "stderr": "lauf.aufruf: genau ein Pfad erwartet", "suche_gestartet": False}),
    ("aufruf-datei-fehlt", "Pfad zeigt ins Leere", {"argv": ["<programm>", "<fehlt>"]},
     {"exit": 66, "stderr": "lauf.datei_unlesbar: Startdatei nicht lesbar", "suche_gestartet": False}),
    ("aufruf-modus-0644", "Startdatei mit Modus 0644", {"datei_modus": "0644", "start": datei(CLOUD)},
     {"exit": 66, "stderr": "lauf.datei_modus: Startdatei nicht eigen, nicht 0600 oder keine Datei",
      "suche_gestartet": False}),
    ("aufruf-symlink", "Startdatei ist ein symbolischer Verweis", {"datei_art": "symlink", "start": datei(CLOUD)},
     {"exit": 66, "stderr": "lauf.datei_modus: Startdatei nicht eigen, nicht 0600 oder keine Datei",
      "suche_gestartet": False}),
    ("aufruf-einstellungen-ohne-klasse", "Einstellungsdatei mit Route ohne Klasse",
     {"start": datei(CLOUD), "einstellungen": mit(EINSTELLUNGEN, lambda o: o["routen"][0].pop("klasse"))},
     {"exit": 78, "stderr": "einst.route_ohne_klasse: Route ohne Klasse", "suche_gestartet": False}),
    ("aufruf-start-ungueltig", "Startdatei ohne sitzung", {"start": CLOUD},
     {"exit": 65, "stderr": "start.schluessel_fehlt: Pflichtfeld fehlt", "suche_gestartet": False}),
    ("aufruf-ordner-fehlt", "Arbeitsordner existiert nicht", {"start": datei(CLOUD), "ordner_vorhanden": False},
     {"exit": 65, "stderr": "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner", "suche_gestartet": False}),
    ("aufruf-einschluss-fehlt", "Einschluss nicht verfügbar", {"start": datei(CLOUD), "einschluss": False},
     {"exit": 69, "stderr": "lauf.einschluss_fehlt: Einschluss nicht verfuegbar", "suche_gestartet": False}),
    ("aufruf-suche-stumm", "Such-Prozess meldet keine Bereitschaft binnen 10 s",
     {"start": datei(CLOUD), "suche": "stumm"},
     {"exit": 69, "stderr": "lauf.suche_nicht_bereit: Suche nicht bereit", "suche_gestartet": True,
      "suche_beendet": True}),
    ("aufruf-suche-falsche-zeile", "Such-Prozess meldet eine andere Zeile als {\"bereit\":true}",
     {"start": datei(CLOUD), "suche": "zeile:{\"bereit\": true}"},
     {"exit": 69, "stderr": "lauf.suche_nicht_bereit: Suche nicht bereit", "suche_gestartet": True,
      "suche_beendet": True}),
    ("aufruf-socket-pfad-zu-lang", "Hostpfad des Sockets länger als sun_path (108 Byte unter Linux)",
     {"start": datei(CLOUD), "socket_ordner_laenge": 120},
     {"exit": 69, "stderr": "lauf.suche_fehlt: Suche nicht startbar", "suche_gestartet": False}),
    ("aufruf-harness-fehlt", "Harness-Programm fehlt", {"start": datei(CLOUD), "einstellungen": EINSTELLUNGEN_2, "harness_vorhanden": False},
     {"exit": 69, "stderr": "lauf.harness_fehlt: Harness nicht startbar", "suche_gestartet": True,
      "suche_beendet": True}),
    ("aufruf-gueltig-cloud", "gültiger Start: Suche bereit, dann exec in den Harness",
     {"start": datei(CLOUD), "einstellungen": EINSTELLUNGEN_2},
     {"exec": "claude", "eltern_ist_programm": True, "stderr": "", "suche_gestartet": True,
      "socket_gast": "/run/brain/such.sock", "socket_modus": "0600", "socket_ordner_modus": "0700",
      "socket_ordner_im_einschluss": False, "suche_umgebung": {}, "suche_dumpable": 0,
      "e3_agent": {"signal": "scheitert", "umgebung_lesen": "scheitert", "ptrace": "scheitert"},
      "suche_argv": ["rangkern", "sitzung", "<vault>", "--socket", "<socket>", "--route", "cloud-beispiel",
                     "--sitzung", "s-1", "--einstellungen", "<einstellungen>", "--protokoll", "<protokoll>", "--socket-rechte", "0600"]}),
    # Nachtrag 20.11.: Schritt 0, die eigene Umgebung darf die Prüfung des Modellwegs nicht umlenken
    ("aufruf-gueltig-umgebung-aehnliche-namen", "ähnliche Namen (SSL_CERT_FILES, OPENSSL_CONFX, ssl_cert_file) ändern nichts: der Start geht weiter",
     {"start": datei(CLOUD_N), "einstellungen": EINSTELLUNGEN_2, "umgebung": {"SSL_CERT_FILES": "/srv/beispiel/ca.pem", "OPENSSL_CONFX": "/srv/beispiel/o.cnf",
                                          "ssl_cert_file": "/srv/beispiel/ca.pem"}},
     {"exec": "claude", "eltern_ist_programm": True, "stderr": "", "suche_gestartet": True}),
] + [
    (f"aufruf-tls-umgebung-{name}", f"{was}: Abweisung vor allem anderen",
     {"start": datei(CLOUD_N), "umgebung": umgebung, **extra},
     {"exit": 78, "stderr": "lauf.tls_umgebung: TLS-Umgebung gesetzt", "suche_gestartet": False,
      "modellweg_gestartet": False, "stderr_ohne_wert": True})
    for name, was, umgebung, extra in [
        ("ssl-cert-file", "SSL_CERT_FILE gesetzt", {"SSL_CERT_FILE": "/srv/beispiel/ca.pem"}, {}),
        ("ssl-cert-dir", "SSL_CERT_DIR gesetzt", {"SSL_CERT_DIR": "/srv/beispiel/certs"}, {}),
        ("openssl-conf", "OPENSSL_CONF gesetzt", {"OPENSSL_CONF": "/srv/beispiel/openssl.cnf"}, {}),
        ("leer", "SSL_CERT_FILE gesetzt, aber leer: gesetzt ist gesetzt", {"SSL_CERT_FILE": ""}, {}),
        ("zwei", "zwei Variablen zugleich", {"SSL_CERT_FILE": "/srv/beispiel/ca.pem", "OPENSSL_CONF": "/srv/beispiel/o.cnf"}, {}),
        ("vor-aufruf", "falsches Argument: Schritt 0 geht vor Schritt 1", {"SSL_CERT_DIR": "/srv/beispiel/certs"},
         {"argv": ["<programm>"]}),
        ("vor-datei", "Startdatei fehlt: Schritt 0 geht vor Schritt 2", {"OPENSSL_CONF": "/srv/beispiel/o.cnf"},
         {"argv": ["<programm>", "<fehlt>"]}),
    ]
] + [
    (f"aufruf-tls-umgebung-mutant-{name}", f"Mutante: {was}; die Prüfung muss rot werden",
     {"start": datei(CLOUD_N), "umgebung": umgebung, "mutant": name},
     {"exit": 78, "tls_pruefung": "rot", "befund": befund})
    for name, was, umgebung, befund in [
        ("ohne-pruefung", "das Programm prüft die Umgebung nicht", {"SSL_CERT_FILE": "/srv/beispiel/ca.pem"},
         "Start gelingt, exec in den Harness"),
        ("nur-ssl-cert-file", "nur SSL_CERT_FILE steht in der Liste", {"OPENSSL_CONF": "/srv/beispiel/o.cnf"},
         "Start gelingt mit OPENSSL_CONF"),
        ("leer-gilt-nicht", "eine leere Variable gilt als nicht gesetzt", {"SSL_CERT_DIR": ""},
         "Start gelingt mit leerem SSL_CERT_DIR"),
        ("wert-in-zeile", "die stderr-Zeile nennt den Wert der Variable", {"SSL_CERT_FILE": "/srv/beispiel/ca.pem"},
         "stderr enthält /srv/beispiel/ca.pem"),
    ]
] + [
    # Nachtrag 20.11.: Startform je Harness, Modellweg, Netzgrenze E5 (früher N1) und E4 (Einstellungen der Fassung 2)
    ("aufruf-gueltig-copilot", "copilot auf Cloud-Route: Modellweg kopf-einsetzen bereit, dann exec",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2},
     {"exec": "copilot", "harness_argv": COP_ARGV, "eltern_ist_programm": True,
      "stderr": "", "suche_gestartet": True, "eigener_netzraum": True,
      "harness_umgebung": {"COPILOT_GITHUB_TOKEN": "<platzhalter>", "HTTPS_PROXY": "http://127.0.0.1:8402",
                           "COPILOT_AUTO_UPDATE": "false", "COPILOT_MASCOT_BLINK": "0"},
      "mcp_datei": "<heim>/.copilot/wb-mcp.json", "mcp_server": ["wissenssuche"],
      "modellweg_argv": ["<modellweg>", "--betriebsart", "kopf-einsetzen", "--socket", "<socket>", "--socket-rechte", "0600",
                         "--sitzung", "s-3", "--route", "cloud-beispiel", "--protokoll", "<protokoll>",
                         "<optionen von kopf-einsetzen nach Vertrag 8>"],
      "modellweg_gast": "127.0.0.1:8402",
      "ca": {"zertifikat_im_bundle_des_einschlusses": True, "bundle_des_wirts_unveraendert": True,
             "schluessel_im_einschluss": False, "SSL_CERT_FILE": None, "NODE_EXTRA_CA_CERTS": None},
      "im_einschluss_nicht": ["gh", "dbus_des_wirts", "copilot_config_mit_schluessel"],
      "e5_agent": {"connect_api.cloud.example:443": "gelingt, TLS endet am Modellweg",
                   "get_/copilot_internal/user": "gelingt, Kopf getauscht", "get_/user/repos": "403 mit Protokollzeile",
                   "connect_api.cloud.example:80": "scheitert", "connect_anderer_name:443": "scheitert",
                   "connect_ip_literal": "scheitert", "rechner_loopback_bruecke": "scheitert",
                   "gast_11434": "scheitert", "direkt_ohne_modellweg": "scheitert"},
      "ohne_marker_nicht_in_argv": ["--probelauf", "--marker", "--marker-zaehlung"]}),
    ("aufruf-gueltig-lokal", "pi auf lokaler Route: Modellweg openai-lokal bereit, Anbieter-Definition im Heim, dann exec",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2},
     {"exec": "pi", "harness_argv": ["pi", "--provider", "wb-lokal", "--model", "qwen3:8b"], "eltern_ist_programm": True,
      "stderr": "", "suche_gestartet": True, "eigener_netzraum": True,
      "modellweg_argv": ["<modellweg>", "--betriebsart", "openai-lokal", "--socket", "<socket>", "--socket-rechte", "0600",
                         "--sitzung", "s-2", "--route", "lokal-beispiel", "--protokoll", "<protokoll>",
                         "--ziel", "http://127.0.0.1:11434/v1", "--modell", "qwen3:8b",
                         "--schluessel-datei", "<schluessel-datei>"],
      "modellweg_umgebung": {}, "modellweg_gast": "127.0.0.1:11434",
      "anbieter_datei": "<heim>/.pi/agent/models.json",
      "anbieter": {"providers": {"wb-lokal": {"baseUrl": "http://127.0.0.1:11434/v1", "api": "openai-completions",
                                             "apiKey": "wb-ohne-schluessel", "models": [{"id": "qwen3:8b"}]}}},
      "e5_agent": {"gast_11434": "erreichbar, fuehrt zum Modellweg",
                   "rechner_loopback_bruecke": "scheitert", "rechner_loopback_route_direkt": "scheitert",
                   "gast_8402": "scheitert", "aussen_beliebig": "scheitert"},
      "e4": {"treffer_marke_im_einschluss": 0, "upstream_zaehler": "steigt", "ziel_direkt": "scheitert"},
      "ohne_marker_nicht_in_argv": ["--probelauf", "--marker", "--marker-zaehlung"]}),
    ("aufruf-modellweg-stumm", "Modellweg meldet keine Bereitschaft binnen 10 s",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2, "modellweg": "stumm"},
     {"exit": 69, "stderr": "lauf.modellweg_nicht_bereit: Modellweg nicht bereit", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_beendet": True}),
    ("aufruf-modellweg-fehlt-cloud", "Modellweg der Cloud-Route startet nicht",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "modellweg": "fehlt"},
     {"exit": 69, "stderr": "lauf.modellweg_fehlt: Modellweg nicht startbar", "suche_gestartet": True,
      "suche_beendet": True}),
    ("aufruf-lokal-ohne-ziel", "pi auf lokaler Route einer Einstellungsdatei der Fassung 1 (ohne Ziel)",
     {"start": datei(LOKAL, "s-2")},
     {"exit": 78, "stderr": "lauf.ziel_fehlt: Route ohne Ziel", "suche_gestartet": False}),
    # E4: Der Schlüssel bleibt draußen (Cloud-Route); Marke statt echtem Schlüssel, Attrappe als Upstream
    ("aufruf-e4-cloud", "E4 auf der Cloud-Route: Probe findet die Marke nicht, Upstream zählt sie",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "schluessel": "<marke>", "upstream": "attrappe"},
     {"exec": "copilot", "e4": {"treffer_marke_im_einschluss": 0, "upstream_zaehler": "steigt",
                                "get_/user/repos": "403", "proc_mem_cli": "EPERM",
                                "ziel_direkt": "scheitert"}}),
] + [
    (f"aufruf-e4-mutant-{name}", f"E4-Mutante: {was}; die Prüfung muss rot werden",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "schluessel": "<marke>", "upstream": "attrappe",
      "mutant": name},
     {"exec": "copilot", "e4_pruefung": "rot", "befund": befund})
    for name, was, befund in [
        ("token-in-umgebung", "der echte Schlüssel steht in der Umgebung des Einschlusses", "treffer_marke_im_einschluss >= 1"),
        ("config-eingebunden", "config.json mit der Marke ist eingebunden", "treffer_marke_im_einschluss >= 1"),
        ("dbus-des-wirts", "der D-Bus-Socket des Wirts ist eingebunden", "secret-tool liefert die Marke"),
        ("gh-angemeldet", "ein gh mit Anmeldung ist erreichbar", "gh auth token liefert die Marke"),
        ("ohne-pfadliste", "die Pfadliste für api.cloud.example fehlt", "GET /user/repos gelingt"),
    ]
] + [
    # Starter des Einschlusses (Variante A, Nachtrag 20.11.): was „Lauf starten“ dem Starter auf fd 3 gibt
    (f"aufruf-starter-{name}", was, {"start": datei(mit(route, lambda o: o.update(arbeitsordner="/srv/beispiel/projekt")), sitzung), "einstellungen": EINSTELLUNGEN_2,
                                     "einschluss_variante": "a-starter"},
     {"exec": "werkbank-einschluss", "eltern_ist_programm": True, "stderr": "",
      "starter_argv": ["/usr/lib/werkbank/werkbank-einschluss", "start", "--beschreibung-fd", "3"],
      "lauf_ohne_apparmor_profil": True, "fd3": "lesende einer pipe, die der lauf anlegt und beschreibt",
      "beschreibung": beschreibung})
    for name, was, route, sitzung, beschreibung in [
        ("pi", "pi auf lokaler Route: lokal_weiter und modell_socket gesetzt, dateien mit models.json, PI_OFFLINE und PI_TELEMETRY nur hier",
         LOKAL, "s-2",
         {"harness": "pi", "modell_socket": "<socket>", "lokal_weiter": "<einschluss-weiter>", "cloud": None,
          "dateien": [{"quelle": "<quelle>", "ziel": "<heim>/.pi/agent/models.json"}],
          "umgebung_des_harness_fest": {"PI_OFFLINE": "1", "PI_TELEMETRY": "0"}, "zustandsordner_beim_start": "leer"}),
        ("copilot", "copilot auf Cloud-Route: kein lokal_weiter, dateien mit wb-mcp.json, kein PI_OFFLINE",
         COPILOT, "s-3",
         {"harness": "copilot", "lokal_weiter": None,
          "dateien": [{"quelle": "<quelle>", "ziel": "<heim>/.copilot/wb-mcp.json"}],
          "umgebung_des_harness_nicht": ["PI_OFFLINE", "PI_TELEMETRY"], "zustandsordner_beim_start": "leer",
          "paket": {"quelle": "<cache>/pkg/<plattform>/<fassung>", "nur_lesend_an": "<heim>/.cache/copilot/pkg/<plattform>/<fassung>"}}),
    ]
] + [
    # Seit 21.11.2026 schreibt der Starter bei Exit 3 und 5 bis 9 eine zweite Zeile (Teilcode, Abschnitt „Teilcode“); das Wort
    # folgt aus der Variante des Fehlers im Starter (rust/crates/einschluss/src/starter.rs, Fehler::teil).
    (f"aufruf-starter-{name}", f"Starter: {was}",
     {"start": datei(mit(LOKAL, lambda o: o.update(arbeitsordner="/srv/beispiel/projekt")), "s-2"), "einstellungen": EINSTELLUNGEN_2, "einschluss_variante": "a-starter",
      "beschreibung_aenderung": aenderung},
     {"exit": exit_, "stderr": zeile_, "suche_gestartet": True})
    for name, was, aenderung, exit_, zeile_ in [
        ("ziel-fremd", "ein Ziel in dateien, das nicht models.json des Heims ist", {"dateien": [{"ziel": "<heim>/.bashrc"}]},
         5, STARTER_ZEILE + "\nlauf.einschluss_teil: 5 beschreibung-ungueltig"),
        ("quelle-verweis-nach-aussen", "Quelle in dateien ist ein Verweis, der außerhalb von Arbeits- und Zustandsordner endet",
         {"dateien": [{"quelle": "<verweis-nach-aussen>"}]}, 5, STARTER_ZEILE + "\nlauf.einschluss_teil: 5 quelle-unbrauchbar"),
        ("quelle-modus-0644", "Quelle in dateien hat Modus 0644", {"dateien": [{"quelle": "<quelle-0644>"}]},
         5, STARTER_ZEILE + "\nlauf.einschluss_teil: 5 quelle-unbrauchbar"),
        ("dateien-fehlt", "das Feld dateien fehlt", {"dateien": "fehlt"}, 3,
         STARTER_ZEILE + "\nlauf.einschluss_teil: 3 eingabe-ungueltig"),
        ("fd3-datei", "fd 3 ist eine Datei, keine Pipe", {"fd3": "datei"}, 3,
         STARTER_ZEILE + "\nlauf.einschluss_teil: 3 eingabe-kein-rohr"),
        ("fremdes-argument", "Aufruf mit fremdem Argument", {"argv": ["start", "--bind", "/", "/"]},
         2, "werkbank-einschluss: Aufruf ungueltig"),
    ]
] + [
    # Marker (Nachtrag 8): Zählordner prüfen, beide Prozesse bekommen --marker und --marker-zaehlung
    ("aufruf-gueltig-marker", "Startdatei mit marker: Suche und Modellweg bekommen --probelauf, --marker und --marker-zaehlung",
     {"start": mit_marker(datei(LOKAL, "s-2")), "einstellungen": EINSTELLUNGEN_2},
     {"exec": "pi", "stderr": "", "suche_gestartet": True,
      "suche_argv_ende": ["--probelauf", "--marker", "MK-7Q2,MK-INTERN", "--marker-zaehlung",
                          "/home/beispiel/zaehlung/s-2.suche.json"],
      "modellweg_argv_ende": ["--probelauf", "--marker", "MK-7Q2,MK-INTERN", "--marker-zaehlung",
                              "/home/beispiel/zaehlung/s-2.modellweg.json"]}),
] + [
    (f"aufruf-marker-ordner-{name}", f"Zählordner {was}",
     {"start": mit_marker(datei(LOKAL, "s-2")), "einstellungen": EINSTELLUNGEN_2, "marker_ordner": lage},
     {"exit": 73, "stderr": "lauf.marker_ordner: Zaehlordner fehlt, ist fremd, nicht 0700 oder ein Verweis",
      "suche_gestartet": False})
    for name, was, lage in [
        ("fehlt", "fehlt", "fehlt"),
        ("modus-0755", "hat Modus 0755", "modus:0755"),
        ("fremd", "gehört einer anderen Kennung", "eigentuemer:fremd"),
        ("verweis", "ist ein symbolischer Verweis", "verweis"),
    ]
] + [
    # Ordnermarke (Nachtrag 12, E6): Kunde und höchste Klasse je Arbeitsordner, außerhalb jedes Einschlusses
    (name, was, {"start": start, "einstellungen": EINSTELLUNGEN_2, "ordnermarken": marken,
                 **({"realpath": real} if real else {})}, erwartet)
    for name, was, start, marken, real, erwartet in [
        ("aufruf-marke-neu", "unmarkierter Ordner: die lokale Sitzung setzt Kunde und Klasse der Route",
         datei(LOKAL, "s-2"), [], None,
         {"exec": "pi", "marke_danach": [{"ordner": "/home/beispiel/projekt", "kennung": "<neu>", "kunde": "k-beispiel",
                                          "klasse_hoch": "kundendaten"}]}),
        ("aufruf-marke-copilot-nach-lokal", "copilot im Ordner der lokalen Sitzung",
         datei(COPILOT, "s-3"), [MARKE_LOKAL], None, ABGEWIESEN_HOEHER),
        ("aufruf-marke-anderer-kunde", "zweiter Kunde im Ordner des ersten",
         datei(LOKAL, "s-2"), [{**MARKE_LOKAL, "kunde": "k-anders"}], None, ABGEWIESEN_KUNDE),
        ("aufruf-marke-kein-kunde", "Ordner mit Marke ohne Kunde, Lauf mit Kunde",
         datei(LOKAL, "s-2"), [{**MARKE_LOKAL, "kunde": None}], None, ABGEWIESEN_KUNDE),
        ("aufruf-marke-unterordner", "Start im Unterordner eines markierten Ordners",
         mit(datei(COPILOT, "s-3"), lambda o: o.update(arbeitsordner="/home/beispiel/projekt/teil")),
         [MARKE_LOKAL], None, ABGEWIESEN_HOEHER),
        ("aufruf-marke-elternordner", "Start im Elternordner eines markierten Ordners",
         mit(datei(COPILOT, "s-3"), lambda o: o.update(arbeitsordner="/home/beispiel")),
         [MARKE_LOKAL], None, ABGEWIESEN_HOEHER),
        ("aufruf-marke-symlink", "Start über einen Verweis auf einen markierten Ordner",
         mit(datei(COPILOT, "s-3"), lambda o: o.update(arbeitsordner="/home/beispiel/verweis")),
         [MARKE_LOKAL], "/home/beispiel/projekt", ABGEWIESEN_HOEHER),
        ("aufruf-marke-kaputt", "Markendatei ist kein gültiges JSON",
         datei(LOKAL, "s-2"), "kaputt", None, ABGEWIESEN_KAPUTT),
        ("aufruf-marke-klasse-unbekannt", "Marke nennt eine Klasse, die die Einstellungsdatei nicht kennt",
         datei(LOKAL, "s-2"), [{**MARKE_LOKAL, "klasse_hoch": "geheim"}], None, ABGEWIESEN_KAPUTT),
        ("aufruf-marke-kaputt-vor-kunde", "kaputte Marke geht vor dem anderen Kunden",
         datei(LOKAL, "s-2"), [{**MARKE_LOKAL, "kunde": "k-anders", "klasse_hoch": "geheim"}], None, ABGEWIESEN_KAPUTT),
        ("aufruf-marke-kunde-vor-klasse", "anderer Kunde geht vor zu niedriger Klasse",
         datei(COPILOT, "s-3"), [{**MARKE_LOKAL, "kunde": "k-anders"}], None, ABGEWIESEN_KUNDE),
        ("aufruf-marke-steigt", "lokal nach copilot im selben Ordner ist erlaubt und hebt die Marke",
         datei(LOKAL, "s-2"), [{**MARKE_LOKAL, "klasse_hoch": "intern"}], None,
         {"exec": "pi", "marke_danach": [MARKE_LOKAL]}),
        ("aufruf-marke-gleich", "copilot nach copilot im selben Ordner: erlaubt, die Marke bleibt",
         datei(COPILOT, "s-3"), [{**MARKE_LOKAL, "klasse_hoch": "intern"}], None,
         {"exec": "copilot", "marke_danach": [{**MARKE_LOKAL, "klasse_hoch": "intern"}]}),
        ("aufruf-marke-realpath-ausser-form", "der Verweis des Arbeitsordners führt auf ein Ziel mit Steuerzeichen: kein Pfad nach den Regeln von arbeitsordner, fail closed",
         mit(datei(LOKAL, "s-2"), lambda o: o.update(arbeitsordner="/srv/beispiel/verweis")),
         [], "/srv/beispiel/a\u0001b", ABGEWIESEN_KAPUTT),
        ("aufruf-marke-sinkt-nie", "lokal hat die Marke gehoben; ein späterer copilot-Start senkt sie nicht",
         datei(COPILOT, "s-3"), [MARKE_LOKAL], None, {**ABGEWIESEN_HOEHER, "marke_danach": [MARKE_LOKAL]}),
    ]
] + [
    (f"aufruf-e6-mutant-{name}", f"E6-Mutante: {was}; die Prüfung muss rot werden",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "ordnermarken": [MARKE_LOKAL], "mutant": name},
     {"exit": 77, "e6_pruefung": "rot", "befund": befund})
    for name, was, befund in [
        ("marke-im-ordner", "die Marke liegt im Arbeitsordner und ist für den Agenten schreibbar",
         "der Agent löscht die Marke, danach startet copilot im Ordner"),
        ("nur-exakter-pfad", "nur der exakte Pfad wird verglichen, nicht Vorfahren und Nachfahren",
         "copilot startet im Unterordner und im Elternordner"),
        ("ohne-realpath", "der Pfad wird ohne realpath verglichen", "copilot startet über den Verweis"),
        ("senken-ohne-mensch", "ein Start mit niedrigerer Klasse senkt die Marke", "die Marke sinkt auf intern"),
    ]
]


# --- Nachtrag 2 (20.11.): Werkzeuge ohne Rückfrage, Modell auto, Zielliste, Kennung der Ordnermarke, Befehl ---
KENN_A = "a1b2c3d4e5f60718293a4b5c6d7e8f90"
KENN_B = "0f1e2d3c4b5a69788796a5b4c3d2e1f0"
ZUSTAND = "/var/lib/werkbank/beispiel"
ZIELLISTE_S3 = ZUSTAND + "/sitzungen/s-3.zielliste.json"


# Cloud-Route ohne Netzziel (Vertrag 2 erlaubt die leere Liste; ohne Ziel gäbe es keine DNS-Grenze der CA)
EINST_OHNE_ZIELE = copy.deepcopy(EINSTELLUNGEN_2)
EINST_OHNE_ZIELE["routen"][0]["netzziele"] = []
# Cloud-Route mit 17 Netzzielen: der Modellweg nimmt höchstens 16 (Vertrag 8, Abschnitt 12)
EINST_ZU_VIELE_ZIELE = copy.deepcopy(EINSTELLUNGEN_2)
EINST_ZU_VIELE_ZIELE["routen"][0]["netzziele"] = [
    {"host": f"api{n}.cloud.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": True,
     "pfade": [{"methode": "GET", "pfad": "/copilot_internal/" + "*"}]} for n in range(1, 18)]
# Cloud-Route mit genau 16 Netzzielen: die Grenze selbst gilt
EINST_16_ZIELE = copy.deepcopy(EINST_ZU_VIELE_ZIELE)
EINST_16_ZIELE["routen"][0]["netzziele"] = EINST_16_ZIELE["routen"][0]["netzziele"][:16]
# Cloud-Route mit einem Unterdomänen-Ziel (kopf-einsetzen): die Liste kann es nicht tragen
_UNTER = {"host": "*." + "frei.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": False,
          "pfade": [{"methode": "GET", "pfad": "/" + "*"}]}
EINST_UNTERDOMAENE = copy.deepcopy(EINSTELLUNGEN_2)
EINST_UNTERDOMAENE["routen"][0]["netzziele"] = EINSTELLUNGEN_2["routen"][0]["netzziele"] + [_UNTER]
EINST_NUR_UNTERDOMAENE = copy.deepcopy(EINSTELLUNGEN_2)
EINST_NUR_UNTERDOMAENE["routen"][0]["netzziele"] = [_UNTER]
# dasselbe Ziel mit http-lesen: es bleibt außerhalb der Liste, der Start geht weiter
EINST_UNTERDOMAENE_LESEN = copy.deepcopy(EINSTELLUNGEN_2)
EINST_UNTERDOMAENE_LESEN["routen"][0]["netzziele"] = EINSTELLUNGEN_2["routen"][0]["netzziele"] + [
    {"host": "*." + "frei.example", "port": 80, "zugriff": "http-lesen"}]
# Dateien neben der Startdatei, die ein Lauf mit Cloud-Route und copilot schreibt
SITZUNGSDATEIEN = ["<startordner>/" + n for n in ("ca.key", "ca.crt", "ca-bundle.pem", "suche.pid", "modellweg.pid")]


def einst_werkzeuge(werte, route_modell=None):
    obj = copy.deepcopy(EINSTELLUNGEN_2)
    obj["harness"] = {"copilot": {"werkzeuge_ohne_rueckfrage": werte}}
    if route_modell is not None:
        obj["routen"][0]["modell"] = route_modell
    return obj


def argv_cop(modell_args=("--model", "claude-sonnet-5.5"), werte=()):
    zusatz = [x for w in werte for x in ("--allow-tool", w)]
    return ["copilot", *modell_args, "--disable-builtin-mcps", "--additional-mcp-config",
            "@<heim>/.copilot/wb-mcp.json", *zusatz]


def ok_cop(argv):
    return {"exec": "copilot", "harness_argv": argv, "eltern_ist_programm": True, "suche_gestartet": True}


NICHT_ALLE = ["--allow-all", "--allow-all-tools", "--allow-all-paths", "--allow-all-urls", "--yolo"]
AUFRUFE_N2 = [
    ("aufruf-copilot-werkzeuge-alle-drei", "harness.copilot nennt shell, write, such-mcp: je ein --allow-tool am Ende",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge(["shell", "write", "such-mcp"])},
     {**ok_cop(argv_cop(werte=["shell", "write", "such-mcp"])), "argv_ohne": NICHT_ALLE}),
    ("aufruf-copilot-werkzeuge-reihenfolge", "die Reihenfolge der Datei gilt",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge(["such-mcp", "shell"])},
     ok_cop(argv_cop(werte=["such-mcp", "shell"]))),
    ("aufruf-copilot-werkzeuge-eins", "ein Wert: genau ein --allow-tool",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge(["write"])},
     ok_cop(argv_cop(werte=["write"]))),
    ("aufruf-copilot-werkzeuge-leer", "leere Liste: die Rückfrage bleibt, kein --allow-tool",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge([])},
     {**ok_cop(argv_cop()), "argv_ohne": ["--allow-tool", *NICHT_ALLE]}),
    ("aufruf-pi-werkzeuge-ohne-wirkung", "der Block ändert den Startbefehl von pi nicht",
     {"start": datei(LOKAL, "s-2"), "einstellungen": einst_werkzeuge(["shell", "write", "such-mcp"])},
     {"exec": "pi", "harness_argv": ["pi", "--provider", "wb-lokal", "--model", "qwen3:8b"],
      "argv_ohne": ["--allow-tool"]}),
    ("aufruf-copilot-auto", "Route ohne modell, Start mit modell auto: --model auto --auto-tier efficiency",
     {"start": mit(datei(COPILOT, "s-3"), lambda o: o.update(modell="auto")), "einstellungen": EINSTELLUNGEN_2},
     ok_cop(argv_cop(("--model", "auto", "--auto-tier", "efficiency")))),
    ("aufruf-copilot-auto-mit-werkzeugen", "auto und Werkzeuge zugleich: erst das Modell, am Ende die Werte",
     {"start": mit(datei(COPILOT, "s-3"), lambda o: o.update(modell="auto")),
      "einstellungen": einst_werkzeuge(["shell"])},
     ok_cop(argv_cop(("--model", "auto", "--auto-tier", "efficiency"), ["shell"]))),
    ("aufruf-copilot-festes-modell-ohne-auto-tier", "festes Modell: kein --auto-tier",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2},
     {**ok_cop(argv_cop()), "argv_ohne": ["--auto-tier"]}),
] + [
    (f"aufruf-copilot-werkzeuge-wert-{name}", f"Wert {wert!r} in der Einstellungsdatei: Ladefehler, kein Start",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge([wert])},
     {"exit": 78, "stderr": "einst.werkzeug_wert: Werkzeugwert nicht erlaubt", "suche_gestartet": False})
    for name, wert in [("allow-all-tools", "allow-all-tools"), ("allow-all", "allow-all"), ("yolo", "yolo"),
                       ("allow-all-paths", "allow-all-paths"), ("allow-all-urls", "allow-all-urls")]
] + [
    (f"aufruf-werkzeuge-mutant-{name}", f"Mutante: {was}; die Prüfung muss rot werden",
     {"start": datei(COPILOT, "s-3"), "einstellungen": einst_werkzeuge(["shell"]), "mutant": name},
     {"exit": 77, "werkzeuge_pruefung": "rot", "befund": befund})
    for name, was, befund in [
        ("allow-all-immer", "das Programm hängt --allow-all-tools an", "argv enthält --allow-all-tools"),
        ("wert-aus-startdatei", "die Werte kommen aus der Startdatei statt der Einstellungsdatei",
         "ein Wert aus der Startdatei erscheint in argv"),
        ("pi-bekommt-werte", "auch pi bekommt --allow-tool", "argv von pi enthält --allow-tool"),
        ("unbekannt-wird-verworfen", "ein unbekannter Wert wird still verworfen statt Ladefehler",
         "Start gelingt mit yolo in der Datei"),
        ("ohne-block-erlaubt-alles", "ohne Block entfällt die Rückfrage", "argv enthält --allow-tool ohne Block"),
    ]
] + [
    # Zielliste je Cloud-Sitzung (Entscheidung)
    ("aufruf-zielliste-cloud", "Cloud-Route: Zielliste im Zustandsordner, 0600, O_EXCL, vor dem Modellweg, bleibt liegen",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND},
     {"exec": "copilot",
      "zielliste": {"pfad": ZIELLISTE_S3, "modus": "0600", "o_excl": True, "vor_modellweg": True,
                    "bleibt_nach_ende": True, "im_einschluss": False, "enthaelt_schluessel": False},
      "modellweg_argv_enthaelt": ["--zielliste", ZIELLISTE_S3]}),
    ("aufruf-zielliste-lokal-keine", "lokale Route: keine Zielliste",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND},
     {"exec": "pi", "zielliste": None}),
    ("aufruf-zielliste-existiert", "die Datei der Sitzung gibt es schon: 73, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "vorhanden": [ZIELLISTE_S3]},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False, "datei_unveraendert": [ZIELLISTE_S3]}),
    ("aufruf-zielliste-sitzungen-fehlt", "Ordner sitzungen fehlt: 73, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "sitzungen_ordner": "fehlt"},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False}),
    ("aufruf-zielliste-leer", "Cloud-Route ohne Netzziel: 73, keine Zielliste, keine CA, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_OHNE_ZIELE, "zustandsordner": ZUSTAND},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False, "zielliste": None,
      "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-zielliste-zu-viele", "Cloud-Route mit 17 Netzzielen: 73, keine Zielliste, keine CA, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_ZU_VIELE_ZIELE, "zustandsordner": ZUSTAND},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False, "zielliste": None,
      "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-zielliste-16-ziele", "Cloud-Route mit genau 16 Netzzielen: der Start geht weiter",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_16_ZIELE, "zustandsordner": ZUSTAND},
     {"exec": "copilot", "datei_da": SITZUNGSDATEIEN}),
    # Unterdomänen-Ziele (Entscheidung vom 21.11.2026): die Liste kennt kein `*`; nie still ohne das Ziel starten
    ("aufruf-zielliste-unterdomaene", "Cloud-Route mit einem einfachen und einem Unterdomänen-Ziel (kopf-einsetzen): 73, keine Zielliste, keine CA, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_UNTERDOMAENE, "zustandsordner": ZUSTAND},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False, "zielliste": None,
      "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-zielliste-unterdomaene-allein", "Cloud-Route nur mit einem Unterdomänen-Ziel (kopf-einsetzen): 73, keine Zielliste, keine CA, kein Modellweg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_NUR_UNTERDOMAENE, "zustandsordner": ZUSTAND},
     {"exit": 73, "stderr": "lauf.zielliste: Zielliste nicht schreibbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False, "zielliste": None,
      "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-zielliste-unterdomaene-http-lesen", "ein Unterdomänen-Ziel mit http-lesen neben einem einfachen kopf-einsetzen-Ziel: der Start geht weiter, das Ziel steht nicht in der Liste",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINST_UNTERDOMAENE_LESEN, "zustandsordner": ZUSTAND},
     {"exec": "copilot", "datei_da": SITZUNGSDATEIEN, "zielliste": {"pfad": ZIELLISTE_S3, "bleibt_nach_ende": True},
      "zielliste_hosts": ["api.cloud.example"]}),
    # HOME im Einschluss (Entscheidung vom 21.11.2026): HOME ist <heim>, ein leeres tmpfs; darunter nur der Zustandsordner
    ("aufruf-heim-pi", "pi: HOME ist <heim>, <heim> ein tmpfs, darunter nur .pi",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2},
     {"exec": "pi", "heim_im_einschluss": {"home": "<heim>", "tmpfs": "<heim>", "unter_heim_nur": ".pi"}}),
    ("aufruf-heim-copilot", "copilot: HOME ist <heim>, <heim> ein tmpfs, darunter nur .copilot",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND},
     {"exec": "copilot", "heim_im_einschluss": {"home": "<heim>", "tmpfs": "<heim>", "unter_heim_nur": ".copilot", "unter_heim_nur_lesend": ".cache/copilot/pkg/<plattform>/<fassung>"}}),
    # Eigene Konfiguration (Entscheidung vom 21.11.2026): zwei feste Gründe, Code 78, kein Kind
    ("aufruf-konfig-fehlt", "die Konfigurationsdatei fehlt: 78, kein Kind",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2, "konfig": "fehlt"},
     {"exit": 78, "stderr": "lauf.konfig_fehlt: Konfigurationsdatei fehlt", "suche_gestartet": False,
      "modellweg_gestartet": False}),
    # Nachbesserung 23.11.: die Einstellungsdatei, deren Pfad die Konfiguration nennt, lässt sich nicht öffnen (Schritt 3)
    ("aufruf-einst-unlesbar", "die Einstellungsdatei fehlt, obwohl die Konfiguration sie nennt: 78, kein Kind",
     {"start": mit(datei(LOKAL, "s-2"), lambda o: o.update(arbeitsordner="/srv/beispiel/projekt")),
      "einstellungen": EINSTELLUNGEN_2, "konfig": "einstellungen-fehlt"},
     {"exit": 78, "stderr": "lauf.einst_unlesbar: Einstellungsdatei nicht lesbar", "suche_gestartet": False,
      "modellweg_gestartet": False}),
] + [
    (f"aufruf-konfig-kaputt-{name}", f"die Konfigurationsdatei {was}: 78, kein Kind",
     {"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2, "konfig": art},
     {"exit": 78, "stderr": "lauf.konfig_kaputt: Konfigurationsdatei nicht lesbar oder ungueltig",
      "suche_gestartet": False, "modellweg_gestartet": False})
    for name, was, art in [
        ("kein-json", "ist kein JSON", "kein-json"),
        ("schluessel-fehlt", "hat keinen Schlüssel vault", "schluessel-fehlt"),
        ("schluessel-unbekannt", "hat einen unbekannten Schlüssel", "schluessel-unbekannt"),
        ("pfad-relativ", "nennt einen relativen Pfad", "pfad-relativ"),
        ("modus-gruppe-schreibbar", "ist für die Gruppe schreibbar (0664)", "modus:0664"),
        ("modus-andere-schreibbar", "ist für andere schreibbar (0646)", "modus:0646"),
        ("zu-gross", "ist 65537 Byte gross", "zu-gross"),
        ("ordner", "ist ein Ordner", "ordner"),
        ("modus-000", "hat den Modus 0000 und lässt sich nicht öffnen", "modus:0000"),
        ("eigentuemer-fremd", "gehört einem fremden Nutzer", "eigentuemer-fremd"),
    ]
] + [
    ("aufruf-aufraeumen-modellweg-fehlt-cloud", "Modellweg der Cloud-Route startet nicht: CA, Bundle und PID-Dateien sind weg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "modellweg": "fehlt"},
     {"exit": 69, "stderr": "lauf.modellweg_fehlt: Modellweg nicht startbar", "suche_gestartet": True,
      "suche_beendet": True, "datei_fehlt": SITZUNGSDATEIEN,
      "zielliste": {"pfad": ZIELLISTE_S3, "bleibt_nach_ende": True}}),
    ("aufruf-aufraeumen-modellweg-stumm-cloud", "Modellweg der Cloud-Route meldet nicht bereit: CA, Bundle und PID-Dateien sind weg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "modellweg": "stumm"},
     {"exit": 69, "stderr": "lauf.modellweg_nicht_bereit: Modellweg nicht bereit", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_beendet": True, "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-aufraeumen-harness-fehlt-cloud", "Harness fehlt nach den PID-Dateien: alle Sitzungsdateien sind weg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "harness_vorhanden": False},
     {"exit": 69, "stderr": "lauf.harness_fehlt: Harness nicht startbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_beendet": True, "datei_fehlt": SITZUNGSDATEIEN}),
    ("aufruf-aufraeumen-fremde-datei-bleibt", "Eine vorhandene ca.key wird abgelehnt und nicht gelöscht; was der Lauf selbst schrieb, ist weg",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "vorhanden": ["<startordner>/ca.key"]},
     {"exit": 69, "stderr": "lauf.modellweg_fehlt: Modellweg nicht startbar", "suche_gestartet": True,
      "suche_beendet": True, "modellweg_gestartet": False,
      "datei_unveraendert": ["<startordner>/ca.key"],
      "datei_fehlt": ["<startordner>/ca.crt", "<startordner>/ca-bundle.pem", "<startordner>/suche.pid",
                      "<startordner>/modellweg.pid"]}),
    ("aufruf-startordner-belegt-suche-pid", "Neben der Startdatei liegt schon suche.pid einer früheren Sitzung: 73, kein Kind, die Datei bleibt",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "vorhanden": ["<startordner>/suche.pid"]},
     {"exit": 73, "stderr": "lauf.startordner_belegt: Startordner enthaelt Dateien einer fruehen Sitzung",
      "suche_gestartet": False, "datei_unveraendert": ["<startordner>/suche.pid"]}),
    ("aufruf-startordner-belegt-modellweg-pid", "Neben der Startdatei liegt schon modellweg.pid einer früheren Sitzung: 73, kein Kind, die Datei bleibt",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "vorhanden": ["<startordner>/modellweg.pid"]},
     {"exit": 73, "stderr": "lauf.startordner_belegt: Startordner enthaelt Dateien einer fruehen Sitzung",
      "suche_gestartet": False, "datei_unveraendert": ["<startordner>/modellweg.pid"]}),
    ("aufruf-aufraeumen-erfolg-behaelt", "Gelingt der Start, bleiben CA, Bundle und PID-Dateien für den Einschluss liegen",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND},
     {"exec": "copilot", "datei_da": SITZUNGSDATEIEN}),
    ("aufruf-zielliste-bleibt-nach-fehler", "der Modellweg startet nicht: die Zielliste bleibt liegen",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND,
      "modellweg": "fehlt"},
     {"exit": 69, "stderr": "lauf.modellweg_fehlt: Modellweg nicht startbar", "suche_gestartet": True,
      "suche_beendet": True, "zielliste": {"pfad": ZIELLISTE_S3, "bleibt_nach_ende": True}}),
] + [
    (f"aufruf-zielliste-mutant-{name}", f"Mutante: {was}; die Prüfung muss rot werden",
     {"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2, "zustandsordner": ZUSTAND, "mutant": name},
     {"exit": 77, "zielliste_pruefung": "rot", "befund": befund})
    for name, was, befund in [
        ("im-arbeitsordner", "die Zielliste liegt im Arbeitsordner", "der Agent liest oder ändert die Zielliste"),
        ("nach-modellweg", "die Zielliste entsteht nach dem Start des Modellwegs", "der Modellweg startet ohne Datei"),
        ("loeschen-am-ende", "die Zielliste wird am Sitzungsende gelöscht", "die Suche findet keine Zielliste"),
        ("ohne-o-excl", "die Zielliste wird ohne O_EXCL geschrieben", "eine vorhandene Datei wird überschrieben"),
    ]
]

# Ordnermarke: Kennung und Befehl
M_KENN = {"ordner": "/home/beispiel/projekt", "kennung": KENN_A, "kunde": "k-beispiel", "klasse_hoch": "kundendaten"}
KENN_FEHLER = {"exit": 78, "stderr": "lauf.ordner_marke_kennung: Ordnermarke mit ungueltiger oder doppelter Kennung",
               "suche_gestartet": False, "modellweg_gestartet": False}
PROT = "<protokollordner>/ordnermarke-<JJJJ-MM>.jsonl"


def befehl(argv, marken, erwartet, **mehr):
    return {"argv": ["werkbank-ordnermarke", *argv], "einstellungen": EINSTELLUNGEN, "ordnermarken": marken,
            "protokollordner": "<protokollordner>", **mehr}, erwartet


def zeile(name, alt_klasse, neu_klasse, kunde_alt="k-beispiel", kunde_neu=None):
    return {"aktionsname": name, "ergebnis": "ausgefuehrt", "fassung": 2, "klasse_alt": alt_klasse,
            "klasse_neu": neu_klasse, "kunde_alt": kunde_alt, "kunde_neu": kunde_neu, "marke": KENN_A}


AUFRUFE_N2 += [
    (n, w, *a) for n, w, a in [
        ("aufruf-marke-kennung-fehlt", "Marke ohne Kennung: Ladefehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{k: v for k, v in M_KENN.items() if k != "kennung"}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-form", "Kennung mit Großbuchstaben: Ladefehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "kennung": KENN_A.upper()}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-31", "Kennung mit 31 Zeichen: Ladefehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "kennung": KENN_A[:31]}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-doppelt", "zwei Marken mit derselben Kennung: Ladefehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [M_KENN, {**M_KENN, "ordner": "/home/beispiel/anderes"}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-vor-kunde", "Kennungsfehler geht vor dem anderen Kunden",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "kunde": "k-anders", "kennung": "xyz"}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-vor-klasse", "Kennungsfehler geht vor zu niedriger Klasse",
         ({"start": datei(COPILOT, "s-3"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{k: v for k, v in M_KENN.items() if k != "kennung"}]}, KENN_FEHLER)),
        ("aufruf-marke-kaputt-vor-kennung", "unbekannte Klasse geht vor dem Kennungsfehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "klasse_hoch": "geheim", "kennung": "xyz"}]},
          {**KENN_FEHLER, "exit": 78, "stderr": "lauf.ordner_marke_kaputt: Ordnermarke unlesbar oder beschaedigt"})),
        ("aufruf-marke-kennung-unverwandte", "die Kennung einer nicht verwandten Marke ist falsch: auch das ist ein Ladefehler",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "ordner": "/home/beispiel/weit-weg", "kennung": "zz"}]}, KENN_FEHLER)),
        ("aufruf-marke-kennung-neu-nie-doppelt", "der Zufall liefert zuerst eine vorhandene Kennung: das Programm zieht neu",
         ({"start": mit(datei(LOKAL, "s-2"), lambda o: o.update(arbeitsordner="/home/beispiel/zweites")),
           "einstellungen": EINSTELLUNGEN_2, "ordnermarken": [{**M_KENN, "ordner": "/home/beispiel/erstes"}],
           "zufall": [KENN_A, KENN_B]},
          {"exec": "pi", "marke_danach": [{**M_KENN, "ordner": "/home/beispiel/erstes"},
                                          {**M_KENN, "ordner": "/home/beispiel/zweites", "kennung": KENN_B}]})),
        ("aufruf-marke-kennung-bleibt", "steigt die Klasse einer Marke, bleibt ihre Kennung",
         ({"start": datei(LOKAL, "s-2"), "einstellungen": EINSTELLUNGEN_2,
           "ordnermarken": [{**M_KENN, "klasse_hoch": "intern"}], "zufall": [KENN_B]},
          {"exec": "pi", "marke_danach": [M_KENN]})),
    ]
]
_L, _E = befehl(["loesen", "/home/beispiel/projekt"], [M_KENN],
                {"exit": 0, "marke_danach": [], "protokoll": {"datei": PROT, "quelle": "ordnermarke",
                                                              "zeilen": [zeile("ordnermarke.geloest", "kundendaten", None)],
                                                              "enthaelt_pfad": False}})
_S, _SE = befehl(["senken", "/home/beispiel/projekt", "--klasse", "intern"], [M_KENN],
                 {"exit": 0, "marke_danach": [{**M_KENN, "klasse_hoch": "intern"}],
                  "protokoll": {"datei": PROT, "quelle": "ordnermarke",
                                "zeilen": [zeile("ordnermarke.gesenkt", "kundendaten", "intern", kunde_neu="k-beispiel")],
                                "enthaelt_pfad": False}})
_Z, _ZE = befehl(["zeigen", "/home/beispiel/projekt"], [M_KENN],
                 {"exit": 0, "marke_danach": [M_KENN], "protokoll": {"zeilen": []}})
AUFRUFE_N2 += [
    ("aufruf-befehl-loesen", "loesen: Marke weg, Zeile ordnermarke.geloest ohne Pfad", _L, _E),
    ("aufruf-befehl-senken", "senken auf intern: Kennung bleibt, Zeile ordnermarke.gesenkt ohne Pfad", _S, _SE),
    ("aufruf-befehl-zeigen", "zeigen ändert nichts und schreibt keine Zeile", _Z, _ZE),
]
for _n, _w, _a, _m, _ex in [
    ("aufruf-befehl-loesen-protokoll-fehlt", "loesen: das Protokoll ist nicht schreibbar, die Marke bleibt",
     ["loesen", "/home/beispiel/projekt"], [M_KENN],
     {"exit": 74, "stderr": "ordnermarke.protokoll: Protokollzeile nicht schreibbar", "marke_danach": [M_KENN]}),
    ("aufruf-befehl-senken-protokoll-fehlt", "senken: das Protokoll ist nicht schreibbar, die Marke bleibt",
     ["senken", "/home/beispiel/projekt", "--klasse", "intern"], [M_KENN],
     {"exit": 74, "stderr": "ordnermarke.protokoll: Protokollzeile nicht schreibbar", "marke_danach": [M_KENN]}),
]:
    _l, _e = befehl(_a, _m, _ex, protokoll="nicht-schreibbar")
    AUFRUFE_N2.append((_n, _w, _l, _e))
for _n, _w, _a, _ex in [
    ("aufruf-befehl-keine-marke", "loesen für einen Ordner ohne Marke", ["loesen", "/home/beispiel/anderes"],
     {"exit": 66, "stderr": "ordnermarke.keine_marke: Keine Marke fuer diesen Ordner", "marke_danach": [M_KENN]}),
    ("aufruf-befehl-senken-gleiche-klasse", "senken auf die Klasse der Marke: abgelehnt, keine Zeile",
     ["senken", "/home/beispiel/projekt", "--klasse", "kundendaten"],
     {"exit": 64, "stderr": "ordnermarke.aufruf: Aufruf ungueltig", "marke_danach": [M_KENN]}),
    ("aufruf-befehl-senken-hoehere-klasse", "senken auf eine höhere Klasse: abgelehnt, keine Zeile",
     ["senken", "/home/beispiel/projekt", "--klasse", "kundendaten"],
     {"exit": 64, "stderr": "ordnermarke.aufruf: Aufruf ungueltig", "marke_danach": [{**M_KENN, "klasse_hoch": "intern"}]}),
    ("aufruf-befehl-senken-klasse-unbekannt", "senken auf eine unbekannte Klasse",
     ["senken", "/home/beispiel/projekt", "--klasse", "geheim"],
     {"exit": 64, "stderr": "ordnermarke.aufruf: Aufruf ungueltig", "marke_danach": [M_KENN]}),
    ("aufruf-befehl-ohne-klasse", "senken ohne --klasse", ["senken", "/home/beispiel/projekt"],
     {"exit": 64, "stderr": "ordnermarke.aufruf: Aufruf ungueltig", "marke_danach": [M_KENN]}),
]:
    _marken = [{**M_KENN, "klasse_hoch": "intern"}] if "hoehere" in _n else [M_KENN]
    _l, _e = befehl(_a, _marken, {**_ex, "protokoll": {"zeilen": []}})
    AUFRUFE_N2.append((_n, _w, _l, _e))
_l, _e = befehl(["loesen", "/home/beispiel/projekt"], [{k: v for k, v in M_KENN.items() if k != "kennung"}],
                {"exit": 78, "stderr": "ordnermarke.marke_kaputt: Ordnermarken unlesbar oder beschaedigt",
                 "protokoll": {"zeilen": []}})
AUFRUFE_N2.append(("aufruf-befehl-kennung-fehlt", "loesen bei einer Marke ohne Kennung: Fehler, keine Zeile", _l, _e))
# Die eigene Konfiguration des Befehls (Entscheidung vom 21.11.2026: dieselben Bedingungen wie bei lauf.konfig_*)
for _n, _w, _art, _code, _grund in [
    ("aufruf-befehl-konfig-fehlt", "die eigene Konfiguration fehlt: nichts wird geändert", "fehlt",
     "ordnermarke.konfig_fehlt", "Konfigurationsdatei fehlt"),
    ("aufruf-befehl-konfig-kaputt", "die eigene Konfiguration ist kein JSON: nichts wird geändert", "kein-json",
     "ordnermarke.konfig_kaputt", "Konfigurationsdatei nicht lesbar oder ungueltig"),
    ("aufruf-befehl-konfig-einstellungen-fehlt", "die Einstellungsdatei fehlt: nichts wird geändert", "einstellungen-fehlt",
     "ordnermarke.konfig_kaputt", "Konfigurationsdatei nicht lesbar oder ungueltig"),
]:
    _l, _e = befehl(["loesen", "/home/beispiel/projekt"], [M_KENN],
                    {"exit": 78, "stderr": f"{_code}: {_grund}", "marke_danach": [M_KENN], "protokoll": {"zeilen": []}},
                    konfig=_art)
    AUFRUFE_N2.append((_n, _w, _l, _e))
AUFRUFE_N2 += [
    (f"aufruf-befehl-mutant-{name}", f"Mutante: {was}; die Prüfung muss rot werden",
     {"argv": ["werkbank-ordnermarke", "loesen", "/home/beispiel/projekt"], "einstellungen": EINSTELLUNGEN,
      "ordnermarken": [M_KENN], "mutant": name},
     {"exit": 73, "befehl_pruefung": "rot", "befund": befund})
    for name, was, befund in [
        ("marke-ohne-zeile", "der Befehl ändert die Marke und schreibt die Zeile nicht, wenn das Protokoll ausfällt",
         "die Marke ist weg, im Protokoll steht nichts"),
        ("pfad-in-zeile", "die Zeile trägt den Pfad des Ordners", "die Zeile enthält /home/beispiel/projekt"),
        ("hash-des-pfads", "die Zeile trägt einen Hash des Pfads statt der Kennung", "marke ist sha256 des Pfads"),
        ("zeile-fuer-zeigen", "zeigen schreibt eine Zeile", "ordnermarke.geloest nach zeigen"),
    ]
]
AUFRUFE += AUFRUFE_N2


# Start-Struktur Fassung 2 (Nachtrag 26.11.2026, L10): Felder effort und name. Das Crate start kennt Fassung 2 noch nicht;
# die Fälle stehen darum in faelle-ausstehend/ und erwartet-ausstehend/. Jede Erwartung rechnet die Referenz unten.
EFFORTS = {  # Vertrag 2, Block effort (gemessen 26.11.2026 mit --help)
    "claude": ("low", "medium", "high", "xhigh", "max"),
    "copilot": ("none", "minimal", "low", "medium", "high", "xhigh", "max"),
    "pi": ("off", "minimal", "low", "medium", "high", "xhigh", "max"),
}
ROUTENART = {"claude": "cloud", "copilot": "cloud", "pi": "lokal"}
EFFORT_FORM = re.compile(r"^[a-z]{1,16}$")
# Verboten im Namen: C0, C1, DEL, Zeilen- und Absatztrenner, Richtungszeichen (Bidi), BOM und Zeichen ohne Breite.
NAME_VERBOTEN = re.compile("[\u0000-\u001f\u007f-\u009f\u2028\u2029\u200b-\u200f\u202a-\u202e\u2066-\u2069\ufeff]")


def name_gueltig(n):
    return n is None or (isinstance(n, str) and 1 <= len(n) <= 64 and not NAME_VERBOTEN.search(n)
                         and n == n.strip() and n.strip() != "")


def erlaubte_efforts(einst, harness, route, modell):
    klasse = next(r["klasse"] for r in einst["routen"] if r["schluessel"] == route)
    passend = [e for e in einst.get("effort", {}).get("grenzen", []) if e["harness"] == harness
               and e.get("klasse", klasse) == klasse and e.get("modell", modell) == modell]
    if not passend:
        return []
    return [w for w in EFFORTS[harness] if all(w in e["erlaubt"] for e in passend)]


def code_f2(start, einst):
    """Referenz der Stufen von Fassung 2 für Starts, deren übrige Felder gültig sind (Stufen 5 bis 18)."""
    if start.get("fassung") == 1:
        return "start.schluessel_unbekannt" if ("effort" in start or "name" in start) else None
    if "effort" not in start or "name" not in start:
        return "start.schluessel_fehlt"
    e, n = start["effort"], start["name"]
    if not (e is None or isinstance(e, str)) or not (n is None or isinstance(n, str)):
        return "start.typ_falsch"
    if not name_gueltig(n):
        return "start.name_form"
    if e is not None and not EFFORT_FORM.match(e):
        return "start.effort_form"
    if start["harness"] not in EFFORTS:
        return "start.harness_unbekannt"
    if e is not None and e not in EFFORTS[start["harness"]]:
        return "start.effort_unbekannt"
    routen = {r["schluessel"]: r for r in einst["routen"]}
    if start["route"] not in routen:
        return "start.route_unbekannt"
    r = routen[start["route"]]
    if "modell" in r and r["modell"] != start["modell"]:
        return "start.modell_route"
    if start["kunde"] is not None and start["kunde"] not in [k["schluessel"] for k in einst["kunden"]]:
        return "start.kunde_unbekannt"
    if ROUTENART[start["harness"]] != r["art"]:
        return "start.harness_route_unvertraeglich"
    if e is not None and e not in erlaubte_efforts(einst, start["harness"], start["route"], start["modell"]):
        return "start.effort_grenze"
    return None


EINST_EFFORT = copy.deepcopy(EINSTELLUNGEN_2)
EINST_EFFORT["effort"] = {"grenzen": [
    {"harness": "copilot", "erlaubt": ["low", "medium", "high"]},
    {"harness": "copilot", "modell": "gpt-6-luna", "erlaubt": ["low", "medium"]},
    {"harness": "pi", "klasse": "kundendaten", "erlaubt": ["off", "low"]}]}
COP2 = dict(COPILOT, fassung=2, effort="medium", name="Auswertung Q4")
PI2 = dict(LOKAL, fassung=2, effort="low", name=None)
AUSSTEHEND = []


# Beispielordner der neuen Fälle (Projektentscheid, 27.11.2026): /heim/beispiel statt des Ordners unter home, damit die
# Prüfung auf Pfade von Personen im Diff keinen Treffer findet. Die Fälle in faelle/ behalten ihren Ordner.
BEISPIEL_ALT, BEISPIEL_NEU = "/home" + "/beispiel/", "/heim/beispiel/"


def heim(start):
    o = start.get("arbeitsordner")
    if isinstance(o, str) and o.startswith(BEISPIEL_ALT):
        return dict(start, arbeitsordner=BEISPIEL_NEU + o[len(BEISPIEL_ALT):])
    return start


def ausstehend(name, form, beschreibung, start, soll, einst=EINST_EFFORT):
    start = heim(start)
    ist = code_f2(start, einst)
    if ist != soll:
        raise SystemExit(f"FEHLER {name}: Referenz {ist!r}, Tabelle {soll!r}")
    inhalt = datei(start) if form == D else start
    eintrag = {"beschreibung": beschreibung, "form": form, "text": text(inhalt), "einstellungen": einst}
    erwartet = ({"gueltig": True, "route_art": ROUTENART[start["harness"]]} if soll is None
                else {"gueltig": False, "code": soll})
    AUSSTEHEND.append((name, eintrag, erwartet))


ausstehend("anfrage-f2-gueltig-copilot", A, "Fassung 2: copilot mit Effort medium und Namen", COP2, None)
ausstehend("datei-f2-gueltig-copilot", D, "Startdatei der Fassung 2 mit Effort und Namen, Sitzung s-1", COP2, None)
ausstehend("anfrage-f2-gueltig-pi", A, "Fassung 2: pi mit Effort low ohne Namen", PI2, None)
ausstehend("anfrage-f2-gueltig-ohne-effort", A, "Fassung 2: effort null; der Harness nimmt seine eigene Vorgabe, auch ohne Block",
           dict(COP2, effort=None), None, einst=EINSTELLUNGEN_2)
ausstehend("anfrage-f2-gueltig-name-umlaut", A, "Name mit Umlaut, Leerzeichen und 64 Zeichen",
           dict(COP2, name="Übung " + "ä" * 58), None)
ausstehend("anfrage-f2-effort-fehlt", A, "Fassung 2 ohne effort", {k: v for k, v in COP2.items() if k != "effort"},
           "start.schluessel_fehlt")
ausstehend("anfrage-f2-name-fehlt", A, "Fassung 2 ohne name", {k: v for k, v in COP2.items() if k != "name"},
           "start.schluessel_fehlt")
ausstehend("anfrage-f1-mit-effort", A, "Fassung 1 kennt effort nicht", dict(COPILOT, effort="low"), "start.schluessel_unbekannt")
ausstehend("anfrage-f2-effort-zahl", A, "effort als Zahl", dict(COP2, effort=2), "start.typ_falsch")
ausstehend("anfrage-f2-name-liste", A, "name als Liste", dict(COP2, name=["a"]), "start.typ_falsch")
for _n, _wert, _was in [("leer", "", "leerer Name"), ("65", "n" * 65, "65 Zeichen"), ("zeilenumbruch", "a\nb", "Zeilenumbruch"),
                        ("tab", "a\tb", "Tabulator"), ("c1", "a\u0085b", "C1-Steuerzeichen NEL"),
                        ("bidi", "a\u202eb", "Richtungszeichen RLO"), ("breitenlos", "a\u200bb", "Zeichen ohne Breite"),
                        ("rand", " Name", "Leerzeichen am Anfang"), ("nur-leer", "   ", "nur Leerzeichen")]:
    ausstehend(f"anfrage-f2-name-{_n}", A, f"Name: {_was}", dict(COP2, name=_wert), "start.name_form")
ausstehend("anfrage-f2-effort-form-gross", A, "Effort groß geschrieben", dict(COP2, effort="High"), "start.effort_form")
ausstehend("anfrage-f2-effort-form-option", A, "Effort mit Bindestrich wie eine Option", dict(COP2, effort="--max"),
           "start.effort_form")
ausstehend("anfrage-f2-effort-form-leer", A, "leerer Effort", dict(COP2, effort=""), "start.effort_form")
ausstehend("anfrage-f2-name-vor-effort", A, "Name geht vor Effort (Stufe 11a vor 11b)", dict(COP2, name="", effort="High"),
           "start.name_form")
ausstehend("anfrage-f2-effort-unbekannt", A, "off ist ein Wert von pi, nicht von copilot", dict(COP2, effort="off"),
           "start.effort_unbekannt")
ausstehend("anfrage-f2-effort-unbekannt-claude", A, "none kennt claude nicht",
           dict(CLOUD, fassung=2, effort="none", name=None), "start.effort_unbekannt")
ausstehend("anfrage-f2-effort-unbekannt-vor-route", A, "unbekannter Effort geht vor unbekannter Route (13a vor 14)",
           dict(COP2, effort="ultra", route="cloud-anders"), "start.effort_unbekannt")
ausstehend("anfrage-f2-gueltig-effort-high", A, "copilot mit high: die Grenze erlaubt low, medium und high", dict(COP2, effort="high"),
           None)
ausstehend("anfrage-f2-effort-grenze-modell", A, "copilot mit gpt-6-luna: die Grenze dieses Modells erlaubt nur low und medium",
           dict(COP2, effort="high", modell="gpt-6-luna"), "start.effort_grenze")
ausstehend("anfrage-f2-effort-ohne-grenze", A, "claude hat keine Grenze: kein Effort wählbar",
           dict(CLOUD, fassung=2, effort="low", name=None), "start.effort_grenze")
ausstehend("anfrage-f2-effort-ohne-block", A, "Einstellungen ohne Block effort: kein Effort wählbar (fail closed)",
           dict(COP2, effort="low"), "start.effort_grenze", einst=EINSTELLUNGEN_2)
ausstehend("anfrage-f2-effort-grenze-pi", A, "pi in kundendaten mit medium: erlaubt sind off und low", dict(PI2, effort="medium"),
           "start.effort_grenze")
ausstehend("anfrage-f2-vertraeglich-vor-grenze", A, "Harness und Route unverträglich geht vor der Grenze (17 vor 18)",
           dict(PI2, route="cloud-beispiel", modell="qwen3:8b", effort="max"), "start.harness_route_unvertraeglich")


def main():
    for teil in ("faelle", "erwartet"):
        ordner = HIER / teil
        ordner.mkdir(exist_ok=True)
        for alt in ordner.glob("*.json"):
            alt.unlink()
    for name, eintrag, erwartet in FAELLE:
        (HIER / "faelle" / f"{name}.json").write_text(text(eintrag), encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(text(erwartet), encoding="utf-8")
    for name, beschreibung, lage, erwartet in AUFRUFE:
        eintrag = {"beschreibung": beschreibung, "form": "aufruf",
                   "lage": {"argv": ["<programm>", "<start>"], "datei_modus": "0600", "datei_art": "datei",
                            "einstellungen": EINSTELLUNGEN, "ordner_vorhanden": True, "einschluss": True,
                            "suche": "bereit", "harness_vorhanden": True, **lage}}
        (HIER / "faelle" / f"{name}.json").write_text(text(eintrag), encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(text(erwartet), encoding="utf-8")
    for teil in ("faelle-ausstehend", "erwartet-ausstehend"):
        ordner = HIER / teil
        ordner.mkdir(exist_ok=True)
        for alt in ordner.glob("*.json"):
            alt.unlink()
    for name, eintrag, erwartet in AUSSTEHEND:
        (HIER / "faelle-ausstehend" / f"{name}.json").write_text(text(eintrag), encoding="utf-8")
        (HIER / "erwartet-ausstehend" / f"{name}.json").write_text(text(erwartet), encoding="utf-8")
    print(f"{len(FAELLE) + len(AUFRUFE)} Fälle und {len(AUSSTEHEND)} ausstehende Fälle geschrieben")


if __name__ == "__main__":
    main()
