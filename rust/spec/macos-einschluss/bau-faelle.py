#!/usr/bin/env python3
"""Erzeugt Schema und sichtbare Fälle des Vertrags Einschluss unter macOS (faelle/, erwartet/).

Aufruf: python3 -I rust/spec/macos-einschluss/bau-faelle.py
Formen: `konfig` (Einschlussdatei der Installation), `bereit` (die Bereitschaftszeile eines Weiterreichers),
`profil` (eine Einschlussbeschreibung und was das gebaute Seatbelt-Profil enthalten muss), `profiltext` (ein Zusatz im
gebauten Profil, den die zweite Prüfung am Text ablehnen muss), `aufruf` (das Programm
"Lauf starten" unter macOS als Ganzes) und `mac` (Außentest am Mac mit echtem Seatbelt). `aufruf` und `mac` sind erst
mit dem Bau prüfbar.
"""
import base64
import copy
import json
from pathlib import Path

HIER = Path(__file__).resolve().parent

KENNUNG = r"^[a-z0-9][a-z0-9._-]{0,63}$"
# Ein absoluter, normalisierter POSIX-Pfad wie in Vertrag 1, dazu ohne `"` und `\`: beide Zeichen haben in einer
# Zeichenkette des Seatbelt-Profils eine Bedeutung, und das Profil maskiert nicht.
_SEGMENT = r'(?!\.\.?(?:/|$))[^/\x00-\x1f\x7f-\x9f"\\]+'
POSIXPFAD = r"^(?:/" + _SEGMENT + r")+$"

SCHEMA = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://werkbank.invalid/spec/macos-einschluss/einschluss.schema.json",
    "title": "Einschluss unter macOS, Fassung 1",
    "$defs": {
        "kennung": {"type": "string", "pattern": KENNUNG},
        "posixpfad": {"type": "string", "minLength": 2, "maxLength": 1024, "pattern": POSIXPFAD},
        "pfadliste": {"type": "array", "minItems": 1, "maxItems": 16, "uniqueItems": True,
                      "items": {"$ref": "#/$defs/posixpfad"}},
        "port": {"type": "integer", "minimum": 1, "maximum": 65535},
        "konfig": {
            "type": "object",
            "additionalProperties": False,
            "required": ["fassung", "projektwurzeln", "programmordner"],
            "properties": {
                "fassung": {"const": 1},
                "projektwurzeln": {"$ref": "#/$defs/pfadliste"},
                "programmordner": {"$ref": "#/$defs/pfadliste"},
            },
        },
        "bereit": {
            "type": "object",
            "additionalProperties": False,
            "required": ["bereit", "port"],
            "properties": {"bereit": {"const": True}, "port": {"$ref": "#/$defs/port"}},
        },
        "beschreibung": {
            "description": "Die Einschlussbeschreibung unter macOS als JSON, nur für die Fälle der Form profil. Im Crate ist sie ein Typ.",
            "type": "object",
            "additionalProperties": False,
            "required": ["harness", "arbeitsordner", "programmordner", "zustand", "heim", "such_socket", "merkmal",
                         "sitzungsports", "umgebung", "geschuetzt", "argv"],
            "properties": {
                "harness": {"$ref": "#/$defs/kennung"},
                "arbeitsordner": {"$ref": "#/$defs/posixpfad"},
                "programmordner": {"type": "array", "maxItems": 16, "items": {"$ref": "#/$defs/posixpfad"}},
                "zustand": {"$ref": "#/$defs/posixpfad"},
                "heim": {"$ref": "#/$defs/posixpfad"},
                "such_socket": {"$ref": "#/$defs/posixpfad"},
                "merkmal": {"$ref": "#/$defs/posixpfad"},
                "sitzungsports": {
                    "type": "object", "additionalProperties": False, "required": ["modellweg"],
                    "properties": {"modellweg": {"$ref": "#/$defs/port"}, "ausgang": {"$ref": "#/$defs/port"}},
                },
                "umgebung": {
                    "type": "object", "additionalProperties": False,
                    "required": ["home", "path", "term", "lang", "tmpdir"],
                    "properties": {k: {"type": "string", "minLength": 1} for k in ("home", "path", "term", "lang", "tmpdir")},
                },
                "geschuetzt": {"type": "array", "items": {"$ref": "#/$defs/posixpfad"}},
                "argv": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
            },
        },
    },
}

KONFIG = {
    "fassung": 1,
    "projektwurzeln": ["/Users/beispiel/Projekte"],
    "programmordner": ["/Users/beispiel/.local/share/claude", "/opt/homebrew"],
}

BESCHREIBUNG = {
    "harness": "claude",
    "arbeitsordner": "/Users/beispiel/Projekte/projekt",
    "programmordner": ["/Users/beispiel/.local/share/claude"],
    "zustand": "/Users/beispiel/Library/Application Support/werkbank/heime/s-1",
    "heim": "/Users/beispiel",
    "such_socket": "/Users/beispiel/.wbs/s-1/such.sock",
    "merkmal": "/Users/beispiel/Library/Application Support/werkbank/dienst/merkmal/s-1/kennung",
    "sitzungsports": {"modellweg": 41001, "ausgang": 41002},
    "umgebung": {"home": "/Users/beispiel/Library/Application Support/werkbank/heime/s-1",
                 "path": "/Users/beispiel/.local/share/claude:/usr/bin:/bin",
                 "term": "xterm-256color", "lang": "de_DE.UTF-8",
                 "tmpdir": "/Users/beispiel/Library/Application Support/werkbank/heime/s-1/tmp"},
    "geschuetzt": ["/Users/beispiel/Knowledge", "/Users/beispiel/Library/Application Support/werkbank/protokoll",
                   "/Users/beispiel/Library/Application Support/werkbank/dienst"],
    "argv": ["/Users/beispiel/.local/share/claude/claude"],
}


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
k("gueltig-leerzeichen", "Leerzeichen im Pfad (Application Support) ist gewöhnlich",
  mit(KONFIG, lambda o: o["programmordner"].append("/Users/beispiel/Library/Application Support/werkbank/bin")))
k("gueltig-punkte-im-namen", "Segmente mit Punkt im Namen sind gewöhnlich",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users/beispiel/.werkbank/pro.jekte", "/Users/beispiel/...x"])))
k("gueltig-umlaut", "Umlaut im Pfad", mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users/beispiel/Übungen"])))
k("zu-gross", "mehr als 4096 Byte", roh=(text(KONFIG)[:-2] + " " * 4096 + "}\n").encode(), code="zu_gross")
k("json-bom", "BOM vor dem Objekt", roh=b"\xef\xbb\xbf" + text(KONFIG).encode(), code="json_ungueltig")
k("json-doppelt", "doppelter Schlüssel fassung",
  roh=text(KONFIG).replace('"fassung": 1', '"fassung": 1, "fassung": 1').encode(), code="json_ungueltig")
k("json-liste", "oberste Ebene ist eine Liste", roh=b"[]\n", code="json_ungueltig")
k("json-kein-utf8", "Byte 0xff im Pfad", roh=text(KONFIG).encode().replace(b"Projekte", b"Proj\xffkte"),
  code="json_ungueltig")
k("schluessel-unbekannt-port", "kein Port in der Konfiguration: Sitzungsports entstehen je Sitzung",
  mit(KONFIG, lambda o: o.__setitem__("modellweg_port", 41001)), code="schluessel_unbekannt")
k("schluessel-unbekannt-shell", "shell gilt nur unter Windows (A117)",
  mit(KONFIG, lambda o: o.__setitem__("shell", "zsh")), code="schluessel_unbekannt")
k("schluessel-unbekannt-regel", "keine frei wählbaren Seatbelt-Regeln",
  mit(KONFIG, lambda o: o.__setitem__("regeln", ["(allow network-outbound)"])), code="schluessel_unbekannt")
k("schluessel-fehlt-programmordner", "programmordner fehlt", mit(KONFIG, lambda o: o.pop("programmordner")),
  code="schluessel_fehlt")
k("typ-wurzeln-text", "projektwurzeln als Text",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", "/Users/beispiel/Projekte")), code="typ_falsch")
k("typ-fassung-text", "fassung als Text", mit(KONFIG, lambda o: o.__setitem__("fassung", "1")), code="typ_falsch")
k("fassung-2", "Fassung 2", mit(KONFIG, lambda o: o.__setitem__("fassung", 2)), code="fassung_unbekannt")
k("liste-leer", "keine Projektwurzel", mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", [])), code="liste_grenze")
k("liste-17", "17 Programmordner",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", [f"/opt/p{i}" for i in range(17)])), code="liste_grenze")
k("liste-doppelt", "derselbe Pfad zweimal",
  mit(KONFIG, lambda o: o["projektwurzeln"].append(o["projektwurzeln"][0])), code="liste_grenze")
for name, pfad, beschr in [
    ("pfad-wurzel", "/", "die Wurzel allein"),
    ("pfad-relativ", "Users/beispiel", "relativer Pfad"),
    ("pfad-tilde", "~/Projekte", "Tilde wird nicht ausgewertet"),
    ("pfad-punktpunkt", "/Users/beispiel/../anders", "Segment .."),
    ("pfad-punkt", "/Users/./beispiel", "Segment ."),
    ("pfad-ende-schraeg", "/Users/beispiel/", "endet mit /"),
    ("pfad-doppel-schraeg", "/Users//beispiel", "leeres Segment"),
    ("pfad-anfuehrung", '/Users/beispiel/a"b', "Anführungszeichen hat im Profil eine Bedeutung"),
    ("pfad-backslash", "/Users/beispiel/a\\b", "Backslash hat im Profil eine Bedeutung"),
    ("pfad-steuerzeichen", "/Users/beispiel/a\tb", "Steuerzeichen"),
    ("pfad-c1", "/Users/beispiel/a\u0085b", "C1-Steuerzeichen"),
]:
    k(name, beschr, mit(KONFIG, lambda o, p=pfad: o.__setitem__("projektwurzeln", [p])), code="pfad_form")
k("pfad-zu-lang", "1025 Byte", mit(KONFIG, lambda o: o.__setitem__(
    "projektwurzeln", ["/" + "/".join(["a" * 100] * 10) + "/" + "b" * 14])), code="pfad_form")
# Stufen, die das Dateisystem oder die Person brauchen: erst mit dem Bau prüfbar.
k("ueberlappt-programm-unter-wurzel", "Programmordner unter einer Projektwurzel",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/Projekte/werkzeug"])), code="pfad_ueberlappt")
k("ueberlappt-wurzel-unter-programm", "Projektwurzel unter einem Programmordner",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/opt/homebrew/projekte"])), code="pfad_ueberlappt")
k("ueberlappt-gross-klein", "derselbe Ordner in anderer Schreibweise (APFS ohne Unterscheidung)",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["/users/BEISPIEL/projekte"])), code="pfad_ueberlappt")
k("geschuetzt-heim", "Projektwurzel ist das Heim der Person",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users/beispiel"])), code="pfad_geschuetzt")
k("geschuetzt-ueber-heim", "Projektwurzel liegt über dem Heim",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users"])), code="pfad_geschuetzt")
k("geschuetzt-system", "Programmordner ist /System",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["/System"])), code="pfad_geschuetzt")
k("geschuetzt-anmeldespeicher", "Programmordner deckt den Schlüsselbund der Person",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/Library"])), code="pfad_geschuetzt")
k("geschuetzt-copilot", "Programmordner ist der Ordner von Copilot CLI mit Token",
  mit(KONFIG, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/.copilot"])), code="pfad_geschuetzt")
k("fehlt", "Projektwurzel ist kein vorhandener Ordner",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users/beispiel/gibt-es-nicht"])), code="pfad_fehlt")
k("umgelenkt-tmp", "/tmp ist ein Verweis auf /private/tmp",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/tmp/projekte"])), code="pfad_umgelenkt")
k("umgelenkt-schreibweise", "Pfad in anderer Schreibweise als auf der Platte",
  mit(KONFIG, lambda o: o.__setitem__("projektwurzeln", ["/Users/beispiel/PROJEKTE"])), code="pfad_umgelenkt")


# ---- bereit ---------------------------------------------------------------------------------------
def b(name, beschr, roh, gueltig):
    fall("bereit-" + name, beschr, "bereit", ok() if gueltig else {"gueltig": False}, b64=base64.b64encode(roh).decode())


b("gueltig", "Weiterreicher meldet seinen Port", b'{"bereit":true,"port":41001}\n', True)
b("gueltig-leerzeichen", "Leerzeichen sind JSON", b'{"bereit": true, "port": 41001}\n', True)
b("ohne-lf", "kein Zeilenende", b'{"bereit":true,"port":41001}', False)
b("port-null", "Port 0", b'{"bereit":true,"port":0}\n', False)
b("port-gross", "Port 65536", b'{"bereit":true,"port":65536}\n', False)
b("port-text", "Port als Text", b'{"bereit":true,"port":"41001"}\n', False)
b("port-bruch", "Port als 41001.0", b'{"bereit":true,"port":41001.0}\n', False)
b("bereit-falsch", "bereit false", b'{"bereit":false,"port":41001}\n', False)
b("schluessel-mehr", "ein zweiter Port", b'{"bereit":true,"port":41001,"port2":41002}\n', False)
b("zwei-zeilen", "Text nach der Zeile", b'{"bereit":true,"port":41001}\n{"bereit":true,"port":41002}\n', False)
b("doppelt", "doppelter Schlüssel", b'{"bereit":true,"port":41001,"port":41002}\n', False)


# ---- profil ---------------------------------------------------------------------------------------
KOPF = ["(version 1)", "(deny default)", '(import "system.sb")', "(allow process-fork)", "(allow process-exec)",
        "(allow signal (target same-sandbox))", "(allow process-info* (target same-sandbox))"]
NIE = ["(allow default)", "(allow network*", "(allow network-outbound)", "(allow network-bind",
       "(allow network-inbound", "(allow file-read*)", "(allow file-write*)", "(allow signal)",
       "(allow process-info*)", "(allow mach-lookup", "(allow mach-priv", "(remote ip \"*:", "(remote tcp",
       "(remote udp"]


def p(name, beschr, obj, fehler=None, enthaelt=(), enthaelt_nicht=()):
    erw = {"gueltig": False, "fehler": fehler} if fehler else ok(enthaelt=KOPF + list(enthaelt),
                                                                   enthaelt_nicht=NIE + list(enthaelt_nicht))
    fall("profil-" + name, beschr, "profil", erw, eingabe=obj)


B = BESCHREIBUNG
p("gueltig", "die Vorführbeschreibung mit Modellweg und Ausgang", B, enthaelt=[
    '(allow network-outbound (remote ip "localhost:41001") (remote ip "localhost:41002"))',
    '(allow network-outbound (remote unix-socket (path-literal "/Users/beispiel/.wbs/s-1/such.sock")))',
    '(allow file-read* file-write* (subpath "/Users/beispiel/Projekte/projekt") '
    '(subpath "/Users/beispiel/Library/Application Support/werkbank/heime/s-1"))',
    '(literal "/Users/beispiel")', '(literal "/Users")',
    '(subpath "/Users/beispiel/Knowledge")', '(subpath "/Users/beispiel/.copilot")',
    '(subpath "/Users/beispiel/Library/Keychains")',
    '(deny sysctl-read (sysctl-name "kern.procargs2") (sysctl-name "kern.procargs") (sysctl-name-prefix "kern.proc."))',
    '(deny process-info* (target others))',
    '(allow file-write-data (literal "/Users/beispiel/Library/Application Support/werkbank/dienst/merkmal/s-1/kennung"))'],
  enthaelt_nicht=['(subpath "/Users/beispiel")', '(subpath "/Users/beispiel/.wbs'])
p("gueltig-ohne-ausgang", "lokale Route: nur der Modellweg",
  mit(B, lambda o: o["sitzungsports"].pop("ausgang")),
  enthaelt=['(allow network-outbound (remote ip "localhost:41001"))'], enthaelt_nicht=['"localhost:41002"'])
p("git-regel", "git unter dem Profil: der Ordner der Command Line Tools wird gelesen und ausgeführt, nie beschrieben (H9)", B,
  enthaelt=['(subpath "/Library/Developer/CommandLineTools")'],
  enthaelt_nicht=['(subpath "/Applications/Xcode.app', 'xcode_select_link', '(allow file-write* (subpath "/Library'])
p("ohne-modellweg", "keine Sitzungsports: eine Netzregel ohne Filter entstünde", mit(B, lambda o: o.__setitem__("sitzungsports", {})),
  fehler="BeschreibungUngueltig")
p("merkmal-im-arbeitsordner", "die Merkmal-Datei liegt im Arbeitsordner",
  mit(B, lambda o: o.__setitem__("merkmal", "/Users/beispiel/Projekte/projekt/kennung")), fehler="Ueberlappt")
p("ports-gleich", "Modellweg und Ausgang mit demselben Port",
  mit(B, lambda o: o["sitzungsports"].__setitem__("ausgang", 41001)), fehler="BeschreibungUngueltig")
p("arbeitsordner-heim", "Arbeitsordner ist das Heim", mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel")),
  fehler="Verboten")
p("arbeitsordner-ueber-heim", "Arbeitsordner liegt über dem Heim", mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users")),
  fehler="Verboten")
p("arbeitsordner-system", "Arbeitsordner unter /System",
  mit(B, lambda o: o.__setitem__("arbeitsordner", "/System/Library/x")), fehler="Verboten")
p("arbeitsordner-tmp", "Arbeitsordner ist /private/tmp", mit(B, lambda o: o.__setitem__("arbeitsordner", "/private/tmp")),
  fehler="Verboten")
p("arbeitsordner-ssh", "Arbeitsordner mit Geheimnisteil", mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/.ssh/p")),
  fehler="Verboten")
p("arbeitsordner-env", "Arbeitsordner heißt .env", mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/Projekte/.env")),
  fehler="Verboten")
p("programm-keychains", "Programmordner deckt die Schlüsselbunde",
  mit(B, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/Library/Keychains"])), fehler="Verboten")
p("programm-library", "Programmordner liegt über den Anmeldespeichern",
  mit(B, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/Library"])), fehler="Verboten")
p("programm-copilot", "Programmordner ist ~/.copilot",
  mit(B, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/.copilot"])), fehler="Verboten")
p("programm-gh", "Programmordner ist ~/.config/gh",
  mit(B, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/.config/gh"])), fehler="Verboten")
p("programm-claude-zugang", "die echte Anmeldung des Harness wird nie freigegeben",
  mit(B, lambda o: o.__setitem__("programmordner", ["/Users/beispiel/.claude"])), fehler="Verboten")
p("ueberlappt-vault", "Arbeitsordner liegt im Vault",
  mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/Knowledge/projekt")), fehler="Ueberlappt")
p("ueberlappt-vault-darunter", "der Vault liegt im Arbeitsordner",
  mit(B, lambda o: o.__setitem__("geschuetzt", ["/Users/beispiel/Projekte/projekt/vault"])), fehler="Ueberlappt")
p("ueberlappt-gross-klein", "Vault in anderer Schreibweise über dem Arbeitsordner",
  mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/KNOWLEDGE/projekt")), fehler="Ueberlappt")
p("ueberlappt-zustand", "Zustand liegt im Arbeitsordner",
  mit(B, lambda o: (o.__setitem__("zustand", "/Users/beispiel/Projekte/projekt/.heim"),
                    o["umgebung"].__setitem__("home", "/Users/beispiel/Projekte/projekt/.heim"),
                    o["umgebung"].__setitem__("tmpdir", "/Users/beispiel/Projekte/projekt/.heim/tmp"))),
  fehler="Ueberlappt")
p("socket-im-arbeitsordner", "Such-Socket liegt im Arbeitsordner",
  mit(B, lambda o: o.__setitem__("such_socket", "/Users/beispiel/Projekte/projekt/such.sock")), fehler="Ueberlappt")
p("socket-zu-lang", "Such-Socket länger als 103 Byte (sun_path 104 mit Nullbyte)",
  mit(B, lambda o: o.__setitem__("such_socket", "/Users/beispiel/" + "s" * 80 + "/such.sock")),
  fehler="BeschreibungUngueltig")
p("home-falsch", "HOME ist nicht der Zustand der Sitzung",
  mit(B, lambda o: o["umgebung"].__setitem__("home", "/Users/beispiel")), fehler="BeschreibungUngueltig")
p("tmpdir-draussen", "TMPDIR liegt nicht im Zustand",
  mit(B, lambda o: o["umgebung"].__setitem__("tmpdir", "/private/tmp")), fehler="BeschreibungUngueltig")
p("harness-form", "Harness verletzt die Kennungsform", mit(B, lambda o: o.__setitem__("harness", "Claude")),
  fehler="BeschreibungUngueltig")
p("argv-leer", "kein Programm", mit(B, lambda o: o.__setitem__("argv", [])), fehler="BeschreibungUngueltig")
p("pfad-anfuehrung", "Arbeitsordner mit Anführungszeichen (Vertrag 1 lässt ihn zu, das Profil nicht)",
  mit(B, lambda o: o.__setitem__("arbeitsordner", '/Users/beispiel/Projekte/a"b')), fehler="BeschreibungUngueltig")
p("pfad-backslash", "Arbeitsordner mit Backslash",
  mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/Projekte/a\\b")), fehler="BeschreibungUngueltig")
p("pfad-punktpunkt", "Arbeitsordner mit ..",
  mit(B, lambda o: o.__setitem__("arbeitsordner", "/Users/beispiel/Projekte/../x")), fehler="BeschreibungUngueltig")


# ---- profiltext -----------------------------------------------------------------------------------
# Die zweite Prüfung am fertigen Text (Fix 22.11.): jede Form von Leerraum und jede Regel ohne Filter wird abgelehnt.
def pt(name, beschr, einfuegen, gueltig=False):
    fall("profiltext-" + name, beschr, "profiltext", ok() if gueltig else {"gueltig": False, "fehler": "BeschreibungUngueltig"},
         eingabe=BESCHREIBUNG, einfuegen=einfuegen)


pt("gueltig-filter", "Gegenprobe: eine Regel mit Filter geht durch", '(allow network-outbound (remote ip "localhost:41003"))', True)
pt("netz-ohne-filter", "(allow network-outbound) ohne Leerraum", "(allow network-outbound)")
pt("netz-leerzeichen-vor-klammer", "(allow network-outbound ) mit einem Leerzeichen", "(allow network-outbound )")
pt("netz-mehrere-leerzeichen", "(allow network-outbound   ) mit drei Leerzeichen", "(allow network-outbound   )")
pt("netz-tab", "(allow<Tab>network-outbound)", "(allow\tnetwork-outbound)")
pt("netz-tab-vor-klammer", "(allow network-outbound<Tab>)", "(allow network-outbound\t)")
pt("netz-zeilenumbruch", "(allow network-outbound<LF>)", "(allow network-outbound\n)")
pt("netz-doppeltes-leerzeichen-zwischen", "(allow  network-outbound) mit zwei Leerzeichen nach allow", "(allow  network-outbound)")
pt("netz-leerzeichen-nach-klammer", "( allow network-outbound)", "( allow network-outbound)")
pt("netz-zwei-operationen", "(allow network-outbound network-bind ) zwei Operationen ohne Filter", "(allow network-outbound network-bind )")
pt("netz-stern", "(allow network*<Tab>) alle Netzoperationen", "(allow network*\t)")
pt("default", "(allow default ) offenes Profil", "(allow default )")
pt("datei-lesen", "(allow file-read* ) ohne Filter", "(allow file-read* )")
pt("datei-schreiben", "(allow file-write*<LF>) ohne Filter", "(allow file-write*\n)")
pt("mach-tab", "(allow<Tab>mach-lookup) ohne Filter", "(allow\tmach-lookup)")
pt("signal-ohne-filter", "(allow signal ) an jeden", "(allow signal )")
pt("prozessinfo-ohne-filter", "(allow process-info* ) von jedem", "(allow process-info* )")
pt("host-stern-leerraum", '(remote<Tab>ip "*:443") statt localhost', '(allow network-outbound (remote\tip "*:443"))')
pt("tcp-leerraum", "(remote<LF>tcp) statt ip", '(allow network-outbound (remote\ntcp "localhost:443"))')
pt("udp-leerraum", "(remote  udp) mit zwei Leerzeichen", '(allow network-outbound (remote  udp "localhost:443"))')
pt("zweites-deny-default", "ein zweites (deny  default) mit Leerzeichen", "(deny  default)")


# ---- aufruf ---------------------------------------------------------------------------------------
LAGE = ("macOS 26; Startdatei 0600 im 0700-Ordner des Dienstes; Einschlussdatei gültig; Arbeitsordner vorhanden unter "
        "einer Projektwurzel; /usr/bin/sandbox-exec vorhanden, root gehört es")
EINSCHLUSS_FEHLT = {"exit": 69, "stderr": "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n"}


def a(name, beschr, lage, erw):
    fall("aufruf-" + name, beschr, "aufruf", erw, lage=lage)


a("gelingt", "alles vorhanden", LAGE, {"exec": ["/usr/bin/sandbox-exec", "-f", "<profil im 0700-Ordner>", "--", "<argv des Harness>"],
                                        "stderr": "", "zustand": "Such-Prozess und Weiterreicher laufen; ihre PIDs stehen in suche.pid und weiter.pid"})
a("seatbelt-fehlt", "sandbox-exec fehlt", LAGE + "; /usr/bin/sandbox-exec fehlt", EINSCHLUSS_FEHLT)
a("seatbelt-fremd", "sandbox-exec gehört nicht root", LAGE + "; /usr/bin/sandbox-exec ist für andere schreibbar (Prüfling)",
  EINSCHLUSS_FEHLT)
a("seatbelt-ohne-wirkung", "Attrappe von sandbox-exec ohne Sperre: die Kanarie liest", LAGE + "; Attrappe am festen Pfad (Prüfling)",
  EINSCHLUSS_FEHLT)
a("profil-laedt-nicht", "das Profil lädt nicht", LAGE + "; Syntaxfehler im gebauten Profil (Prüfling)", EINSCHLUSS_FEHLT)
a("konfig-ungueltig", "Einschlussdatei mit unbekanntem Schlüssel", LAGE + "; Einschlussdatei mit regeln", EINSCHLUSS_FEHLT)
a("ordner-nicht-unter-wurzel", "Arbeitsordner liegt unter keiner Projektwurzel", LAGE + "; Arbeitsordner /Users/beispiel/anders",
  EINSCHLUSS_FEHLT)
a("ordner-anfuehrung", "Arbeitsordner mit Anführungszeichen", LAGE + '; Arbeitsordner /Users/beispiel/Projekte/a"b',
  EINSCHLUSS_FEHLT)
a("ordner-fehlt", "Arbeitsordner fehlt (Schritt 5 von Vertrag 1)", LAGE + "; Arbeitsordner gibt es nicht",
  {"exit": 65, "stderr": "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner\n"})
a("weiterreicher-stumm", "ein Weiterreicher meldet keinen Port", LAGE + "; Weiterreicher des Modellwegs schreibt nichts (Prüfling)",
  {**EINSCHLUSS_FEHLT, "zustand": "Such-Prozess und gestartete Weiterreicher sind beendet"})
a("weiterreicher-fremd", "am gemeldeten Port hört nicht der Weiterreicher", LAGE + "; Weiterreicher meldet den Port der Browser-Brücke (Prüfling)",
  {**EINSCHLUSS_FEHLT, "zustand": "Such-Prozess und gestartete Weiterreicher sind beendet"})
a("bruecke-als-port", "ein Sitzungsport ist der Port der Browser-Brücke", LAGE + "; der Kern-Dienst hört schon auf dem gemeldeten Port",
  {**EINSCHLUSS_FEHLT, "zustand": "Such-Prozess und gestartete Weiterreicher sind beendet"})
a("suche-nicht-bereit", "Such-Prozess meldet sich nicht (wie Vertrag 1)", LAGE + "; Such-Prozess schreibt nichts",
  {"exit": 69, "stderr": "lauf.suche_nicht_bereit: Suche nicht bereit\n"})
a("socket-zu-lang", "Hostpfad des Such-Sockets länger als 103 Byte (wie Vertrag 1)", LAGE + "; Socket-Ordner tief",
  {"exit": 69, "stderr": "lauf.suche_fehlt: Suche nicht startbar\n"})
a("kein-mac", "Lauf starten im macOS-Modul auf einem anderen System", "Linux; Bau des macOS-Moduls (cfg-Test)",
  EINSCHLUSS_FEHLT)


# ---- mac (Außentests) -----------------------------------------------------------------------------
ANFORDERUNG = {"M17": "A142", "M1": "A142", "M2": "A145", "M3": "A145", "M4": "A145", "M5": "A145", "M6": "A145", "M7": "A144",
               "M8": "A142", "M9": "A142", "M10": "A145", "M11": "A142", "M12": "A142", "M13": "A142", "M14": "A142",
               "M15": "A142", "M16": "A142"}


def mac(m, art, ausgangslage, handlung, erwartet, messwerte, beleg, gemeinsam=None):
    name = f"mac-{m.lower()}-{art}"
    fall(name, f"{m} {art}", "mac", {"erwartet": erwartet, "messwerte": messwerte},
         eigenschaft=m, anforderung=ANFORDERUNG[m], art=art, ausgangslage=ausgangslage, handlung=handlung, beleg=beleg,
         **({"gemeinsam": gemeinsam} if gemeinsam else {}))


GRUND = ("Mac mit macOS 26; Lauf s-1 unter dem gebauten Profil mit Modellweg-Port M1 und Ausgangs-Port A1; Lauf s-2 "
         "unter seinem Profil mit M2; Hörer der Browser-Brücke auf Port K; Vorführ-Vault mit der Notiz MK-7Q2 außerhalb; "
         "Such-Prozess je Sitzung außerhalb; Wegwerf-Token unter ~/.copilot und ~/.config/gh einer Testperson")
OHNE = "derselbe Versuch ohne Profil, als die Person"

mac("M1", "fall", GRUND, "Der Agent liest die Notizdatei mit cat, über einen Verweis im Arbeitsordner, in anderer "
    "Schreibweise, als Enkel und als abgelöster Nachfahr (doppelter fork)",
    "jeder Versuch: Operation not permitted (EPERM)", ["errno je Versuch"],
    "gemessen: Messproben D3, D9, D13, D15, D16, D20", gemeinsam="Notizdatei lesen scheitert")
mac("M1", "gegenprobe", GRUND, OHNE, "jeder Versuch liest die Notiz; der Test wird rot", ["errno je Versuch"],
    "gemessen: dieselben Proben ohne Profil")
mac("M2", "fall", GRUND, "Der Agent verbindet sich per TCP mit der eigenen LAN-Adresse (Port ohne Hörer und Port M1), "
    "sendet UDP an sie und fragt den Namensdienst",
    "TCP und UDP: EPERM, bevor ein Paket den Stapel erreicht; der Mach-Dienst des Namensdienstes ist gesperrt",
    ["errno je Versuch", "Antwort von sandbox_check für com.apple.dnssd.service"],
    "gemessen: Messproben N7, N8, N9 und Mach-Probe", gemeinsam="fremde Adresse scheitert")
mac("M2", "gegenprobe", GRUND, OHNE, "TCP: ECONNREFUSED (der Stapel ist erreicht), UDP gesendet; der Test wird rot",
    ["errno je Versuch"], "gemessen: dieselben Proben ohne Profil")
mac("M3", "fall", GRUND, "Der Agent ruft die Browser-Brücke auf 127.0.0.1:K mit eigenem Socket und mit curl",
    "EPERM; curl meldet einen Fehler", ["errno", "Exitcode von curl"], "gemessen: Messproben N5, N12",
    gemeinsam="Port der Browser-Brücke unerreichbar")
mac("M3", "gegenprobe", GRUND, OHNE, "die Brücke antwortet; der Test wird rot", ["Antwort"], "gemessen")
mac("M4", "fall", GRUND, "Der Agent ruft den eigenen Modellweg über 127.0.0.1:M1, [::1]:M1 und localhost:M1",
    "jede Verbindung steht, der Weiterreicher antwortet", ["Antwort je Weg"], "gemessen: Messproben N1, N2, N11, N14",
    gemeinsam="eigener Modellweg-Port erreichbar")
mac("M4", "gegenprobe", GRUND, "derselbe Aufruf aus dem Lauf s-2 (Profil ohne M1)", "EPERM; der Port hängt am Profil der Sitzung",
    ["errno"], "gemessen: Messprobe N15")
mac("M5", "fall", GRUND, "Der Agent von s-1 ruft den Modellweg-Port M2 der Sitzung s-2", "EPERM", ["errno"],
    "gemessen: Messprobe N6", gemeinsam="Modellweg-Port einer anderen Sitzung nicht erreichbar")
mac("M5", "gegenprobe", GRUND, OHNE, "M2 antwortet; der Test wird rot", ["Antwort"], "gemessen")
mac("M6", "fall", GRUND, "Der Agent setzt ANTHROPIC_BASE_URL und jede Variable mit route im Namen auf den Port K "
    "und ruft das Ziel der Variable", "EPERM; die Netzgrenze steht im Profil, das vor dem exec feststand",
    ["errno", "Exitcode von curl"], "gemessen: Messprobe N13 (Umgebung auf K)", gemeinsam="E1: Umgebung ändert die Route nicht")
mac("M6", "gegenprobe", GRUND, OHNE, "das Ziel der Variable antwortet; der Test wird rot", ["Antwort"], "gemessen")
mac("M7", "fall", GRUND, "Der Agent liest die Wegwerf-Token unter ~/.copilot, ~/.config/gh und ~/.ssh, die Startdatei im "
    "0700-Ordner, listet das Heim und liest eine Plist außerhalb über cfprefsd (defaults read)",
    "jeder Versuch scheitert (EPERM, defaults mit Fehler)", ["errno je Versuch"],
    "gemessen: Messproben D4 bis D8, D19", gemeinsam="Token nicht lesbar")
mac("M7", "gegenprobe", GRUND, OHNE, "jeder Versuch liest; der Test wird rot", ["errno je Versuch"], "gemessen")
mac("M8", "fall", GRUND, "Der Agent sendet dem Such-Prozess Signal 0 und SIGCONT (Sonde und /bin/kill), liest seine "
    "Umgebung (KERN_PROCARGS2), fragt proc_pidinfo und proc_pidpath und ruft task_for_pid",
    "Signal und SIGCONT: EPERM; task_for_pid scheitert. Umgebung, proc_pidinfo und proc_pidpath bleiben lesbar: das ist eine "
    "Grenze des Profils, kein Schutz (Messlauf 2c, H1 bis H3); der Test prüft sie als lesbar und hält die Grenze fest",
    ["errno je Versuch"],
    "gemessen: Messlauf 2c, P1 bis P8 und H1 bis H4 (messlauf2-ergebnis.json); Umgebung P3, H1 lesbar, pidinfo P5, H2 lesbar, "
    "pidpath P6, H3 lesbar", gemeinsam="Such-Prozess nicht signalisierbar")
mac("M8", "gegenprobe", GRUND, OHNE, "Signal gelingt; der Test wird rot", ["errno je Versuch"],
    "gemessen: dieselben Proben ohne Profil (P1, P2, P8, H4)")
mac("M9", "fall", GRUND, "Der Agent verbindet sich mit dem Such-Socket seiner Sitzung und mit dem von s-2 im selben Ordner",
    "eigener Socket antwortet; fremder EPERM", ["Antwort", "errno"], "gemessen: Messproben U1, U2")
mac("M9", "gegenprobe", GRUND, OHNE, "beide Sockets antworten; der Test wird rot", ["Antwort je Socket"], "gemessen")
mac("M10", "fall", GRUND, "Der Agent bindet und hört auf 127.0.0.1", "EPERM beim bind", ["errno"], "gemessen: Messprobe N10")
mac("M10", "gegenprobe", GRUND, OHNE, "bind und listen gelingen; der Test wird rot", ["errno"], "gemessen")
mac("M11", "fall", GRUND, "Der Agent wendet ein offenes Profil an: sandbox_init mit (allow default) und sandbox-exec "
    "mit (allow default) im Lauf; danach liest er die Notiz", "die Notiz bleibt gesperrt; Seatbelt lockert nie",
    ["Ergebnis je Versuch"], "gemessen: Messproben D17, D18")
mac("M11", "gegenprobe", GRUND, OHNE, "die Notiz ist lesbar; der Test wird rot", ["Ergebnis"], "gemessen")
mac("M12", "fall", GRUND, "Der Agent sendet dem Agenten der Sitzung s-2 Signal 0 und dem eigenen Kind Signal 0",
    "fremder Agent: EPERM; eigenes Kind: gelingt", ["errno je Versuch"], "gemessen: Messproben P9, P10")
mac("M12", "gegenprobe", GRUND, OHNE, "Signal an den fremden Agenten gelingt; der Test wird rot", ["errno"], "gemessen")
mac("M13", "fall", GRUND, "Der Agent setzt mit launchctl submit einen Auftrag ab, der die Notiz liest",
    "der Auftrag entsteht nicht, oder er läuft ohne Leserecht auf die Notiz", ["Exitcode von launchctl", "Marke des Auftrags"],
    "gemessen: Messprobe L1")
mac("M13", "gegenprobe", GRUND, OHNE, "der Auftrag liest die Notiz; der Test wird rot", ["Marke des Auftrags"],
    "nicht gemessen")

mac("M14", "fall", GRUND, "Vor jedem Start prüft der Starter das fertige Profil an Kanarien: eine erlaubte Probe "
    "(Arbeitsordner lesen) muss gelingen, gesperrte Proben (Kanarie-Datei außerhalb, fremder Loopback-Port, Signal 0 an einen "
    "Prozess außerhalb) müssen mit EPERM scheitern; dann startet der Lauf",
    "alle Kanarien wie verlangt; der Lauf startet; Dauer der Selbstprüfung unter 1 s",
    ["Ergebnis je Kanarie", "Dauer"], "gemessen: Startkosten 0,013 bis 0,015 s je sandbox-exec (Messlauf 1); "
    "die Kanarien selbst sind die Proben D3, N5, H4 (die Umgebung ist keine Kanarie: sie bleibt lesbar, H1)")
mac("M14", "gegenprobe", GRUND, "Mutanten: (a) Selbstprüfung übersprungen; (b) Attrappe von sandbox-exec ohne Wirkung, "
    "Selbstprüfung meldet trotzdem grün; (c) Kanarie-Fehler ohne EPERM (etwa ENOENT) zählt als gesperrt",
    "jeder Mutant macht den Fall rot: ohne wirksames Profil startet der Lauf nicht (Exit 69, lauf.einschluss_fehlt)",
    ["Exitcode", "stderr-Zeile"], "nicht gemessen")
mac("M15", "fall", GRUND + "; der Lauf startet ein Kind im Hintergrund, einen Enkel nach setsid mit doppeltem fork und "
    "ein Kind mit posix_spawn und POSIX_SPAWN_SETSID",
    "Die Sitzung endet. Der Wächter des Dienstes sendet SIGTERM an die Prozessgruppe und an jeden eigenen Prozess mit dem "
    "Merkmal des Laufs, nach der Frist von 5 s SIGKILL",
    "nach der Frist lebt kein Prozess mit dem Merkmal; kein Prozess ohne Merkmal bekam ein Signal",
    ["PIDs mit Merkmal vor und nach dem Ende", "Zeit bis zum Ende", "Zähler fremder Treffer: 0"],
    "gemessen: Messlauf 2c, prozessende: 2 von 2 abgelöste Prozesse gefunden, 655 eigene Prozesse in 2,375 s geprüft, 0 fremde "
    "Treffer, Ende nach SIGTERM in 0,015 s (messlauf2-ergebnis.json)")
mac("M15", "gegenprobe", GRUND, "Mutant: der Wächter beendet nur die Prozessgruppe des Laufs",
    "der gelöste Enkel und das Kind mit POSIX_SPAWN_SETSID leben nach der Frist; der Test wird rot",
    ["PIDs nach dem Ende"], "gemessen: Messlauf 2c, lebt_nach_killpg: die zwei gelösten Prozesse leben (messlauf2-ergebnis.json)")
mac("M16", "fall", GRUND + "; Lauf mit gelöstem Enkel wie in M15",
    "Der Dienst (Wächter) stirbt durch SIGKILL, solange der Lauf lebt; der gelöste Enkel versucht bis zum Neustart alle "
    "100 ms die Notiz und den Port der Browser-Brücke. Danach startet der Dienst neu und will eine neue Sitzung starten",
    "Bis zum Neustart darf der Enkel leben; jeder seiner Versuche scheitert mit EPERM (er bleibt im Profil). "
    "Vor dem ersten neuen Start sucht der Dienst alle Merkmal-Ordner seines 0700-Ordners, beendet jeden eigenen Prozess "
    "mit einem dieser Merkmale nach der Frist aus M15 und löscht dann die Merkmal-Ordner; erst danach startet die neue Sitzung",
    ["PIDs mit altem Merkmal vor und nach dem Neustart", "Reihenfolge: Aufräumen vor Start"],
    "hergeleitet: dieselbe Suche wie M15 (gemessen), aber ohne Wächter im Lauf; Neustart nicht gemessen")
mac("M16", "gegenprobe", GRUND, "Mutant: kein Aufräumen beim nächsten Start",
    "der gelöste Enkel des alten Laufs lebt nach dem Start der neuen Sitzung; der Test wird rot", ["PIDs mit altem Merkmal"],
    "nicht gemessen")


mac("M17", "fall", GRUND + "; Wegwerf-Repo im Arbeitsordner; Mac mit den Command Line Tools unter /Library/Developer/CommandLineTools",
    "Der Agent ruft git --version, git status --short und git commit --allow-empty im Wegwerf-Repo, mit HOME im Sitzungsheim "
    "und ohne DEVELOPER_DIR",
    "jeder Aufruf: Exit 0; git meldet auf stderr nur die Warnung, dass es die Datei xcrun_db unter /var/folders nicht anlegen "
    "kann; unter dem Ordner der Command Line Tools entsteht keine Datei",
    ["Exitcode je Aufruf", "stderr je Aufruf", "Ergebnis eines Schreibversuchs unter /Library/Developer/CommandLineTools"],
    "gemessen: Messlauf 3 (22.11.), Varianten V1 und W3 in git-ergebnis.json und git-ergebnis2.json")
mac("M17", "gegenprobe", GRUND, "Mutant: das Profil ohne den Ordner /Library/Developer/CommandLineTools unter den Systempfaden",
    "git scheitert mit Exit 1 und der Meldung xcode-select: error: No developer tools were found; der Test wird rot",
    ["Exitcode", "stderr"], "gemessen: Messlauf 3, V0 und W0 (zuvor H9, Messlauf 2c)")


def main():
    (HIER / "einschluss.schema.json").write_text(json.dumps(SCHEMA, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
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
