#!/usr/bin/env python3
"""Erzeugt Schema und sichtbare Fälle des Vertrags Einschluss unter Windows (faelle/, erwartet/).

Aufruf: python3 -I rust/spec/windows-einschluss/bau-faelle.py
Formen: `konfig` (Einschlussdatei der Installation), `steuer` (eine Anfrage an die Steuer-Pipe des
Plattformdienstes), `helfer` (eine Zeile an den Admin-Helfer), `aufruf` (das Programm "Lauf starten" und
`einschluss-loeschen` als Ganzes) und `vm` (Außentest für die Windows-VM). `aufruf` und `vm` sind erst mit
dem Bau prüfbar.
"""
import base64
import copy
import json
from pathlib import Path

HIER = Path(__file__).resolve().parent

KENNUNG = r"^[a-z0-9][a-z0-9._-]{0,63}$"
# Ein Segment eines Windows-Pfads: keine reservierten Zeichen, kein Steuerzeichen, kein Gerätename,
# endet nicht auf Punkt oder Leerzeichen (damit sind auch "." und ".." ausgeschlossen).
_GERAET = r"(?:[cC][oO][nN]|[pP][rR][nN]|[aA][uU][xX]|[nN][uU][lL]|[cC][oO][mM][0-9]|[lL][pP][tT][0-9])"
_ZEICHEN = r'[^<>:"/\\|?*\x00-\x1f\x7f]'
SEGMENT = r"(?!" + _GERAET + r"(?:\.|\\|$))" + _ZEICHEN + r"*" + r'[^<>:"/\\|?*\x00-\x1f\x7f. ]'
WINPFAD = r"^[A-Za-z]:(?:\\" + SEGMENT + r")+$"
ORDNER = r"^[0-9a-f]{64}$"

SCHEMA = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://werkbank.invalid/spec/windows-einschluss/einschluss.schema.json",
    "title": "Einschluss unter Windows, Fassung 1",
    "$defs": {
        "kennung": {"type": "string", "pattern": KENNUNG},
        "winpfad": {"type": "string", "minLength": 4, "maxLength": 1024, "pattern": WINPFAD},
        "ordner": {"type": "string", "pattern": ORDNER},
        "pfadliste": {"type": "array", "minItems": 1, "maxItems": 16, "uniqueItems": True,
                      "items": {"$ref": "#/$defs/winpfad"}},
        "konfig": {
            "type": "object",
            "additionalProperties": False,
            "required": ["fassung", "projektwurzeln", "programmordner", "shell"],
            "properties": {
                "fassung": {"const": 1},
                "projektwurzeln": {"$ref": "#/$defs/pfadliste"},
                "programmordner": {"$ref": "#/$defs/pfadliste"},
                "shell": {"const": "powershell"},
            },
        },
        "steuer": {
            "oneOf": [
                {"$ref": "#/$defs/op_projekt_neu"},
                {"$ref": "#/$defs/op_lauf_start"},
                {"$ref": "#/$defs/op_lauf_ende"},
                {"$ref": "#/$defs/op_projekt_loeschen"},
                {"$ref": "#/$defs/op_peer"},
            ]
        },
        "op_projekt_neu": {
            "type": "object", "additionalProperties": False, "required": ["op", "ordner"],
            "properties": {"op": {"const": "projekt_neu"}, "ordner": {"$ref": "#/$defs/ordner"}},
        },
        "op_lauf_start": {
            "type": "object", "additionalProperties": False, "required": ["op", "ordner", "sitzung", "pid", "ports"],
            "properties": {"op": {"const": "lauf_start"}, "ordner": {"$ref": "#/$defs/ordner"},
                           "sitzung": {"$ref": "#/$defs/kennung"},
                           "pid": {"type": "integer", "minimum": 1, "maximum": 4294967295},
                           "ports": {"type": "array", "minItems": 1, "maxItems": 4, "uniqueItems": True,
                                     "items": {"type": "integer", "minimum": 1, "maximum": 65535}}},
        },
        "op_lauf_ende": {
            "type": "object", "additionalProperties": False, "required": ["op", "sitzung"],
            "properties": {"op": {"const": "lauf_ende"}, "sitzung": {"$ref": "#/$defs/kennung"}},
        },
        "op_projekt_loeschen": {
            "type": "object", "additionalProperties": False, "required": ["op", "ordner"],
            "properties": {"op": {"const": "projekt_loeschen"}, "ordner": {"$ref": "#/$defs/ordner"}},
        },
        "op_peer": {
            "type": "object", "additionalProperties": False, "required": ["op"],
            "properties": {
                "op": {"const": "peer"},
                "tcp": {"type": "object", "additionalProperties": False, "required": ["lokal", "fern"],
                        "properties": {"lokal": {"type": "integer", "minimum": 1, "maximum": 65535},
                                       "fern": {"type": "integer", "minimum": 1, "maximum": 65535}}},
                "pid": {"type": "integer", "minimum": 1, "maximum": 4294967295},
            },
            "oneOf": [{"required": ["tcp"], "not": {"required": ["pid"]}},
                      {"required": ["pid"], "not": {"required": ["tcp"]}}],
        },
    },
}

KONFIG = {
    "fassung": 1,
    "projektwurzeln": ["C:\\Users\\beispiel\\Projekte"],
    "programmordner": ["C:\\Users\\beispiel\\AppData\\Local\\Programs\\werkbank-harness"],
    "shell": "powershell",
}
O = "3f" * 32  # Ordner-Schlüssel, 64 Hex-Zeichen
SID_A = "S-1-15-2-1111111111-2222222222-3333333333-1444444444-1555555555-3666666666-2777777777"


def text(obj):
    return json.dumps(obj, ensure_ascii=False, indent=1) + "\n"


def mit(basis, aendern):
    obj = copy.deepcopy(basis)
    aendern(obj)
    return obj


FAELLE = {}


def fall(name, beschreibung, form, erw, **felder):
    assert name not in FAELLE, name
    FAELLE[name] = ({"beschreibung": beschreibung, "form": form, **felder}, erw)


def ok(**mehr):
    return {"gueltig": True, **mehr}


def nein(code):
    return {"gueltig": False, "code": code}


# ---- konfig ---------------------------------------------------------------------------------------
def k(name, beschr, obj=None, code=None, roh=None):
    feld = {"b64": base64.b64encode(roh).decode()} if roh is not None else {"text": text(obj)}
    fall("konfig-" + name, beschr, "konfig", nein("einschluss." + code) if code else ok(), **feld)


k("gueltig", "die Vorführkonfiguration", KONFIG)
k("gueltig-zwei-wurzeln", "zwei Projektwurzeln auf zwei Laufwerken",
  mit(KONFIG, lambda o: o["projektwurzeln"].append("D:\\Arbeit")))
k("gueltig-punkte-im-namen", "Segmente mit Punkt im Namen sind gewöhnlich",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["C:\\Users\\beispiel\\.werkbank\\pro.jekte"])))
k("gueltig-umlaut", "Umlaut im Pfad", mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["C:\\Users\\beispiel\\Übungen"])))
k("gueltig-geraet-im-namen", "CONFIG und COM10 sind keine Gerätenamen",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["C:\\CONFIG\\COM10"])))
k("zu-gross", "mehr als 4096 Byte", roh=(text(KONFIG)[:-2] + " " * 4096 + "}\n").encode(), code="zu_gross")
k("json-bom", "BOM vor dem Objekt", roh=b"\xef\xbb\xbf" + text(KONFIG).encode(), code="json_ungueltig")
k("json-doppelt", "doppelter Schlüssel shell",
  roh=text(KONFIG).replace('"shell": "powershell"', '"shell": "powershell", "shell": "powershell"').encode(),
  code="json_ungueltig")
k("json-liste", "oberste Ebene ist eine Liste", roh=b"[]\n", code="json_ungueltig")
k("schluessel-unbekannt-port", "der Port gehört nicht in die Konfiguration des Starters",
  mit(KONFIG, lambda o: o.__setitem__("modellweg_port", 41001)), code="schluessel_unbekannt")
k("schluessel-unbekannt-sid", "eine SID gehört nicht in die Konfiguration",
  mit(KONFIG, lambda o: o.__setitem__("sid", SID_A)), code="schluessel_unbekannt")
k("schluessel-fehlt-shell", "shell fehlt", mit(KONFIG, lambda o: o.pop("shell")), code="schluessel_fehlt")
k("typ-wurzeln-text", "projektwurzeln als Text",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", "C:\\Projekte")), code="typ_falsch")
k("fassung-2", "Fassung 2", mit(KONFIG, lambda o: o.__setitem__("fassung", 2)), code="fassung_unbekannt")
k("liste-leer", "keine Projektwurzel", mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", [])), code="liste_grenze")
k("liste-17", "17 Programmordner",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", [f"C:\\P\\p{i}" for i in range(17)])), code="liste_grenze")
k("liste-doppelt", "derselbe Pfad zweimal",
  mit(KONFIG, lambda o: o["projektwurzeln"].append(o["projektwurzeln"][0])), code="liste_grenze")
for name, pfad, beschr in [
    ("pfad-laufwerk", "C:\\", "Laufwerkswurzel allein"),
    ("pfad-relativ", "Users\\beispiel", "relativer Pfad"),
    ("pfad-unc", "\\\\server\\freigabe\\p", "UNC-Pfad"),
    ("pfad-geraetepfad", "\\\\?\\C:\\Projekte", "Gerätepfad mit \\\\?\\"),
    ("pfad-schraegstrich", "C:/Daten/beispiel/Projekte", "Schrägstrich statt Backslash"),
    ("pfad-punktpunkt", "C:\\Users\\beispiel\\..\\anders", "Segment .."),
    ("pfad-punkt", "C:\\Users\\.\\beispiel", "Segment ."),
    ("pfad-ende-backslash", "C:\\Users\\beispiel\\", "endet mit Backslash"),
    ("pfad-doppel-backslash", "C:\\Users\\\\beispiel", "leeres Segment"),
    ("pfad-ende-punkt", "C:\\Users\\beispiel.", "Segment endet mit Punkt"),
    ("pfad-ende-leerzeichen", "C:\\Users\\beispiel ", "Segment endet mit Leerzeichen"),
    ("pfad-stream", "C:\\Users\\beispiel\\p:daten", "Doppelpunkt (Datenstrom) im Segment"),
    ("pfad-geraet-nul", "C:\\Projekte\\NUL", "Gerätename NUL"),
    ("pfad-geraet-com1-endung", "C:\\Projekte\\com1.txt", "Gerätename mit Endung"),
    ("pfad-steuerzeichen", "C:\\Projekte\\a\tb", "Steuerzeichen"),
    ("pfad-stern", "C:\\Projekte\\*", "Platzhalter"),
]:
    k(name, beschr, mit(KONFIG, lambda o, p=pfad: o.__setitem__("projektwurzeln", [p])), code="pfad_form")
k("pfad-zu-lang", "1025 Zeichen", mit(KONFIG, lambda o: o.__setitem__(
    "projektwurzeln", ["C:\\" + "\\".join(["a" * 100] * 10) + "\\" + "b" * 12])), code="pfad_form")
k("shell-bash", "Git Bash ist nicht erlaubt (A117)", mit(KONFIG, lambda o: o.__setitem__("shell", "bash")),
  code="shell_unbekannt")
# Stufen, die das Dateisystem oder die Person brauchen: erst mit dem Bau prüfbar.
k("ueberlappt-programm-unter-wurzel", "Programmordner unter einer Projektwurzel",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["C:\\Users\\beispiel\\Projekte\\werkzeug"])),
  code="pfad_ueberlappt")
k("ueberlappt-wurzel-unter-programm", "Projektwurzel unter einem Programmordner",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln",
                                      ["C:\\Users\\beispiel\\AppData\\Local\\Programs\\werkbank-harness\\p"])),
  code="pfad_ueberlappt")
k("ueberlappt-gross-klein", "derselbe Ordner in anderer Schreibweise",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["c:\\users\\BEISPIEL\\projekte"])), code="pfad_ueberlappt")
k("geschuetzt-profil", "Projektwurzel ist das Profil der Person",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["C:\\Users\\beispiel"])), code="pfad_geschuetzt")
k("geschuetzt-ueber-profil", "Projektwurzel liegt über dem Profil",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["C:\\Users"])), code="pfad_geschuetzt")
k("geschuetzt-windows", "Programmordner ist der Windows-Ordner",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["C:\\Windows"])), code="pfad_geschuetzt")

# ---- steuer ---------------------------------------------------------------------------------------
def s(name, beschr, obj=None, code=None, roh=None, antwort=None, lage=None):
    feld = {"b64": base64.b64encode(roh).decode()} if roh is not None else {"text": json.dumps(obj, ensure_ascii=False) + "\n"}
    if lage is not None:
        feld["lage"] = lage
    erw = nein("dienst." + code) if code else ok(antwort=antwort)
    fall("steuer-" + name, beschr, "steuer", erw, **feld)


NEU = {"op": "projekt_neu", "ordner": O}
START = {"op": "lauf_start", "ordner": O, "sitzung": "s-1", "pid": 4242, "ports": [50101, 50102]}
ENDE = {"op": "lauf_ende", "sitzung": "s-1"}
LOESCHEN = {"op": "projekt_loeschen", "ordner": O}
PEER_TCP = {"op": "peer", "tcp": {"lokal": 41001, "fern": 50123}}
PEER_PID = {"op": "peer", "pid": 4242}

s("projekt-neu", "Projekt anlegen", NEU, antwort={"ok": True, "sid": "<S-1-15-2-...>"})
s("projekt-neu-wiederholt", "zweites projekt_neu für denselben Ordner ist wirkungsgleich", NEU,
  antwort={"ok": True, "sid": "<dieselbe SID>"}, lage={"projekt": "besteht"})
s("lauf-start", "Lauf starten mit angehaltenem Agenten", START, antwort={"ok": True}, lage={"projekt": "besteht"})
s("lauf-ende", "Ende eines Laufs entfernt seine Erlaubnisse", ENDE,
  antwort={"ok": True}, lage={"laeufe": ["s-1"], "danach": "keine Erlaubnis für 50101 und 50102, Sperre steht"})
s("lauf-ende-anderer-lebt", "Ende eines Laufs, ein zweiter lebt: dessen Erlaubnisse bleiben", ENDE,
  antwort={"ok": True}, lage={"laeufe": ["s-1", "s-2"], "danach": "Erlaubnisse von s-2 stehen"})
s("lauf-start-ein-port", "Lauf mit nur dem Modellweg", mit(START, lambda o: o.__setitem__("ports", [50101])),
  antwort={"ok": True}, lage={"projekt": "besteht"})
s("projekt-loeschen", "Löschen ohne lebenden Lauf", LOESCHEN, antwort={"ok": True}, lage={"laeufe": []})
s("projekt-loeschen-wiederholt", "Löschen ist wiederholbar, auch ohne Sublayer", LOESCHEN, antwort={"ok": True},
  lage={"projekt": "geloescht"})
s("peer-tcp", "Peer einer Loopback-Verbindung", PEER_TCP,
  antwort={"ok": True, "pid": 4242, "start": "<FILETIME>", "sid": "<S-1-15-2-...>", "sitzung": "s-1"})
s("peer-pid", "Peer über die PID (Such-Pipe)", PEER_PID,
  antwort={"ok": True, "pid": 4242, "start": "<FILETIME>", "sid": "<S-1-15-2-...>", "sitzung": "s-1"})
s("zu-gross", "mehr als 4096 Byte", roh=(json.dumps(NEU)[:-1] + " " * 4096 + "}\n").encode(), code="zu_gross")
s("json-syntax", "Syntaxfehler", roh=b'{"op": "projekt_neu",\n', code="json_ungueltig")
s("json-doppelt", "doppelter Schlüssel op",
  roh=('{"op": "peer", "op": "projekt_neu", "ordner": "' + O + '"}\n').encode(), code="json_ungueltig")
s("json-zwei-zeilen", "zwei Objekte in einer Nachricht",
  roh=(json.dumps(NEU) + json.dumps(NEU) + "\n").encode(), code="json_ungueltig")
s("op-unbekannt", "op setze_ausnahme gibt es nicht", {"op": "setze_ausnahme", "ordner": O}, code="op_unbekannt")
s("op-fehlt", "ohne op", {"ordner": O}, code="op_unbekannt")
s("op-gross", "op in Großbuchstaben", {"op": "PROJEKT_NEU", "ordner": O}, code="op_unbekannt")
s("schluessel-port", "der Starter wählt den Port nicht", mit(START, lambda o: o.__setitem__("port", 41001)),
  code="schluessel_unbekannt")
s("schluessel-sid", "der Starter nennt keine SID", mit(NEU, lambda o: o.__setitem__("sid", SID_A)),
  code="schluessel_unbekannt")
s("schluessel-pfad", "der Starter schickt keinen Pfad",
  mit(NEU, lambda o: o.__setitem__("arbeitsordner", "C:\\Users\\beispiel\\Projekte\\p")), code="schluessel_unbekannt")
s("schluessel-fehlt-pid", "lauf_start ohne pid", mit(START, lambda o: o.pop("pid")), code="schluessel_fehlt")
s("peer-beides", "peer mit tcp und pid", {"op": "peer", "tcp": {"lokal": 41001, "fern": 50123}, "pid": 4242},
  code="schluessel_fehlt")
s("peer-nichts", "peer ohne tcp und ohne pid", {"op": "peer"}, code="schluessel_fehlt")
s("typ-pid-text", "pid als Text", mit(START, lambda o: o.__setitem__("pid", "4242")), code="typ_falsch")
s("typ-pid-bruch", "pid als 4242.0", roh=json.dumps(START).replace("4242", "4242.0").encode() + b"\n", code="typ_falsch")
s("form-sitzung", "Sitzung mit Großbuchstaben", mit(START, lambda o: o.__setitem__("sitzung", "S-1")), code="form")
s("form-ordner-kurz", "Ordner-Schlüssel mit 63 Zeichen", mit(NEU, lambda o: o.__setitem__("ordner", O[:63])), code="form")
s("form-ordner-gross", "Ordner-Schlüssel in Großbuchstaben", mit(NEU, lambda o: o.__setitem__("ordner", O.upper())),
  code="form")
s("grenze-ports-leer", "ports leer", mit(START, lambda o: o.__setitem__("ports", [])), code="grenze")
s("grenze-ports-fuenf", "fünf Ports", mit(START, lambda o: o.__setitem__("ports", [50101, 50102, 50103, 50104, 50105])),
  code="grenze")
s("grenze-ports-doppelt", "derselbe Port zweimal", mit(START, lambda o: o.__setitem__("ports", [50101, 50101])),
  code="grenze")
s("grenze-ports-null", "Port 0", mit(START, lambda o: o.__setitem__("ports", [0])), code="grenze")
s("typ-ports-zahl", "ports als einzelne Zahl", mit(START, lambda o: o.__setitem__("ports", 50101)), code="typ_falsch")
s("schluessel-fehlt-ports", "lauf_start ohne ports", mit(START, lambda o: o.pop("ports")), code="schluessel_fehlt")
s("grenze-pid-null", "pid 0", mit(START, lambda o: o.__setitem__("pid", 0)), code="grenze")
s("grenze-pid-gross", "pid über 2^32-1", mit(START, lambda o: o.__setitem__("pid", 4294967296)), code="grenze")
s("grenze-port-null", "Port 0", {"op": "peer", "tcp": {"lokal": 0, "fern": 50123}}, code="grenze")
s("grenze-port-gross", "Port 65536", {"op": "peer", "tcp": {"lokal": 41001, "fern": 65536}}, code="grenze")
# Stufen mit Lage: erst mit dem Bau prüfbar.
s("aufrufer-peer-fremd", "peer eines Prozesses einer anderen Person", PEER_TCP, code="peer_unbekannt",
  lage={"peer": "Prozess einer anderen Person"})
s("projekt-unbekannt", "lauf_start ohne projekt_neu", START, code="projekt_unbekannt", lage={"projekt": "fehlt"})
s("sitzung-besteht", "dieselbe Sitzung zweimal", START, code="sitzung_besteht", lage={"laeufe": ["s-1"]})
s("sitzung-unbekannt", "lauf_ende ohne Lauf", ENDE, code="sitzung_unbekannt", lage={"laeufe": []})
s("prozess-laeuft", "der Agent ist nicht angehalten", START, code="prozess_falsch",
  lage={"agent": "nicht angehalten"})
s("prozess-ohne-container", "der Prozess hat nicht die SID des Projekts", START, code="prozess_falsch",
  lage={"agent": "Prozess der Person ohne AppContainer"})
s("prozess-anderes-projekt", "der Prozess hat die SID eines anderen Projekts", START, code="prozess_falsch",
  lage={"agent": "AppContainer eines anderen Projekts"})
s("prozess-anderer-eltern", "der Prozess ist nicht vom Aufrufer erzeugt", START, code="prozess_falsch",
  lage={"agent": "AppContainer des Projekts, erzeugt von einem anderen Prozess"})
s("port-fremd-bruecke", "der Starter fordert den Port der Browser-Brücke an", mit(START, lambda o: o.__setitem__(
  "ports", [50101, 8443])), code="port_fremd", lage={"8443": "Hörer des Kern-Dienstes (Browser-Brücke)",
                                                    "50101": "Weiterleiter des Aufrufers"})
s("port-fremd-dienst", "Port eines fremden lokalen Dienstes", mit(START, lambda o: o.__setitem__("ports", [5432])),
  code="port_fremd", lage={"5432": "Hörer eines fremden Dienstes"})
s("port-fremd-nur-v4", "Weiterleiter hört nur auf 127.0.0.1, nicht auf [::1]", mit(START, lambda o: o.__setitem__(
  "ports", [50101])), code="port_fremd", lage={"50101": "Weiterleiter des Aufrufers, nur 127.0.0.1"})
s("port-fremd-anderer-lauf", "Port ist schon für einen anderen Lauf erlaubt", mit(START, lambda o: o.__setitem__(
  "ports", [50201])), code="port_fremd", lage={"50201": "Sitzungsport von s-2", "laeufe": ["s-2"]})
s("port-fremd-kein-hoerer", "auf dem Port hört niemand", mit(START, lambda o: o.__setitem__("ports", [50999])),
  code="port_fremd", lage={"50999": "frei"})
s("lauf-lebt", "Löschen mit lebendem Lauf", LOESCHEN, code="lauf_lebt", lage={"laeufe": ["s-1"]})
s("ausnahme-bleibt", "Löschen bei ausgefallenem Helfer: Sperre bleibt", LOESCHEN, code="ausnahme_bleibt",
  lage={"laeufe": [], "helfer": "gestoppt"})
s("helfer-fehlt-neu", "Projekt anlegen bei ausgefallenem Helfer: Sperre steht, keine Ausnahme", NEU,
  code="helfer_fehlt", lage={"helfer": "gestoppt"})
s("peer-unbekannt", "Verbindung keinem Prozess zuzuordnen", PEER_TCP, code="peer_unbekannt",
  lage={"peer": "Client endete vor der Abfrage"})
s("peer-ohne-job", "Prozess mit SID des Projekts, aber in keinem Job des Dienstes", PEER_PID, code="peer_unbekannt",
  lage={"peer": "AppContainer des Projekts ausserhalb jedes Laufs"})

# ---- helfer ---------------------------------------------------------------------------------------
def h(name, beschr, roh, antwort, lage=None):
    feld = {"b64": base64.b64encode(roh).decode()}
    if lage is not None:
        feld["lage"] = lage
    fall("helfer-" + name, beschr, "helfer", {"antwort": antwort}, **feld)


SID = SID_A.encode()
h("setze", "SETZE für eine SID mit Sperre", b"SETZE " + SID + b"\n", "OK gesetzt vorher=0", lage={"sperre": True})
h("setze-wiederholt", "SETZE für eine SID, die schon in der Liste steht", b"SETZE " + SID + b"\n",
  "OK gesetzt vorher=1", lage={"sperre": True, "liste": [SID_A]})
h("entferne", "ENTFERNE einer SID in der Liste", b"ENTFERNE " + SID + b"\n", "OK entfernt vorher=1",
  lage={"liste": [SID_A]})
h("entferne-fehlt", "ENTFERNE einer SID ohne Eintrag", b"ENTFERNE " + SID + b"\n", "OK entfernt vorher=0")
h("ohne-sperre", "Härtung: SETZE ohne dauerhafte Sperre", b"SETZE " + SID + b"\n", "ABGELEHNT ohne_sperre",
  lage={"sperre": False})
h("zu-lang-512", "512 Zeichen und LF", b"A" * 512 + b"\n", "ABGELEHNT zu_lang")
h("zu-lang-3000", "3000 Zeichen und LF", b"A" * 3000 + b"\n", "ABGELEHNT zu_lang")
h("grenze-511", "511 Zeichen und LF (512 Byte) ist noch lesbar", b"A" * 511 + b"\n", "ABGELEHNT anfrage_unbekannt")
h("form-klein", "Verb klein geschrieben", b"setze " + SID + b"\n", "ABGELEHNT anfrage_form")
h("form-cr", "CR vor dem LF", b"SETZE " + SID + b"\r\n", "ABGELEHNT anfrage_form")
h("form-nul", "Nullbyte", b"SETZE " + SID + b"\x00\n", "ABGELEHNT anfrage_form")
h("form-zwei-befehle", "zwei Befehle mit Semikolon", b"ENTFERNE " + SID + b";SETZE " + SID + b"\n",
  "ABGELEHNT anfrage_form")
h("unbekannt-loesche", "LOESCHE gibt es nicht", b"LOESCHE " + SID + b"\n", "ABGELEHNT anfrage_unbekannt")
h("unbekannt-ohne-sid", "SETZE ohne SID", b"SETZE\n", "ABGELEHNT anfrage_unbekannt")
h("unbekannt-leer", "leere Zeile", b"\n", "ABGELEHNT anfrage_unbekannt")
h("sid-nutzer", "Nutzer-SID statt AppContainer-SID", b"SETZE S-1-5-21-1-2-3-1001\n", "ABGELEHNT sid_form")
h("sid-system", "ENTFERNE S-1-5-18", b"ENTFERNE S-1-5-18\n", "ABGELEHNT sid_form")
h("sid-faehigkeit", "Fähigkeits-SID S-1-15-3", b"SETZE " + SID.replace(b"S-1-15-2", b"S-1-15-3") + b"\n",
  "ABGELEHNT sid_form")
h("sid-sechs", "sechs Zahlen", b"SETZE " + SID.rsplit(b"-", 1)[0] + b"\n", "ABGELEHNT sid_form")
h("sid-acht", "acht Zahlen", b"SETZE " + SID + b"-5\n", "ABGELEHNT sid_form")
h("sid-ueber-2-32", "Zahl über 2^32-1", b"SETZE " + SID.replace(b"1111111111", b"4294967296") + b"\n",
  "ABGELEHNT sid_form")
h("sid-fuehrende-null", "Zahl mit führender Null", b"SETZE " + SID.replace(b"1111111111", b"0111111111") + b"\n",
  "ABGELEHNT sid_form")
h("sid-leerzeichen", "zwei Leerzeichen vor der SID", b"SETZE  " + SID + b"\n", "ABGELEHNT sid_form")
h("ohne-zeilenende", "kein LF binnen 3 s: keine Antwort, Verbindung zu", b"SETZE " + SID, None,
  lage={"danach": "Schreiber wartet 3 s"})
h("aufrufer-person", "die Person öffnet die Pipe", b"SETZE " + SID + b"\n", "Fehler 5 an der Pipe",
  lage={"aufrufer": "Person"})
h("aufrufer-agent", "ein Agent im AppContainer öffnet die Pipe", b"SETZE " + SID + b"\n", "Fehler 5 an der Pipe",
  lage={"aufrufer": "Agent"})
h("aufrufer-pruefung", "Aufruferprüfung im Code bei offener DACL", b"SETZE " + SID + b"\n", "ABGELEHNT aufrufer",
  lage={"aufrufer": "Administrator", "dacl": "offen (nur Probe)"})

# ---- aufruf ---------------------------------------------------------------------------------------
START_DATEI = {"fassung": 1, "sitzung": "s-1", "harness": "claude", "modell": "claude-sonnet-5-5",
               "route": "cloud-beispiel", "kunde": "k-beispiel",
               "arbeitsordner": "C:\\Users\\beispiel\\Projekte\\projekt"}


def a(name, beschr, lage, erw):
    fall("aufruf-" + name, beschr, "aufruf", erw, lage={"start": START_DATEI, **lage})


ZEILE = "lauf.einschluss_fehlt: Einschluss nicht verfuegbar"
a("gelingt", "Einschluss gebaut, Agent läuft an", {}, {"agent": "laeuft", "exit": "der des Agenten",
                                                         "dienst_ops": ["projekt_neu", "lauf_start", "lauf_ende"]})
a("konfig-ungueltig", "Einschlussdatei ungültig", {"konfig": "shell bash"},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("dienst-fehlt", "Plattformdienst läuft nicht", {"dienst": "gestoppt"},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("dienst-falscher-besitzer", "die Person hat die Steuer-Pipe vorbelegt", {"dienst": "Pipe von der Person vorbelegt"},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("ordner-ausserhalb", "Arbeitsordner liegt unter keiner Projektwurzel",
  {"start_aendern": {"arbeitsordner": "C:\\Users\\beispiel\\Documents"}},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("ordner-gleich-wurzel", "Arbeitsordner ist die Projektwurzel selbst",
  {"start_aendern": {"arbeitsordner": "C:\\Users\\beispiel\\Projekte"}},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("ordner-umgelenkt", "Arbeitsordner ist ein Verbindungspunkt auf das Profil", {"ordner": "Junction auf C:\\Users\\beispiel"},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("ordner-kurzname", "Arbeitsordner in 8.3-Kurzform", {"start_aendern": {"arbeitsordner": "C:\\Users\\beispiel\\PROJEK~1\\projekt"}},
  {"exit": 69, "stderr": ZEILE, "agent": "nie erzeugt", "dienst_ops": []})
a("lauf-start-abgelehnt", "Dienst lehnt lauf_start ab: Agent läuft nie an", {"dienst_antwort": "dienst.prozess_falsch"},
  {"exit": 69, "stderr": ZEILE, "agent": "angehalten erzeugt, beendet ohne Lauf", "dienst_ops": ["projekt_neu", "lauf_start"]})
a("datei-dacl", "Startdatei mit ACE für ALL APPLICATION PACKAGES", {"start_dacl": "zusaetzlich AC"},
  {"exit": 66, "stderr": "lauf.datei_modus: Startdatei nicht eigen, nicht 0600 oder keine Datei", "agent": "nie erzeugt",
   "dienst_ops": []})
a("ordner-posix", "Startdatei mit POSIX-Pfad unter Windows", {"start_aendern": {"arbeitsordner": "/srv/beispiel/projekt"}},
  {"exit": 65, "stderr": "start.ordner_ungueltig: Arbeitsordner ungueltig", "agent": "nie erzeugt", "dienst_ops": []})
fall("aufruf-loeschen-gelingt", "einschluss-loeschen ohne lebenden Lauf", "aufruf",
     {"exit": 0, "danach": "Ausnahme, Sperre, Sublayer, Freigaben, Profil weg; Arbeitsordner und Inhalt bleiben"},
     lage={"argv": ["einschluss-loeschen", "C:\\Users\\beispiel\\Projekte\\projekt"], "laeufe": []})
fall("aufruf-loeschen-lauf-lebt", "einschluss-loeschen mit lebendem Lauf", "aufruf",
     {"exit": 75, "stderr": "loeschen.lauf_lebt: Lauf lebt noch", "danach": "unveraendert"},
     lage={"argv": ["einschluss-loeschen", "C:\\Users\\beispiel\\Projekte\\projekt"], "laeufe": ["s-1"]})
fall("aufruf-loeschen-helfer-fehlt", "einschluss-loeschen bei ausgefallenem Helfer", "aufruf",
     {"exit": 75, "stderr": "loeschen.ausnahme_bleibt: Ausnahme nicht entfernt, Sperre bleibt",
      "danach": "Ausnahme und Sperre stehen, Freigaben und Profil bleiben"},
     lage={"argv": ["einschluss-loeschen", "C:\\Users\\beispiel\\Projekte\\projekt"], "helfer": "gestoppt"})
fall("aufruf-loeschen-zwei-argumente", "einschluss-loeschen mit zwei Argumenten", "aufruf",
     {"exit": 64, "stderr": "loeschen.aufruf: genau ein Pfad erwartet", "danach": "unveraendert"},
     lage={"argv": ["einschluss-loeschen", "C:\\a", "C:\\b"]})

# ---- vm (Außentests) ------------------------------------------------------------------------------
def vm(w, art, ausgangslage, handlung, erwartet, messwerte, beleg):
    name = f"vm-{w.lower()}-{art}"
    fall(name, f"{w} {art}", "vm",
         {"erwartet": erwartet, "messwerte": messwerte},
         eigenschaft=w, art=art, ausgangslage=ausgangslage, handlung=handlung, beleg=beleg)


GRUND = ("Windows-VM; Plattformdienst unter NT SERVICE mit den fünf ACEs, Admin-Helfer unter LocalSystem; "
         "Projekt A mit Arbeitsordner unter der Projektwurzel; ein Lauf in A mit Claude Code (PowerShell-Werkzeug) "
         "gegen die Attrappe des Modellwegs hinter einem Weiterleiter auf dem Sitzungsport M; die Browser-Brücke des "
         "Kern-Dienstes hört auf dem Loopback des Rechners; ssh-Stummel vorn im PATH, SSH_AUTH_SOCK leer")
vm("W1", "fall", GRUND + "; Such-Prozess und Halter laufen",
   "Der Agent öffnet die Pipe des Halters, öffnet den Such-Prozess, den Halter und den Starter mit OpenProcess "
   "(PROCESS_TERMINATE, PROCESS_VM_READ, PROCESS_QUERY_LIMITED_INFORMATION) und fragt die Such-Pipe seiner Sitzung",
   "Halter-Pipe Fehler 5; jedes OpenProcess Fehler 5; die Such-Pipe antwortet",
   ["Fehlercode je Versuch", "Antwort der Such-Pipe"],
   "gemessen: Halter-Pipe Fehler 5 (Fragen 2, 5); nicht gemessen: OpenProcess aus dem Container")
vm("W1", "gegenprobe", GRUND, "Derselbe OpenProcess-Versuch aus einem Prozess der Person ohne Container",
   "OpenProcess auf den Such-Prozess gelingt; damit trägt der Container die Grenze", ["Fehlercode je Versuch"],
   "nicht gemessen")
vm("W2", "fall", GRUND + "; zwei Läufe s-1 (Route lokal-beispiel) und s-2 (Route cloud-beispiel) im selben Projekt",
   "Der Agent von s-2 öffnet die Such-Pipe von s-1 und fragt nach einer Notiz, die nur lokal-beispiel sieht",
   "Die Such-Pipe von s-1 lehnt die Verbindung ab (Peer nicht im Job von s-1); keine Marke der Notiz in einer Antwort",
   ["Ergebnis je Verbindung", "Zähler der Marken in Antworten: 0"], "nicht gemessen")
vm("W2", "gegenprobe", GRUND + "; zwei Läufe wie im Fall; Such-Prozess ohne Peer-Prüfung (Prüfling)",
   "wie im Fall", "Der Agent von s-2 bekommt Treffer der Route lokal-beispiel; der Test wird rot",
   ["Zähler der Marken in Antworten: größer 0"], "nicht gemessen")
vm("W3", "fall", GRUND + "; Projekt B mit eigener SID; Testdatei im Profil der Person; Werkbank-Daten nur beim Halter",
   "Der Agent und sein PowerShell-Werkzeug lesen die Profildatei, listen und lesen den Arbeitsordner von B, "
   "schreiben eine Marke in den eigenen Arbeitsordner, listen die Elternordner; das Profil wird nach "
   "DPAPI-CurrentUser-Daten der Werkbank durchsucht",
   "Profildatei und Ordner von B: Fehler 5; Marke steht im eigenen Ordner; Elternordner zeigen nur Namen, "
   "Dateien darin Fehler 5; 0 DPAPI-Funde",
   ["Fehlercode je Zugriff", "Marke", "Zähler der DPAPI-Funde: 0"],
   "gemessen: Fragen 2, 5, 6 (B: 7 von 7 Fehler 5); nicht gemessen: Namen in Elternordnern, DPAPI-Suche")
vm("W3", "gegenprobe", GRUND + "; Freigabe des Arbeitsordners von B für ALL APPLICATION PACKAGES statt für die SID",
   "Der Agent in A liest und beschreibt den Ordner von B", "Alle Zugriffe gelingen; der Test wird rot",
   ["Fehlercode je Zugriff"], "gemessen: Frage 6, 7 von 7 gelingen")
vm("W4", "fall", GRUND + "; ein fremder Hörer Q auf 127.0.0.1 und [::1]; ein Ziel im LAN und eines im Internet",
   "Der Agent verbindet sich mit M, Q und der Browser-Brücke über 127.0.0.1, [::1] und [::ffff:127.0.0.1], mit dem "
   "LAN-Ziel, mit dem Internet-Ziel und löst einen Namen auf",
   "M auf allen drei Wegen verbunden; Q und Browser-Brücke auf keinem (10013); LAN und Internet scheitern",
   ["Ergebnis und Fehlercode je Verbindung", "Filterliste vor und nach dem Lauf"],
   "gemessen: erlaubter Port und Q (Fragen 7, IPv6); nicht gemessen: Browser-Brücke, Internet-Ziel ohne Fähigkeit, "
   "Namensauflösung")
vm("W4", "gegenprobe", GRUND + "; nur das V4-Filterpaar gesetzt", "Der Agent verbindet sich mit [::1]:Q",
   "Die Verbindung steht; der Test wird rot", ["Ergebnis je Verbindung"], "gemessen: IPv6, [::1]:Q offen")
vm("W5", "fall", GRUND, "Während der Agent läuft, wird der Plattformdienst hart beendet; der Agent versucht M und Q "
   "alle 10 ms",
   "Der Job endet, oder der Agent erreicht weder M noch Q; Q zu keinem Zeitpunkt erreichbar; die Sperre steht danach",
   ["Lebensdauer des Jobs nach dem Tod des Dienstes", "Ergebnis je Versuch", "Filterliste"],
   "gemessen: Fragen 9, 10a (Job endet nach 1 bis 11 ms)")
vm("W5", "gegenprobe", GRUND + "; Form V1: Starter hält das Job-Handle, alle Filter dynamisch", "wie im Fall",
   "Der weiterlaufende Agent erreicht Q; der Test wird rot", ["Zeit bis zur ersten Verbindung auf Q"],
   "gemessen: Frage 9, Q nach 134 ms")
vm("W6", "fall", GRUND, "Das PowerShell-Werkzeug startet ein Kind mit CREATE_BREAKAWAY_FROM_JOB und ein Enkel-Kind; "
   "danach endet der Lauf regulär",
   "Breakaway Fehler 5; nach dem Jobende lebt kein Prozess des Baums und keine Erlaubnis der SID",
   ["Fehlercode", "Prozessliste nach dem Jobende", "Filterliste"], "gemessen: Fragen 2, 5")
vm("W6", "gegenprobe", GRUND + "; Job mit JOB_OBJECT_LIMIT_BREAKAWAY_OK (Prüfling)", "wie im Fall",
   "Das Kind überlebt das Jobende; der Test wird rot", ["Prozessliste nach dem Jobende"], "nicht gemessen")
vm("W7", "fall", GRUND + "; Projekt A ruht", "Windows startet neu; ein Prozess der SID von A startet vor dem "
   "Plattformdienst und versucht den alten Sitzungsport M und Q; danach startet ein neuer Lauf mit neuem Sitzungsport M2",
   "Sperre und Ausnahme stehen, keine Erlaubnis; vor dem Lauf weder M noch Q; im Lauf nur M2",
   ["Filterliste und Ausnahmeliste nach dem Neustart", "Ergebnis je Verbindung"], "gemessen: Frage 10b2")
vm("W7", "gegenprobe", GRUND + "; Sperre als dynamischer Filter (Prüfling)", "wie im Fall",
   "Nach dem Neustart steht die Ausnahme ohne Sperre; der Prozess erreicht Q; der Test wird rot",
   ["Ergebnis je Verbindung"], "nicht gemessen")
vm("W8", "fall", GRUND + "; die Person ohne Adminrechte",
   "Der Agent öffnet die Filter-Engine; die Person liest, löscht und ergänzt Filter; die Person ruft "
   "NetworkIsolationSetAppContainerConfig",
   "Agent: FwpmEngineOpen0 0x5; Person: je 0x5 bzw. Fehler 5; Filter und Ausnahmen bleiben",
   ["Fehlercode je Versuch", "Filterliste und Ausnahmeliste vorher und nachher"], "gemessen: A122, Frage 10a")
vm("W8", "gegenprobe", GRUND + "; Plattformdienst ohne die fünf ACEs", "Der Dienst setzt ein Filterpaar",
   "Fehler 5; kein Lauf startet (Exit 69)", ["Fehlercode"], "gemessen: Frage 10a, ohne ACE Fehler 5")
vm("W9", "fall", GRUND + "; SID C ohne dauerhafte Sperre",
   "Die Fälle helfer-* dieses Vertrags gegen den gebauten Helfer, dazu SETZE C vom Dienst",
   "jede Antwort wie in erwartet/helfer-*; SETZE C: ABGELEHNT ohne_sperre; Ausnahmeliste nach jedem Fehlversuch gleich",
   ["Antwort je Zeile", "Ausnahmeliste vorher und nachher"],
   "gemessen: Frage 11 (30 Gegenversuche); nicht gemessen: ohne_sperre")
vm("W9", "gegenprobe", GRUND + "; Helfer ohne Prüfung der Sperre (Prüfling)", "SETZE C vom Dienst",
   "C steht danach in der Ausnahmeliste ohne Sperre; der Test wird rot", ["Ausnahmeliste"], "nicht gemessen")
vm("W10", "fall", GRUND + "; zwei Läufe s-1 und s-2 im selben Projekt, je eigener Schein",
   "Jeder Lauf fragt den Modellweg mit dem eigenen und mit dem fremden Schein; dann endet s-1",
   "eigener Schein beantwortet, fremder abgelehnt; nach dem Ende von s-1 arbeitet s-2 weiter",
   ["Antwort je Anfrage", "Zähler fremder Scheine, die beantwortet werden: 0"],
   "gemessen: Frage 11, je mindestens 42 von 42; 129 von 129")
vm("W10", "gegenprobe", GRUND + "; Modellweg vergleicht nur die SID, nicht die Sitzung des Peers (Prüfling)",
   "wie im Fall", "Der fremde Schein wird beantwortet; der Test wird rot", ["Antwort je Anfrage"], "nicht gemessen")
vm("W11", "fall", "Windows-VM; zwei Personen P1 und P2 ohne Adminrechte, jede mit einem Projekt am gleich benannten "
   "Pfad unter ihrer eigenen Projektwurzel; beide Projekte angelegt",
   "Vergleich der zwei AppContainer-SIDs; der Agent von P1 liest den Arbeitsordner von P2",
   "Die SIDs sind verschieden; Fehler 5", ["SID je Projekt", "Fehlercode"], "nicht gemessen")
vm("W11", "gegenprobe", "wie im Fall; Profilname ohne Anteil der Person (Prüfling)", "wie im Fall",
   "Gleiche SID; der Test wird rot", ["SID je Projekt"], "nicht gemessen")
vm("W12", "fall", GRUND + "; Lauf s-1 lebt",
   "einschluss-loeschen für A, dann Ende von s-1, dann einschluss-loeschen bei gestopptem Helfer, dann mit Helfer, "
   "dann ein zweites Mal",
   "erst Exit 75 ohne Änderung; bei gestopptem Helfer Exit 75, Sperre bleibt; dann Exit 0 und Ausnahme, Sperre, "
   "Sublayer, Freigaben und Profil weg; beim zweiten Mal Exit 0",
   ["Exitcode je Aufruf", "Ausnahmeliste, Filterliste, ACL und Profil nach jedem Schritt"],
   "gemessen: Frage 11 (3); nicht gemessen: Wiederholung ohne Sublayer")
vm("W12", "gegenprobe", GRUND + "; Dienst löscht die Sperre vor der Antwort des Helfers (Prüfling)",
   "einschluss-loeschen bei gestopptem Helfer",
   "Die Ausnahme steht ohne Sperre; ein Prozess der SID erreicht Q; der Test wird rot",
   ["Filterliste", "Ergebnis der Verbindung auf Q"], "nicht gemessen")
vm("W14", "fall", GRUND + "; Projekt B mit eigener SID und laufender Sitzung s-9 mit Sitzungsport N; ein fremder "
   "Hörer Q", "(a) Der Agent in A verbindet sich mit der Browser-Brücke und mit Q; (b) mit M; (c) mit N. "
   "(d) Ein Prozess der Person sendet lauf_start mit dem Port der Browser-Brücke",
   "(a) 10013; (b) verbunden, der Modellweg antwortet; (c) 10013; (d) dienst.port_fremd, Filterliste unverändert",
   ["Ergebnis und Fehlercode je Verbindung", "Antwort auf lauf_start", "Filterliste vorher und nachher"],
   "nicht gemessen")
vm("W14", "gegenprobe", GRUND + "; Plattformdienst ohne Prüfung der Port-Besitzer (Prüfling)",
   "Ein Prozess der Person sendet lauf_start mit dem Port der Browser-Brücke; danach verbindet sich der Agent mit ihr",
   "Der Dienst erlaubt den Port, und der Agent erreicht die Browser-Brücke; der Test wird rot",
   ["Antwort auf lauf_start", "Ergebnis der Verbindung"], "nicht gemessen")
vm("W13", "fall", GRUND + "; Git Bash ist auf der Maschine installiert",
   "Die Attrappe verlangt einen Aufruf des Werkzeugs PowerShell, der die Marke M schreibt; danach einen Aufruf "
   "von bash.exe aus dem Werkzeug",
   "M steht im Arbeitsordner, und die nächste Anfrage an die Attrappe enthält M; bash.exe wird nicht gefunden "
   "oder scheitert; nach dem Jobende lebt kein Prozess",
   ["Datei im Arbeitsordner", "Mitschrift der Attrappe", "Ergebnis des bash-Aufrufs", "Prozessliste"],
   "gemessen: Frage 5 (Marke, Sperren, Jobende); nicht gemessen: bash-Aufruf aus dem Werkzeug")
vm("W13", "gegenprobe", GRUND + "; Starter setzt CLAUDE_CODE_USE_POWERSHELL_TOOL nicht und legt Git Bash in den PATH "
   "(Prüfling)", "wie im Fall", "Das Werkzeug bricht ab (0xC0000022); der Test wird rot",
   ["Mitschrift der Attrappe"], "gemessen: Nachmessung, Git Bash 0xC0000022")


def main():
    (HIER / "einschluss.schema.json").write_text(json.dumps(SCHEMA, ensure_ascii=False, indent=1) + "\n",
                                                 encoding="utf-8")
    for ordner in ("faelle", "erwartet"):
        (HIER / ordner).mkdir(exist_ok=True)
        for alt in (HIER / ordner).glob("*.json"):
            alt.unlink()
    for name, (f, e) in sorted(FAELLE.items()):
        (HIER / "faelle" / f"{name}.json").write_text(text(f), encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(text(e), encoding="utf-8")
    print(f"{len(FAELLE)} Fälle geschrieben")


if __name__ == "__main__":
    main()
