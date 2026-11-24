#!/usr/bin/env python3
"""Prüft die sichtbaren Fälle des Vertrags Einschluss unter macOS.

Aufruf: <python mit jsonschema> -I rust/spec/macos-einschluss/pruefen.py
Ohne jsonschema endet es mit Exit 2 und einer Zeile auf stderr. Wer bewusst ohne Schema prüfen will, ruft es mit
--ohne-schema auf: dann prüft es das Muster posixpfad mit re gegen die Referenz statt des Schemas, meldet das Schema in der
Ausgabe als nicht geprüft und endet mit Exit 0 nur bei 0 Fehlern.
Geprüft wird: Paarung faelle/erwartet, Pflichtfelder je Form, Größe und JSON-Lesen, die Einschlussdatei gegen eine
Referenz der Stufen 1 bis 8 und gegen das Schema, die Bereitschaftszeilen gegen eine Referenz,
jeder Fall der Form profil gegen den Referenzbau des Profils aus dem README (Abschnitt "Profil"), die stderr-Zeilen der
Form aufruf gegen Vertrag 1, jeden Fall der Form profiltext gegen die zweite Prüfung und je Eigenschaft M1 bis M17 genau ein Fall und eine Gegenprobe mit Anforderung und Beleg.
Erst mit dem Bau prüfbar: Stufen 10 bis 13 der Einschlussdatei, alle Fälle der Formen aufruf und mac.
Exit 0, wenn jeder prüfbare Punkt stimmt.
"""
import base64
import json
import re
import sys
import unicodedata
from pathlib import PurePosixPath

from pathlib import Path

HIER = Path(__file__).resolve().parent
GRENZE = 4096
EIGENSCHAFTEN = [f"M{i}" for i in range(1, 18)]
ANFORDERUNGEN = {"A142", "A143", "A144", "A145"}
SCHEMA_CODES = {"schluessel_unbekannt", "schluessel_fehlt", "typ_falsch", "fassung_unbekannt", "liste_grenze", "pfad_form"}
BAU_CODES = {"pfad_ueberlappt", "pfad_geschuetzt", "pfad_fehlt", "pfad_umgelenkt"}
# Vertrag 1, feste Gründe der lauf.*-Codes
GRUENDE = {
    "lauf.aufruf": "genau ein Pfad erwartet", "lauf.datei_unlesbar": "Startdatei nicht lesbar",
    "lauf.datei_modus": "Startdatei nicht eigen, nicht 0600 oder keine Datei",
    "lauf.einst_unlesbar": "Einstellungsdatei nicht lesbar", "lauf.ordner_fehlt": "Arbeitsordner fehlt oder ist kein Ordner",
    "lauf.einschluss_fehlt": "Einschluss nicht verfuegbar", "lauf.suche_fehlt": "Suche nicht startbar",
    "lauf.suche_nicht_bereit": "Suche nicht bereit", "lauf.harness_fehlt": "Harness nicht startbar",
    "lauf.intern": "interner Fehler",
}
STEUER = re.compile(r"[\x00-\x1f\x7f-\x9f]")


def streng_lesen(roh):
    if roh.startswith(b"\xef\xbb\xbf"):
        raise ValueError("BOM")
    text = roh.decode("utf-8")

    def paare(liste):
        obj = {}
        for k, v in liste:
            if k in obj:
                raise ValueError("doppelter Schlüssel")
            obj[k] = v
        return obj

    obj = json.loads(text, object_pairs_hook=paare, parse_float=lambda s: ("bruch", s))
    if not isinstance(obj, dict):
        raise ValueError("kein Objekt")
    return obj


# ---- Pfade -----------------------------------------------------------------------------------------
def posixpfad(p):
    """Absoluter, normalisierter POSIX-Pfad ohne Steuerzeichen, `"` und `\\`, höchstens 1024 Byte (README, Einschlussdatei)."""
    if not isinstance(p, str) or not p.startswith("/") or p == "/" or p.endswith("/") or "//" in p:
        return False
    if len(p.encode("utf-8")) > 1024 or STEUER.search(p) or '"' in p or "\\" in p:
        return False
    return all(s not in (".", "..") for s in p.split("/")[1:])


def falt(p):
    """Vergleichsform: APFS unterscheidet Groß- und Kleinschreibung in der Vorgabe nicht."""
    return unicodedata.normalize("NFD", p).casefold()


def innerhalb(kind, eltern):
    """kind ist eltern oder liegt darunter (ohne Rücksicht auf Groß- und Kleinschreibung)."""
    k, e = falt(kind), falt(eltern)
    return k == e or k.startswith(e.rstrip("/") + "/")


def ueberlappt(a, b):
    return innerhalb(a, b) or innerhalb(b, a)


# ---- Referenz der Einschlussdatei (Stufen 1 bis 8) -------------------------------------------------
def konfig_referenz(roh):
    if len(roh) > GRENZE:
        return "einschluss.zu_gross"
    try:
        obj = streng_lesen(roh)
    except (ValueError, UnicodeDecodeError):
        return "einschluss.json_ungueltig"
    pflicht = {"fassung", "projektwurzeln", "programmordner"}
    if set(obj) - pflicht:
        return "einschluss.schluessel_unbekannt"
    if pflicht - set(obj):
        return "einschluss.schluessel_fehlt"
    f = obj["fassung"]
    if not isinstance(f, int) or isinstance(f, bool) or not all(isinstance(obj[k], list) for k in ("projektwurzeln", "programmordner")):
        return "einschluss.typ_falsch"
    if any(not isinstance(x, str) for k in ("projektwurzeln", "programmordner") for x in obj[k]):
        return "einschluss.typ_falsch"
    if f != 1:
        return "einschluss.fassung_unbekannt"
    for k in ("projektwurzeln", "programmordner"):
        if not 1 <= len(obj[k]) <= 16 or len(set(obj[k])) != len(obj[k]):
            return "einschluss.liste_grenze"
    if any(not posixpfad(x) for k in ("projektwurzeln", "programmordner") for x in obj[k]):
        return "einschluss.pfad_form"
    return None


# ---- Referenz der Bereitschaftszeile ---------------------------------------------------------------
def bereit_referenz(roh):
    if not roh.endswith(b"\n") or roh.count(b"\n") != 1:
        return False
    try:
        obj = streng_lesen(roh[:-1])
    except (ValueError, UnicodeDecodeError):
        return False
    if set(obj) != {"bereit", "port"} or obj["bereit"] is not True:
        return False
    p = obj["port"]
    return isinstance(p, int) and not isinstance(p, bool) and 1 <= p <= 65535


# ---- Referenzbau des Profils (README, Abschnitt "Profil") ------------------------------------------
SYSTEM_LESEN = ["/usr", "/bin", "/sbin", "/System", "/Library/Apple", "/Library/Developer/CommandLineTools",
                "/private/etc", "/private/var/db/timezone", "/private/var/db/dyld", "/private/var/select"]
SYSTEM_VERBOTEN = ["/", "/Users", "/System", "/Library", "/Applications", "/private", "/private/tmp", "/private/var",
                   "/tmp", "/var", "/etc", "/dev", "/Volumes", "/usr", "/bin", "/sbin", "/opt", "/cores"]
SYSTEM_UNTER = ["/System", "/usr", "/bin", "/sbin", "/Library", "/Applications", "/private/etc", "/private/var/db",
                "/dev", "/etc", "/var", "/opt/homebrew"]
GEHEIMNIS_TEILE = [".aws", ".config", ".docker", ".gnupg", ".kube", ".secrets-sync", ".ssh", "90-secrets", ".env",
                   ".copilot", "Keychains"]
ANMELDESPEICHER = [".ssh", ".gnupg", ".aws", ".azure", ".kube", ".docker", ".copilot", ".config/gh", ".config/gcloud",
                   ".config/op", ".password-store", ".netrc", ".git-credentials", ".npmrc", ".pypirc", ".cargo/credentials.toml",
                   ".claude", ".claude.json", ".codex", ".pi", ".gemini", ".qwen", ".local/share/opencode",
                   "Library/Keychains", "Library/Cookies", "Library/Safari", "Library/Mail", "Library/Messages",
                   "Library/Application Support/Google/Chrome", "Library/Application Support/Firefox",
                   "Library/Application Support/BraveSoftware", "Library/Application Support/Microsoft Edge",
                   "Library/Application Support/1Password", "Library/Group Containers"]
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
SUN_PATH = 103


class Abgelehnt(Exception):
    pass


def q(p):
    return '"%s"' % p


def bauen(b):
    """Gibt den Profiltext oder wirft Abgelehnt(<Fehlername wie im Crate>)."""
    # 1. Form
    pfade = [b["arbeitsordner"], b["zustand"], b["heim"], b["such_socket"], b["merkmal"]] + b["programmordner"] + b["geschuetzt"]
    if not KENNUNG.match(b["harness"]) or not b["argv"] or any("\0" in a or not a for a in b["argv"]):
        raise Abgelehnt("BeschreibungUngueltig")
    if any(not posixpfad(p) for p in pfade):
        raise Abgelehnt("BeschreibungUngueltig")
    sp = b["sitzungsports"]
    ports = ([sp["modellweg"]] if "modellweg" in sp else []) + ([sp["ausgang"]] if "ausgang" in sp else [])
    if not ports or "modellweg" not in sp:  # eine Netzregel ohne Filter gibt es nie
        raise Abgelehnt("BeschreibungUngueltig")
    if any(not isinstance(p, int) or isinstance(p, bool) or not 1 <= p <= 65535 for p in ports) or len(set(ports)) != len(ports):
        raise Abgelehnt("BeschreibungUngueltig")
    if len(b["such_socket"].encode("utf-8")) > SUN_PATH:
        raise Abgelehnt("BeschreibungUngueltig")
    u = b["umgebung"]
    if u["home"] != b["zustand"] or not posixpfad(u["tmpdir"]) or not innerhalb(u["tmpdir"], b["zustand"]) or u["tmpdir"] == b["zustand"]:
        raise Abgelehnt("BeschreibungUngueltig")
    if any(not w or STEUER.search(w) for w in (u["path"], u["term"], u["lang"])):
        raise Abgelehnt("BeschreibungUngueltig")
    # 2. Verbote
    arbeit, heim = b["arbeitsordner"], b["heim"]
    if any(falt(arbeit) == falt(s) for s in SYSTEM_VERBOTEN) or any(innerhalb(arbeit, s) for s in SYSTEM_UNTER) \
            or innerhalb(heim, arbeit):
        raise Abgelehnt("Verboten")
    if any(falt(heim) == falt(s) for s in SYSTEM_VERBOTEN) or any(innerhalb(heim, s) for s in SYSTEM_UNTER):
        raise Abgelehnt("Verboten")
    freigaben = [arbeit, b["zustand"]] + b["programmordner"]
    speicher = [heim + "/" + s for s in ANMELDESPEICHER]
    for p in freigaben:
        if any(t in GEHEIMNIS_TEILE for t in PurePosixPath(p).parts):
            raise Abgelehnt("Verboten")
        if any(ueberlappt(p, s) for s in speicher) or innerhalb(heim, p):
            raise Abgelehnt("Verboten")
    # 3. Überlappungen
    for i, x in enumerate(freigaben):
        for y in freigaben[i + 1:]:
            if ueberlappt(x, y):
                raise Abgelehnt("Ueberlappt")
        if any(ueberlappt(x, g) for g in b["geschuetzt"]):
            raise Abgelehnt("Ueberlappt")
        if innerhalb(b["such_socket"], x) or innerhalb(b["merkmal"], x):
            raise Abgelehnt("Ueberlappt")
    # 4. Text
    meta = sorted({str(a) for p in [arbeit, b["zustand"]] + b["programmordner"] for a in PurePosixPath(p).parents})
    netz = " ".join('(remote ip "localhost:%d")' % p for p in ports)
    zeilen = [
        "(version 1)",
        "(deny default)",
        '(import "system.sb")',
        "(allow process-fork)",
        "(allow process-exec)",
        "(allow signal (target same-sandbox))",
        "(allow process-info* (target same-sandbox))",
        "(allow sysctl-read)",
        "(allow pseudo-tty)",
        '(allow file-ioctl (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$") (literal "/dev/ptmx"))',
        '(allow file-read* file-write-data (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$") (literal "/dev/ptmx")'
        ' (literal "/dev/null") (literal "/dev/zero"))',
        "(allow file-read* file-map-executable %s)" % " ".join("(subpath %s)" % q(p) for p in SYSTEM_LESEN + b["programmordner"]),
        "(allow file-read-metadata %s)" % " ".join("(literal %s)" % q(p) for p in meta),
        "(allow file-read* file-write* (subpath %s) (subpath %s))" % (q(arbeit), q(b["zustand"])),
        "(allow network-outbound %s)" % netz,
        "(allow network-outbound (remote unix-socket (path-literal %s)))" % q(b["such_socket"]),
        "(deny file-read* file-write* %s)" % " ".join("(subpath %s)" % q(p) for p in b["geschuetzt"] + speicher),
        '(deny sysctl-read (sysctl-name "kern.procargs2") (sysctl-name "kern.procargs") (sysctl-name-prefix "kern.proc."))',
        "(deny process-info* (target others))",
        "(allow file-write-data (literal %s))" % q(b["merkmal"]),
    ]
    return "\n".join(zeilen) + "\n"


# Regeln ohne Filter, die der Bau erzeugt und die harmlos sind; jede andere Regel ohne Filter lehnt die zweite Prüfung ab.
OHNE_FILTER_ERLAUBT = {"process-fork", "process-exec", "sysctl-read", "pseudo-tty"}
VERBOTEN_IM_TEXT = ("(allow default)", "(allow network*", "(allow network-outbound)", "(allow network-bind",
                    "(allow network-inbound", "(allow file-read*)", "(allow file-write*)", "(allow mach",
                    '(remote ip "*', "(remote tcp", "(remote udp")
FILTERLOSE_REGEL = re.compile(r"\(allow ([^\s()]+(?: [^\s()]+)*)\)")


def normalisiert(text):
    """Jeden Leerraum (Leerzeichen, Tab, Zeilenumbruch, mehrere) zu einem Leerzeichen, keins nach "(" und vor ")"."""
    t = re.sub(r"\s+", " ", text)
    return t.replace("( ", "(").replace(" )", ")")


def profil_pruefen(text):
    """Die zweite Prüfung am fertigen Text (README, "Prüfung am Text"). Die Vergleiche laufen über den normalisierten Text."""
    z = text.splitlines()
    n = normalisiert(text)
    if z[:3] != ["(version 1)", "(deny default)", '(import "system.sb")'] or n.count("(deny default)") != 1:
        return False
    if any(v in n for v in VERBOTEN_IM_TEXT):
        return False
    for treffer in FILTERLOSE_REGEL.findall(n):
        if any(op not in OHNE_FILTER_ERLAUBT for op in treffer.split(" ")):
            return False
    return z[-1].startswith("(allow file-write-data (literal ") and z[-3].startswith("(deny sysctl-read")


# ---- Hauptteil -------------------------------------------------------------------------------------
def main():
    fehler = []
    faelle = {p.name: p for p in (HIER / "faelle").glob("*.json")}
    erwartet = {p.name: p for p in (HIER / "erwartet").glob("*.json")}
    for n in sorted(set(faelle) ^ set(erwartet)):
        fehler.append(f"{n}: ohne Gegenstück")
    try:
        import jsonschema
        schema = json.loads((HIER / "einschluss.schema.json").read_text(encoding="utf-8"))
        jsonschema.Draft202012Validator.check_schema(schema)

        def gegen_schema(obj, teil):
            v = jsonschema.Draft202012Validator({**schema, "$ref": f"#/$defs/{teil}"})
            return v.is_valid(obj)
        schema_stand = "geprüft"
    except ImportError:
        if "--ohne-schema" not in sys.argv[1:]:
            print("pruefen.py: das Modul jsonschema fehlt, das Schema wäre ungeprüft; Exit 2 "
                  "(mit --ohne-schema bewusst ohne Schema prüfen)", file=sys.stderr)
            return 2
        gegen_schema = None
        schema_stand = "NICHT geprüft (--ohne-schema, jsonschema fehlt); Muster posixpfad mit re gegen die Referenz geprüft"
        schema = json.loads((HIER / "einschluss.schema.json").read_text(encoding="utf-8"))
        muster = re.compile(schema["$defs"]["posixpfad"]["pattern"])
        for p in ["/a", "/a/b c", "/a/.x", "/a/...x", "/Übungen", "/", "/a/", "//a", "/a/./b", "/a/../b", "/a\tb",
                  '/a"b', "/a\\b", "/a\u0085b", "a/b", "~/a"]:
            if bool(muster.fullmatch(p)) != posixpfad(p):
                fehler.append(f"Schema-Muster posixpfad widerspricht der Referenz bei {p!r}")
    abdeckung = {}
    zahl = {}
    for name in sorted(set(faelle) & set(erwartet)):
        try:
            f = json.loads(faelle[name].read_text(encoding="utf-8"))
            e = json.loads(erwartet[name].read_text(encoding="utf-8"))
        except ValueError as x:
            fehler.append(f"{name}: kein JSON ({x})")
            continue
        form = f.get("form")
        zahl[form] = zahl.get(form, 0) + 1
        if not f.get("beschreibung"):
            fehler.append(f"{name}: beschreibung fehlt")
        if form == "konfig":
            roh = base64.b64decode(f["b64"]) if "b64" in f else f["text"].encode("utf-8")
            ref = konfig_referenz(roh)
            soll = e.get("code") if not e.get("gueltig") else None
            code = (soll or "").split(".", 1)[-1]
            if soll and code in BAU_CODES:
                if ref is not None:
                    fehler.append(f"{name}: Stufe {soll} braucht den Bau, Referenz meldet schon {ref}")
            elif ref != soll:
                fehler.append(f"{name}: Referenz {ref}, erwartet {soll}")
            if gegen_schema and ref not in ("einschluss.zu_gross", "einschluss.json_ungueltig"):
                obj = streng_lesen(roh)
                gilt = gegen_schema(obj, "konfig")
                if gilt != (ref is None):
                    fehler.append(f"{name}: Schema {'nimmt an' if gilt else 'lehnt ab'}, Referenz {ref}")
        elif form == "bereit":
            roh = base64.b64decode(f["b64"])
            if bereit_referenz(roh) != e.get("gueltig"):
                fehler.append(f"{name}: Bereitschaftszeile, erwartet gueltig={e.get('gueltig')}")
            if gegen_schema and roh.endswith(b"\n") and roh.count(b"\n") == 1:
                try:
                    obj = streng_lesen(roh[:-1])
                except ValueError:
                    obj = None
                if obj is not None and not any(isinstance(v, tuple) for v in obj.values()):
                    if gegen_schema(obj, "bereit") != e.get("gueltig"):
                        fehler.append(f"{name}: Schema widerspricht der Referenz")
        elif form == "profil":
            b = f["eingabe"]
            if gegen_schema and not gegen_schema(b, "beschreibung") and e.get("fehler") != "BeschreibungUngueltig":
                fehler.append(f"{name}: Schema lehnt die Beschreibung ab")
            try:
                text = bauen(b)
                ist = None
            except Abgelehnt as x:
                ist, text = str(x), ""
            if e.get("gueltig"):
                if ist:
                    fehler.append(f"{name}: Referenz lehnt ab ({ist})")
                    continue
                if not profil_pruefen(text):
                    fehler.append(f"{name}: Prüfung am Text schlägt fehl")
                for s in e["enthaelt"]:
                    if s not in text:
                        fehler.append(f"{name}: fehlt {s!r}")
                for s in e["enthaelt_nicht"]:
                    if s in text:
                        fehler.append(f"{name}: enthält {s!r}")
            elif ist != e.get("fehler"):
                fehler.append(f"{name}: Referenz {ist}, erwartet {e.get('fehler')}")
        elif form == "profiltext":
            # Der Referenzbau, danach ein Zusatz vor den drei letzten Zeilen; die zweite Prüfung muss ihn ablehnen
            # (oder bei "gueltig": annehmen).
            zeilen = bauen(f["eingabe"]).splitlines()
            zeilen[-3:-3] = [f["einfuegen"]]
            nimmt_an = profil_pruefen("\n".join(zeilen) + "\n")
            if nimmt_an != e.get("gueltig"):
                fehler.append(f"{name}: zweite Prüfung {'nimmt an' if nimmt_an else 'lehnt ab'}, erwartet gueltig={e.get('gueltig')}")
        elif form == "aufruf":
            if "exec" in e:
                if e.get("stderr") != "" or e["exec"][:2] != ["/usr/bin/sandbox-exec", "-f"]:
                    fehler.append(f"{name}: exec-Fall muss stumm in /usr/bin/sandbox-exec -f gehen")
            else:
                z = e.get("stderr", "")
                code = z.split(":", 1)[0]
                if not (z.endswith("\n") and z.count("\n") == 1 and z.isascii() and len(z.encode()) <= 200
                        and z == f"{code}: {GRUENDE.get(code, '?')}\n"):
                    fehler.append(f"{name}: stderr-Zeile verletzt Vertrag 1")
                if e.get("exit") not in (64, 65, 66, 69, 70, 78):
                    fehler.append(f"{name}: Exitcode außerhalb von Vertrag 1")
        elif form == "mac":
            m = f.get("eigenschaft")
            abdeckung.setdefault(m, []).append(f.get("art"))
            if f.get("anforderung") not in ANFORDERUNGEN:
                fehler.append(f"{name}: Anforderung fehlt oder unbekannt")
            if not str(f.get("beleg", "")).startswith(("gemessen", "hergeleitet", "nicht gemessen")):
                fehler.append(f"{name}: Beleg ohne Stand")
            for feld in ("ausgangslage", "handlung"):
                if not f.get(feld):
                    fehler.append(f"{name}: {feld} fehlt")
            if not e.get("erwartet") or not e.get("messwerte"):
                fehler.append(f"{name}: erwartet oder messwerte fehlt")
        else:
            fehler.append(f"{name}: Form {form!r} unbekannt")
    for m in EIGENSCHAFTEN:
        if sorted(abdeckung.get(m, [])) != ["fall", "gegenprobe"]:
            fehler.append(f"{m}: braucht genau einen Fall und eine Gegenprobe, hat {abdeckung.get(m)}")
    for m in set(abdeckung) - set(EIGENSCHAFTEN):
        fehler.append(f"{m}: Eigenschaft unbekannt")
    print(f"Fälle: {sum(zahl.values())} ({', '.join(f'{k} {v}' for k, v in sorted(zahl.items()))}); Schema: {schema_stand}")
    for x in fehler:
        print("FEHLER", x)
    print(f"{len(fehler)} Fehler")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
