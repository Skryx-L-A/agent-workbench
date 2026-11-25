#!/usr/bin/env python3
"""Referenzprüfer des Vertrags Paketbau und Prüfbefehl (Python, nur für die sichtbaren Fälle).

Er legt die Reihenfolge der Stufen so fest, wie README.md sie beschreibt. Die Umsetzung in Rust muss mit ihm
übereinstimmen; die verdeckten Fälle des Baus prüfen beide gegeneinander. Er braucht nur die Standardbibliothek.
"""
import json
import re

# ---------------------------------------------------------------------------------------------------- Manifest

MANIFEST_GRENZE = 1048576
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
PFAD = re.compile(r"^[a-z0-9._-]+(/[a-z0-9._-]+)*$")
PFAD_MAC = re.compile(r"^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$")
VERSION = re.compile(r"^(0|[1-9][0-9]{0,2})\.(0|[1-9][0-9]{0,2})\.(0|[1-9][0-9]{0,4})$")
SHA = re.compile(r"^[0-9a-f]{64}$")
RESERVIERT = {"con", "prn", "aux", "nul"} | {f"com{i}" for i in range(1, 10)} | {f"lpt{i}" for i in range(1, 10)}
ARTEN = ("programm", "dienst", "daten", "lizenz")
PLATTFORMEN = ("windows-x64", "ubuntu-26.04-x64", "macos-arm64")
# Das AppArmor-Profil gibt es in zwei Formen (Entscheidung vom 20.11.2026, Nachtrag zur Folge 1); auf 26.04.1 gemessen und entschieden: Form A, Fassung V1.
# Form A (Entscheidung V1, 20.11.): Allein der feste Starter `/usr/lib/werkbank/werkbank-einschluss` trägt das Elternprofil mit dem Übergang
# `/usr/bin/bwrap Px -> werkbank-einschluss-bwrap` in ein Kindprofil; `werkbank-lauf` hat kein Profil. Form A besteht aus der Profildatei
# und dem Starter.
# Form B: eine eigene Kopie `/usr/lib/werkbank/bwrap` (vom Skript `postinst` aus `/usr/bin/bwrap` kopiert, nicht im Paket) mit Anhang auf genau diesen Pfad.
# Mindestens eine Form muss vollständig im Manifest stehen. Die Dateinamen sind Annahme, bis das Ergebnis vorliegt.
PROFIL_FORMEN = {
    "ubuntu-26.04-x64": (
        (("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss", "daten"), ("usr/lib/werkbank/werkbank-einschluss", "programm")),
        (("etc/apparmor.d/usr.lib.werkbank.bwrap", "daten"),),
    ),
}
MANIFEST_ORT = {"windows-x64": "manifest.json", "ubuntu-26.04-x64": "usr/share/werkbank/manifest.json",
                "macos-arm64": "Library/Werkbank/manifest.json"}
BAUARTEN = ("auslieferung", "test")
TOP_PFLICHT = ("fassung", "paket", "version", "plattform", "bau", "merkmale", "dateien", "bibliotheken")
DATEI_PFLICHT = ("pfad", "groesse", "sha256", "art")
MODULE_DREHBUCHLIB = ("ablauf", "bericht", "bruecke", "fremd", "konfig", "lauf", "messen", "modell", "probelauf", "schritte", "seite", "weiterreicher", "ziel")
DREHBUCHLIB = tuple((f"usr/lib/werkbank/probelauf/drehbuchlib/{m}.py", "programm") for m in MODULE_DREHBUCHLIB)
PFLICHTDATEIEN = {
    "windows-x64": (("bin/wb.exe", "programm"), ("bin/wb-halter.exe", "dienst"),
                    ("bin/wb-plattform.exe", "dienst"), ("lizenzen/sbom.cdx.json", "daten")),
    "macos-arm64": (
        ("Library/Werkbank/bin/werkbank-einrichten", "programm"), ("Library/Werkbank/bin/werkbank-pruefen", "programm"),
        ("Library/Werkbank/lib/werkbank-halter", "dienst"), ("Library/Werkbank/lib/werkbank-lauf", "programm"),
        ("Library/LaunchDaemons/werkbank.halter.plist", "daten"), ("Library/Werkbank/sbom.cdx.json", "daten"),
    ),
    "ubuntu-26.04-x64": (
        ("usr/bin/werkbank-probelauf", "programm"), ("usr/bin/werkbank-pruefen", "programm"), ("usr/bin/werkbank-ordnermarke", "programm"), ("usr/bin/werkbank-ca-zulassung", "programm"),
        ("usr/lib/werkbank/werkbank-lauf", "programm"), ("usr/lib/werkbank/werkbank-lauf-kind", "programm"), ("usr/lib/werkbank/werkbank-kern", "dienst"), ("usr/lib/werkbank/werkbank-bruecke", "dienst"),
        ("usr/lib/werkbank/werkbank-modellweg", "dienst"), ("usr/lib/werkbank/werkbank-halter", "dienst"), ("usr/lib/werkbank/rangkern", "programm"),
        ("usr/lib/werkbank/kette", "programm"), ("usr/lib/werkbank/such-mcp", "programm"), ("usr/lib/werkbank/probelauf/drehbuch.py", "programm"),
        *DREHBUCHLIB,
        ("usr/share/werkbank/probelauf/einstellungen.json", "daten"), ("usr/share/werkbank/probelauf/konfig.json", "daten"),
        ("usr/lib/systemd/user/werkbank-kern.service", "daten"),
        ("usr/lib/systemd/user/werkbank-bruecke.service", "daten"), ("usr/lib/systemd/user/wb-halter.service", "daten"),
        ("usr/share/werkbank/sbom.cdx.json", "daten"),
    ),
}


def streng_lesen(roh):
    """Liest Bytes als ein JSON-Objekt; wirft ValueError bei BOM, Nicht-UTF-8, Syntaxfehler, Doppel, Nachlauf."""
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

    obj = json.loads(text, object_pairs_hook=paare)
    if not isinstance(obj, dict):
        raise ValueError("kein Objekt")
    return obj


def ist_ganz(x):
    return isinstance(x, int) and not isinstance(x, bool)


def pfad_gut(p, plattform=None):
    eigener = (MANIFEST_ORT[plattform],) if plattform in MANIFEST_ORT else tuple(MANIFEST_ORT.values())
    muster = PFAD_MAC if plattform == "macos-arm64" else PFAD
    if len(p) > 200 or not muster.match(p) or p in eigener:
        return False
    for seg in p.split("/"):
        if seg in (".", "..") or seg.endswith("."):
            return False
        if seg.split(".")[0] in RESERVIERT:
            return False
    return True


def manifest_code(roh):
    """Gibt den Code der ersten verletzten Stufe (ohne Präfix `paket.`) oder None zurück."""
    if len(roh) > MANIFEST_GRENZE:
        return "zu_gross"
    try:
        m = streng_lesen(roh)
    except (ValueError, UnicodeDecodeError):
        return "json_ungueltig"
    dateien = m.get("dateien")
    liste = dateien if isinstance(dateien, list) else []
    dicts = [d for d in liste if isinstance(d, dict)]
    # 3 unbekannte Schlüssel
    if set(m) - set(TOP_PFLICHT) or any(set(d) - set(DATEI_PFLICHT) for d in dicts):
        return "schluessel_unbekannt"
    # 4 fehlende Pflichtschlüssel
    if any(k not in m for k in TOP_PFLICHT) or any(k not in d for d in dicts for k in DATEI_PFLICHT):
        return "schluessel_fehlt"
    # 5 Typen
    ok = (ist_ganz(m["fassung"]) and all(isinstance(m[k], str) for k in ("paket", "version", "plattform", "bau"))
          and isinstance(m["merkmale"], list) and all(isinstance(x, str) for x in m["merkmale"])
          and isinstance(dateien, list) and all(isinstance(d, dict) for d in dateien)
          and ist_ganz(m["bibliotheken"]))
    if ok:
        ok = all(isinstance(d["pfad"], str) and ist_ganz(d["groesse"]) and isinstance(d["sha256"], str)
                 and isinstance(d["art"], str) for d in dateien)
    if not ok:
        return "typ_falsch"
    # 6 Fassung
    if m["fassung"] != 1:
        return "fassung_unbekannt"
    # 7 Anzahl
    if not 1 <= len(dateien) <= 2000 or len(m["merkmale"]) > 16 or not 1 <= m["bibliotheken"] <= 100000:
        return "anzahl"
    # 8 Version
    v = VERSION.match(m["version"])
    if not v or int(v.group(1)) > 255 or int(v.group(2)) > 255 or int(v.group(3)) > 65535:
        return "version_form"
    # 9 Werte
    if (m["paket"] != "werkbank" or m["plattform"] not in PLATTFORMEN or m["bau"] not in BAUARTEN
            or any(d["art"] not in ARTEN for d in dateien)):
        return "wert_unbekannt"
    # 10 Kennung der Merkmale
    if any(not KENNUNG.match(x) for x in m["merkmale"]):
        return "kennung_form"
    # 11 Pfade
    if any(not pfad_gut(d["pfad"], m["plattform"]) for d in dateien):
        return "pfad_form"
    # 12 Prüfsumme
    if any(not SHA.match(d["sha256"]) for d in dateien):
        return "pruefsumme_form"
    # 13 Größe
    if any(not 0 <= d["groesse"] <= 2147483647 for d in dateien):
        return "groesse_bereich"
    # 14 doppelte Pfade
    pfade = [d["pfad"].lower() for d in dateien]
    if len(set(pfade)) != len(pfade):
        return "pfad_doppelt"
    # 15 doppelte Merkmale
    if len(set(m["merkmale"])) != len(m["merkmale"]):
        return "merkmal_doppelt"
    # 16 verbotene Merkmale im Auslieferungsbau
    if m["bau"] == "auslieferung" and any(x == "attrappe" or x.startswith("test") for x in m["merkmale"]):
        return "merkmal_verboten"
    # 17 Pflichtdateien
    vorhanden = {(d["pfad"], d["art"]) for d in dateien}
    if any(p not in vorhanden for p in PFLICHTDATEIEN[m["plattform"]]):
        return "pflichtdatei_fehlt"
    formen = PROFIL_FORMEN.get(m["plattform"])
    if formen and not any(all(p in vorhanden for p in form) for form in formen):
        return "pflichtdatei_fehlt"
    return None


# ---------------------------------------------------------------------------------------------------- Listen

GRUND_MIN = 12
SBOM_ZIELE = ("lizenzen/sbom.cdx.json", "usr/share/werkbank/sbom.cdx.json")
VORFUEHR_ZIEL = "usr/share/werkbank/probelauf/einstellungen.json"
VORFUEHR_QUELLE = "datei:spec/einstellungen/vorfuehrung.json"
LISTE_ARTEN = ("programm", "dienst", "daten", "lizenz")
QUELLE_BIN = re.compile(r"^bin:([a-z0-9][a-z0-9._-]{0,63}):([a-z0-9][a-z0-9._-]{0,63})$")
QUELLE_DATEI = re.compile(r"^datei:([A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*)$")
QUELLE_ERZEUGT = re.compile(r"^erzeugt:(sbom)$")
MUSTER_ENDUNG = re.compile(r"^\*\.[a-z0-9]{1,16}$")
MUSTER_ORDNER = re.compile(r"^[a-z0-9._-]+(/[a-z0-9._-]+)*/$")
MUSTER_CRATE = re.compile(r"^crate:([a-z0-9][a-z0-9._-]{0,63})$")


def zeilen_ohne_kommentar(text):
    for z in text.split("\n"):
        if z.strip() == "" or z.startswith("#"):
            continue
        yield z


def liste_lesen(text, plattform=None):
    """Positivliste -> Liste von (art, ziel, quelle); wirft ValueError bei einer Formverletzung."""
    ergebnis = []
    for z in zeilen_ohne_kommentar(text):
        teile = z.split()
        if len(teile) != 3:
            raise ValueError(z)
        art, ziel, quelle = teile
        if art not in LISTE_ARTEN or not pfad_gut(ziel, plattform):
            raise ValueError(z)
        if not (QUELLE_BIN.match(quelle) or QUELLE_ERZEUGT.match(quelle)):
            d = QUELLE_DATEI.match(quelle)
            if not d or any(s in (".", "..") for s in d.group(1).split("/")):
                raise ValueError(z)
        if QUELLE_ERZEUGT.match(quelle) and ziel not in SBOM_ZIELE:
            raise ValueError(z)
        ergebnis.append((art, ziel, quelle))
    return ergebnis


def ausschluss_lesen(text):
    """Ausschlussliste -> Liste von Mustern; wirft ValueError bei Formverletzung oder Grund unter 12 Zeichen."""
    muster = []
    for z in zeilen_ohne_kommentar(text):
        if "  # " not in z:
            raise ValueError(z)
        m, grund = z.split("  # ", 1)
        if m != m.strip() or len(grund.strip()) < GRUND_MIN or " " in m:
            raise ValueError(z)
        if not (PFAD.match(m) or MUSTER_ENDUNG.match(m) or MUSTER_ORDNER.match(m) or MUSTER_CRATE.match(m)):
            raise ValueError(z)
        muster.append(m)
    return muster


def trifft(muster, pfad):
    if muster.startswith("crate:"):
        return False
    if muster.startswith("*."):
        return pfad.rsplit("/", 1)[-1].endswith(muster[1:])
    if muster.endswith("/"):
        return pfad.startswith(muster)
    return pfad == muster


def modus_code(modi):
    """Stufe 9 der Gegenprobe: `modi` bildet jeden Eintrag des gestuften Baums (Datei, Ordner mit `/` am Ende) auf seinen Modus als
    Oktaltext ab. Gibt `modus_schreibbar` zurück, wenn einer für die Gruppe oder die Welt schreibbar ist, sonst None."""
    return "modus_schreibbar" if any(int(m, 8) & 0o022 for m in modi.values()) else None


def liste_code(fall):
    """Fall der Form `liste`; gibt den Code (ohne Präfix) oder None zurück."""
    try:
        positiv = liste_lesen(fall["liste"], fall.get("plattform"))
    except ValueError:
        return "liste_form"
    try:
        ausschluss = ausschluss_lesen(fall["ausschluss"])
    except ValueError:
        return "ausschluss_form"
    ziele = [z for _, z, _ in positiv]
    if len(set(ziele)) != len(ziele):
        return "liste_doppelt"
    if any(q not in fall["quellen"] for _, _, q in positiv):
        return "quelle_fehlt"
    if any(z == VORFUEHR_ZIEL and q != VORFUEHR_QUELLE for _, z, q in positiv):
        return "vorfuehrung_einstellung"
    manifest_orte = set(MANIFEST_ORT.values())
    baum = [p.lower() for p in fall["baum"] if p.lower() not in manifest_orte]
    gesperrte_crates = {m[len("crate:"):] for m in ausschluss if m.startswith("crate:")}
    for _, z, q in positiv:
        b = QUELLE_BIN.match(q)
        if b and b.group(1) in gesperrte_crates:
            return "ausschluss_getroffen"
    if any(trifft(m, p) for m in ausschluss for p in set(ziele) | set(baum)):
        return "ausschluss_getroffen"
    if any(p not in set(ziele) for p in baum):
        return "ungelistet"
    if any(z not in set(baum) for z in ziele):
        return "fehlt_im_baum"
    # Stufe 9: `modi` ist optional (Pfad oder `<ordner>/` mit Modus als Oktaltext); ein Eintrag mit Schreibrecht für Gruppe oder Welt ist verboten.
    if fall.get("schritt") == 2:
        return modus_code(fall.get("modi", {})) or setgid_code(fall.get("modi", {}), fall.get("gruppen", {}))
    return modus_code(fall.get("modi", {}))


# Schritt II (Systemnutzer wb-halter): die feste setgid-Liste der Rollenprogramme. Pfad im Baum -> Rollengruppe.
# Annahme bis Zug 3 (Halter 2.3 Fassung 2): nur die Rolle `bruecke` hat heute ein Rollenprogramm unter Linux (Halter 2.3,
# `werkbank-modellweg`); die Programme der Rollen `hub` und `oberflaeche` trägt erst Zug 3 nach.
SETGID_LISTE = {"usr/lib/werkbank/werkbank-modellweg": "wb-bruecke"}
ROLLENGRUPPEN = ("wb-hub", "wb-bruecke", "wb-oberflaeche")


def setgid_code(modi, gruppen):
    """Stufe 9 in Schritt II, zweiter Teil: setuid und sticky nie; setgid nur an Pfaden der festen Liste, dort genau 2755 mit der
    Rollengruppe der Liste; ein Pfad der Liste im Baum ohne setgid ist ebenso falsch. Gibt `modus_setgid` oder None zurück."""
    for pfad, m in modi.items():
        wert = int(m, 8)
        if wert & 0o5000:
            return "modus_setgid"
        if wert & 0o2000 and (pfad not in SETGID_LISTE or m != "2755" or gruppen.get(pfad) != SETGID_LISTE[pfad]):
            return "modus_setgid"
        if pfad in SETGID_LISTE and not wert & 0o2000:
            return "modus_setgid"
    return None


# ---------------------------------------------------------------------------------------------------- Stückliste

# Kein festes CA-Bündel im Produkt: Der System-Truststore gilt immer (Firma 20.11.).
VERBOTENE_BIBLIOTHEKEN = ("webpki-roots", "webpki-root-certs")
SL_NAME = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")
SL_VERSION = re.compile(r"^[0-9A-Za-z.+-]{1,64}$")
SL_LIZENZ = re.compile(r"^[A-Za-z0-9.+() -]{1,128}$")


def stueckliste_code(fall):
    """Fall der Form `stueckliste`: Ausgabe von `wb lizenzen` und die Bibliotheken im Paket."""
    roh = fall["text"].encode("utf-8")
    if (not roh or not roh.endswith(b"\n") or b"\r" in roh or any(b < 0x20 and b not in (0x09, 0x0A) for b in roh)
            or any(b > 0x7E for b in roh)):
        return "stueckliste_form"
    zeilen = roh.decode("ascii")[:-1].split("\n")
    felder = [z.split("\t") for z in zeilen]
    if any(len(f) != 4 or any(x == "" for x in f) for f in felder):
        return "stueckliste_leer_feld"
    if any(not (SL_NAME.match(n) and SL_VERSION.match(v) and SL_LIZENZ.match(li)) for n, v, li, _ in felder):
        return "stueckliste_feld_form"
    if any(not (h == "crates.io" or (h == "ubuntu-archiv" and n == "bubblewrap")) for n, _, _, h in felder):
        return "stueckliste_herkunft"
    if any(n in VERBOTENE_BIBLIOTHEKEN for n, _, _, _ in felder):
        return "stueckliste_verboten"
    schluessel = [(n, v) for n, v, _, _ in felder]
    if len(set(schluessel)) != len(schluessel):
        return "stueckliste_doppelt"
    if schluessel != sorted(schluessel):
        return "stueckliste_reihenfolge"
    if any((b["name"], b["version"]) not in set(schluessel) for b in fall["im_paket"]):
        return "stueckliste_fehlt"
    return None


# ---------------------------------------------------------------------------------------------------- Steuerdatei (deb)

STEUER_PFLICHT = ("Package", "Version", "Architecture", "Depends", "Description")
STEUER_ABHAENGIGKEITEN = ("python3", "bubblewrap", "apparmor", "ca-certificates", "systemd", "libssl3t64")
STEUER_FELD = re.compile(r"^[A-Za-z][A-Za-z0-9-]*$")


def steuer_lesen(text):
    """Liest die Steuerdatei (`control`) als Feld -> Wert; wirft ValueError bei Formverletzung."""
    if not text.endswith("\n") or any(ord(c) > 0x7E or (ord(c) < 0x20 and c != "\n") for c in text):
        raise ValueError("Zeichen oder Zeilenende")
    felder, aktuell = {}, None
    for z in text[:-1].split("\n"):
        if z.startswith((" ", "\t")):
            if aktuell is None:
                raise ValueError("Fortsetzung ohne Feld")
            felder[aktuell] += "\n" + z.strip()
            continue
        name, doppelpunkt, wert = z.partition(":")
        if not doppelpunkt or not STEUER_FELD.match(name) or name in felder:
            raise ValueError(z)
        felder[name] = wert.strip()
        aktuell = name
    return felder


def steuer_code(fall):
    """Fall der Form `steuerdatei`: Text der Steuerdatei, `manifest_version` und `geruest` (Gerüstbau, Vorgabe: nein)."""
    try:
        f = steuer_lesen(fall["text"])
    except ValueError:
        return "steuer_form"
    if any(not f.get(k) for k in STEUER_PFLICHT):
        return "steuer_feld_fehlt"
    if f["Package"] != "werkbank" or f["Architecture"] != "amd64" or f["Version"] != fall["manifest_version"] + ("~geruest" if fall.get("geruest") else ""):
        return "steuer_wert"
    namen = {d.split("(")[0].split("|")[0].strip() for d in f["Depends"].split(",")}
    if any(n not in namen for n in STEUER_ABHAENGIGKEITEN):
        return "steuer_abhaengigkeit"
    return None


# ---------------------------------------------------------------------------------------------------- AppArmor-Profil

PROFIL_KOPF = re.compile(r"^\s*profile\s+(\S+)(?:\s+(/\S+))?(?:\s+flags=\(([^)]*)\))?\s*\{\s*$")
BWRAP = "/usr/bin/bwrap"
BWRAP_KOPIE = "/usr/lib/werkbank/bwrap"


def glob_trifft(muster, pfad):
    """Grobes Gegenstück zu AppArmor-Anhängen: `*` bleibt im Segment, `**` geht über Segmente."""
    ausdruck = re.escape(muster).replace(r"\*\*", "\0").replace(r"\*", "[^/]*").replace("\0", ".*").replace(r"\{", "(?:").replace(r"\}", ")").replace(",", "|")
    return re.fullmatch(ausdruck, pfad) is not None


def profil_code(fall):
    """Fall der Form `profil`: Text einer Profildatei des Pakets. Gibt den Code ohne Präfix oder None zurück."""
    text = fall["text"]
    if (not text.strip() or not text.endswith("\n") or any(ord(c) > 0x7E or (ord(c) < 0x20 and c not in "\n\t") for c in text)
            or text.count("{") != text.count("}")):
        return "profil_form"
    profile, aktuell = [], None
    for zeile in text.split("\n"):
        z = zeile.split("#")[0].rstrip()
        k = PROFIL_KOPF.match(z)
        if k:
            name, anhang = k.group(1), k.group(2)
            if anhang is None and name.startswith("/"):
                anhang = name
            aktuell = {"name": name, "anhang": anhang, "userns": False,
                       "unconfined": "unconfined" in re.split(r"[\s,]+", k.group(3) or "")}
            profile.append(aktuell)
        elif re.match(r"^\s*(/\S*\s+)?\{\s*$", z):
            return "profil_form"
        elif aktuell is not None and re.match(r"^\s*(allow\s+)?userns\s*,", z):
            aktuell["userns"] = True
    if not profile:
        return "profil_form"
    if any(p["anhang"] and glob_trifft(p["anhang"], BWRAP) for p in profile):
        return "profil_system_bwrap"
    if any(p["userns"] and p["anhang"] not in (None, BWRAP_KOPIE) for p in profile):
        return "profil_userns_unbegrenzt"
    if any(p["unconfined"] for p in profile):
        return "profil_unconfined"
    return None


# ---------------------------------------------------------------------------------------------------- Statischer Bau

# Programme des deb für Ubuntu, die statisch gelinkt sein müssen (`-C target-feature=+crt-static`): Quelle `bin:<crate>:<programm>` und Ziel.
# Nur, was im Einschluss oder unter dem Profil des Starters läuft (Entscheidung vom 21.11.2026). `werkbank-lauf`, `werkbank-lauf-kind` und
# `werkbank-ordnermarke` laufen außerhalb des Einschlusses und ohne Profil: dynamisch, gegen das OpenSSL der Distribution (`libssl3t64` aus `dpkg-shlibdeps`).
STATISCH = {"usr/lib/werkbank/werkbank-einschluss": ("einschluss", "werkbank-einschluss"),
            "usr/lib/werkbank/einschluss-weiter": ("einschluss", "einschluss-weiter")}


def statisch_code(fall):
    """Fall der Form `statisch`: die gestuften Programme mit ihrem ELF-Befund (`interp`: hat einen Programminterpreter; `needed`: Liste der
    DT_NEEDED). Statisch heißt: kein Interpreter und keine Liste. Gibt den Code ohne Präfix oder None zurück."""
    for p in fall["programme"]:
        if p["ziel"] in STATISCH and (p["interp"] or p["needed"]):
            return "programm_nicht_statisch"
    return None


# ---------------------------------------------------------------------------------------------------- lauf.json (postinst)

LAUF_JSON = "etc/werkbank/lauf.json"
LAUF_DATEN = "/var/lib/werkbank"
LAUF_PFAD_ZEICHEN = re.compile(r"^[A-Za-z0-9._/+@-]+$")
LAUF_HARNESS = ("copilot", "pi")
LAUF_PATH_FEST = "/usr/local/bin:/usr/bin:/bin"
# Where the enclosure sees a folder without a bind: everything below /usr (the system part is bound whole).
SOCKET_ENDE = "/modell.sock"
SITZUNG_MAX = 64


def lauf_ordner_form(p):
    """The form `ordner_form` of crate lauf: absolute, no trailing slash, no `//`, no `.` or `..` segment, at most 1024 bytes."""
    return (isinstance(p, str) and 1 < len(p) <= 1024 and p.startswith("/") and not p.endswith("/") and "//" not in p
            and all(t not in (".", "..") for t in p.split("/")[1:]) and LAUF_PFAD_ZEICHEN.match(p) is not None)


def lauf_paketwurzel(programm):
    """The program folder of one harness: the root of its npm package, `<prefix>/lib/node_modules/<package>` (two levels for `@scope/package`), never the
    prefix (a prefix such as `<home>/.local` holds the login store `.local/share/keyrings`); for a program that is no file of such a package, the folder it lies in."""
    ordner = programm.rsplit("/", 1)[0]
    if "/lib/node_modules/" not in programm:
        return ordner
    prefix, rest = programm.split("/lib/node_modules/", 1)
    teile = rest.split("/")
    n = 2 if teile[0].startswith("@") else 1
    if len(teile) <= n:
        return ordner
    return prefix + "/lib/node_modules/" + "/".join(teile[:n])


def lauf_programmordner(harness):
    """The folders the harnesses need: the package root of each harness (`lauf_paketwurzel`) and the folder of its node (the folder the file lies in);
    nothing at or below /usr; doubles and folders inside another folder of the list collapse; order by first use."""
    kandidaten = []
    for name in LAUF_HARNESS:
        h = harness.get(name)
        if not h:
            continue
        kandidaten.append(lauf_paketwurzel(h["programm"]))
        if h.get("node"):
            kandidaten.append(h["node"].rsplit("/", 1)[0])
    aus = []
    for k in kandidaten:
        if k == "/usr" or k.startswith("/usr/") or k in aus:
            continue
        aus.append(k)
    return [k for k in aus if not any(o != k and k.startswith(o + "/") for o in aus)]


def lauf_harness_ordner(h):
    """The folders one harness needs (package root, folder of its node), nothing at or below /usr: what postinst asks the check for, in this order."""
    aus = [lauf_paketwurzel(h["programm"])]
    if h.get("node"):
        aus.append(h["node"].rsplit("/", 1)[0])
    return [k for k in aus if not (k == "/usr" or k.startswith("/usr/"))]


def lauf_abgelehnt(fall):
    """The harnesses that postinst does not enter: `(harness, folder)` in the order copilot, pi; the folder is the first one of the harness that the check
    refuses (`pruefer_verboten` of the case). The pair is one line of the state file `/var/lib/werkbank/programmordner-abgelehnt` (`<harness> <folder>`)."""
    aus = []
    for name in LAUF_HARNESS:
        h = fall.get("harness", {}).get(name)
        if not h:
            continue
        for d in lauf_harness_ordner(h):
            if d in fall.get("pruefer_verboten", []):
                aus.append((name, d))
                break
    return aus


def lauf_path(harness):
    """PATH of the harness: the bin folder of a node outside /usr first, then the fixed system folders."""
    for name in LAUF_HARNESS:
        h = harness.get(name)
        if h and h.get("node"):
            ordner = h["node"].rsplit("/", 1)[0]
            if not (ordner == "/usr" or ordner.startswith("/usr/")):
                return ordner + ":" + LAUF_PATH_FEST
    return LAUF_PATH_FEST


def laufkonfig(fall):
    """Fall der Form `laufkonfig`: the content of /etc/werkbank/lauf.json that postinst writes, or None when it writes no file (no user, or a
    value that is no folder form). `vorher` is the value of `ca_zulassung` of an earlier file or None; only `zugelassen` and `abgelehnt` stay. `antwort` (optional) is the answer of debconf that waits."""
    nutzer, uid, heim = fall.get("nutzer"), fall.get("uid"), fall.get("heim")
    if not nutzer or not isinstance(uid, int) or not lauf_ordner_form(heim):
        return None
    harness = fall.get("harness", {})
    if any(not lauf_ordner_form(h["programm"]) or (h.get("node") and not lauf_ordner_form(h["node"])) for h in harness.values()):
        return None
    # `pruefer_verboten` (optional): the program folders that the rule of the enclosure refuses (answer of `werkbank-pruefen --programmordner`; the Rust test
    # `tests/programmordner_faelle.rs` holds the real rule to this list). A harness with such a folder is not entered (`lauf_abgelehnt`): neither its
    # `harness.<name>` nor its folders reach the file, and nothing stops postinst.
    verweigert = {n for n, _ in lauf_abgelehnt(fall)}
    harness = {n: h for n, h in harness.items() if n not in verweigert}
    o = f"{LAUF_DATEN}/{nutzer}"
    ca = fall.get("vorher") if fall.get("vorher") in ("zugelassen", "abgelehnt") else "offen"
    # `vorher_form` (optional): the earlier file was rewritten by another tool. White space around ":" and line breaks do not matter; a doubled key or a value
    # that is none of the three (also with a blank inside) is invalid and gives `offen`.
    if fall.get("vorher_form") in ("doppelt", "als-wert", "leerzeichen-im-wert"):
        ca = "offen"
    # An answer of debconf that waits (flag `seen` true; the question of `dpkg-reconfigure` or `debconf-set-selections`) is the explicit step of an administrator and counts
    # before the file: one of the three values. Any other answer is ignored.
    if fall.get("antwort") in ("zugelassen", "offen", "abgelehnt"):
        ca = fall["antwort"]
    return {"fassung": 1, "einstellungen": f"{o}/einstellungen.json", "vault": f"{o}/vorfuehr-vault", "protokoll": f"{o}/protokoll", "zustand": o,
            "laufzeit": f"{LAUF_DATEN}/l/{uid}", "sitzungen": f"{LAUF_DATEN}/s/{uid}", "heim": heim, "path": lauf_path(harness),
            "programme": {"rangkern": "/usr/lib/werkbank/rangkern", "modellweg": "/usr/lib/werkbank/werkbank-modellweg", "kind": "/usr/lib/werkbank/werkbank-lauf-kind",
                          "such_mcp": "/usr/lib/werkbank/such-mcp", "weiter": "/usr/lib/werkbank/einschluss-weiter"},
            "harness": {n: harness[n]["programm"] for n in LAUF_HARNESS if n in harness},
            "programmordner": lauf_programmordner(harness),
            "schluessel": {"copilot": f"{o}/copilot.schluessel", "lokal": f"{o}/modellweg.schluessel"},
            "ca_zulassung": ca}


KERN_JSON, BRUECKE_JSON = "etc/werkbank/kern.json", "etc/werkbank/bruecke.json"
SOCKET_PFAD_MAX = 100  # `dienst`: der Socket-Pfad, und `bruecke`: `dienst.socket` (absoluter Pfad, höchstens 100 Byte)


def _dk_zahl(text, lo, hi):
    """A decimal number of at most 5 digits without a leading zero in the range (the rule of `dk_zahl` in postinst)."""
    return bool(re.fullmatch(r"[0-9]{1,5}", text)) and (text == "0" or text[0] != "0") and lo <= int(text) <= hi


def dienst_admin(fall):
    """The admin fields of kern.json (`lauf.groesse`) and bruecke.json (`port`) that an update keeps (contract "Welche Felder der Admin setzt").
    `vorher_kern` and `vorher_bruecke` (optional) are the texts of the old files. The old text is read as JSON text: tab, CR and LF count as blanks; the key must
    stand exactly once and the value must have the valid form (columns 20 to 1000, rows 5 to 1000; port 0 to 65535), else the default and one fixed journal line.
    Returns (groesse, port, journal lines)."""
    groesse, port, journal = [120, 40], 0, []
    kern = fall.get("vorher_kern")
    if kern is not None and '"groesse"' in kern:
        t = re.sub(r"[\n\r\t]", " ", kern)
        m = re.findall(r'"groesse" *: *\[ *([0-9]+) *, *([0-9]+) *\]', t)
        if t.count('"groesse"') == 1 and len(m) == 1 and _dk_zahl(m[0][0], 20, 1000) and _dk_zahl(m[0][1], 5, 1000):
            groesse = [int(m[0][0]), int(m[0][1])]
        else:
            journal.append("werkbank: kern.json: groesse ungueltig, die Vorgabe gilt")
    bruecke = fall.get("vorher_bruecke")
    if bruecke is not None and '"port"' in bruecke:
        t = re.sub(r"[\n\r\t]", " ", bruecke)
        m = re.findall(r'"port" *: *([0-9]+) *[,}]', t)
        if t.count('"port"') == 1 and len(m) == 1 and _dk_zahl(m[0], 0, 65535):
            port = int(m[0])
        else:
            journal.append("werkbank: bruecke.json: port ungueltig, die Vorgabe gilt")
    return groesse, port, journal


def dienstkonfig(fall):
    """Fall der Form `dienstkonfig`: the content of /etc/werkbank/kern.json and bruecke.json that postinst writes for the user `nutzer` (uid `uid`),
    or None when it writes no file (no user, no number). `<run>` is /run/user/<uid>. The environment of the sessions is exactly TERM (contract 1,
    "Aufruf Lauf starten"). `lauf.sitzungen` of kern.json is the same value as `sitzungen` of lauf.json (`laufkonfig`): the core service hands out no
    session id whose folder still exists there (a missing field is an error of the package). `lauf.marker_datei` is `<run>/werkbank-probelauf/marker.json` (the file of the
    trial run, only for it) and `lauf.marker_ordner` is `/var/lib/werkbank/z/<uid>` (the count folder, created by postinst); both fields belong to the package. `lauf.ziellisten` is the folder of the target lists of the sessions, `<zustand>/sitzungen` of lauf.json (`/var/lib/werkbank/<nutzer>/sitzungen`): the core service skips every session number whose `<ziellisten>/s-<n>.zielliste.json` exists. `lauf.laufzeit` is `laufzeit` of lauf.json (`/var/lib/werkbank/l/<uid>`). Both files carry `deflate` true: the bridge sends the value of the service as X-Deflate (contract bruecke, BR1)."""
    nutzer, uid = fall.get("nutzer"), fall.get("uid")
    if not nutzer or not isinstance(uid, int) or uid < 0:
        return None
    run = f"/run/user/{uid}"
    groesse, port, _ = dienst_admin(fall)
    return {
        "kern": {"socket": {"ordner": f"{run}/werkbank", "name": "dienst.sock"},
                 "betrachter": {"person": nutzer, "raum": "werkbank", "ansicht": "browser"}, "deflate": True,
                 "lauf": {"programm": "/usr/lib/werkbank/werkbank-lauf", "env": {"TERM": "xterm-256color"}, "groesse": groesse,
                          "einstellungen": f"{LAUF_DATEN}/{nutzer}/einstellungen.json", "sitzungen": f"{LAUF_DATEN}/s/{uid}",
                          "marker_datei": f"{run}/werkbank-probelauf/marker.json", "marker_ordner": f"{LAUF_DATEN}/z/{uid}",
                          "ziellisten": f"{LAUF_DATEN}/{nutzer}/sitzungen", "laufzeit": f"{LAUF_DATEN}/l/{uid}"},
                 "anfang": {"personen": {nutzer: {"art": "mensch", "rolle": "nutzer"}}, "sitzungen": []}},
        "bruecke": {"dienst": {"socket": f"{run}/werkbank/dienst.sock"}, "port": port, "token_datei": f"{run}/werkbank/bruecke.token",
                    "port_datei": f"{run}/werkbank/bruecke.port", "deflate": True},
    }


# Keys of a unit that make systemd set NoNewPrivileges for a user manager (seccomp-based options of systemd.exec); a core unit with one of them is refused as well.
KERN_EINHEIT_NNP_SCHLUESSEL = ("NoNewPrivileges", "SystemCallFilter", "SystemCallArchitectures", "RestrictAddressFamilies", "RestrictNamespaces",
                               "RestrictRealtime", "RestrictSUIDSGID", "MemoryDenyWriteExecute", "LockPersonality")
KERN_EINHEIT = "usr/lib/systemd/user/werkbank-kern.service"
SYSTEMD_WAHR = ("1", "yes", "true", "on")


def kern_einheit_code(fall):
    """Fall der Form `kerneinheit`: the text of the unit file of the core service. The core service starts the sessions, and the profile change of the
    fixed starter to bwrap is refused under NoNewPrivileges (AppArmor: "no new privs"); the enclosure sets the flag itself for the harness. So the unit
    carries no NoNewPrivileges with a true value (the last assignment counts) and none of the keys that imply it (`KERN_EINHEIT_NNP_SCHLUESSEL`;
    an empty value resets a key). Gives the code without prefix or None."""
    aktiv = {}
    for zeile in fall["text"].split("\n"):
        zeile = zeile.strip()
        if not zeile or zeile[0] in "#;" or "=" not in zeile:
            continue
        schluessel, _, wert = zeile.partition("=")
        schluessel, wert = schluessel.strip(), wert.strip().strip("\"'").lower()
        if schluessel == "NoNewPrivileges":
            aktiv[schluessel] = wert in SYSTEMD_WAHR
        elif schluessel in KERN_EINHEIT_NNP_SCHLUESSEL:
            aktiv[schluessel] = wert != ""
    return "kern_einheit_nonewprivs" if any(aktiv.values()) else None


def dienstkonfig_socket_laenge(uid):
    return len(f"/run/user/{uid}/werkbank/dienst.sock")


def laufzeit_socket_laenge(uid):
    """The longest socket path under `laufzeit` of a user: folder, a session name of 64 bytes, `/modell.sock`."""
    return len(f"{LAUF_DATEN}/l/{uid}") + 1 + SITZUNG_MAX + len(SOCKET_ENDE)


# ---------------------------------------------------------------------------------------------------- Entfernen (purge)

PURGE_SYSTEMDATEIEN = ("datei:/var/log/dpkg.log", "datei:/var/log/apt/", "datei:/var/log/journal/", "datei:/var/cache/apt/",
                       "datei:/var/lib/apt/")
EINTRAG = re.compile(r"^(datei|ordner):/[A-Za-z0-9._/@+-]{1,300}$|^profil:(/[A-Za-z0-9._/-]{1,200}|[a-z0-9][a-z0-9._-]{0,63})$|^(konto|gruppe|einheit|prozess):[a-z0-9][a-z0-9._-]{0,63}$")


def purge_freigestellt(eintrag, fall):
    """Ein Rückstand ist nur freigestellt, wenn er der Fall ausdrücklich nennt und die Regel ihn kennt."""
    art, _, wert = eintrag.partition(":")
    if art not in ("datei", "ordner"):
        return False
    if wert == fall["ergebnisdatei"]:
        return True
    heim = f"{fall['heimwurzel']}/{fall['nutzer']}/.copilot"
    if wert == heim or wert.startswith(heim + "/"):
        return True
    return any(eintrag == p or (p.endswith("/") and (eintrag.startswith(p) or eintrag == p[:-1])) for p in PURGE_SYSTEMDATEIEN)


def werkbank_profil(eintrag):
    """Ein geladenes Profil des Produkts (Label `werkbank-*` oder Pfad unter `/usr/lib/werkbank/`) darf nach `purge` nie mehr da sein."""
    art, _, wert = eintrag.partition(":")
    return art == "profil" and (wert.startswith("werkbank-") or wert.startswith("/usr/lib/werkbank/"))


def entfernen_code(fall, suche="einhaengepunkte"):
    """Fall der Form `entfernen`: Zustand `vorher` (vor der Installation) und `nachher` (nach `apt purge`).

    `suche` wählt die Regel der Suche nach Resten in Schritt II. `einhaengepunkte` ist die geltende (Teil A2, `purge` Schritt 3: jeder lokale
    Einhängepunkt, fail closed). `wurzel` ist die ersetzte Regel (`find / -xdev`) und nur Gegenprobe: sie kennt nur das Wurzel-Dateisystem."""
    vorher, nachher = set(fall["vorher"]), set(fall["nachher"])
    zwei = fall.get("schritt") == 2
    rest = fall.get("rest", {"dateien": [], "prozesse": []}) if zwei else {"dateien": [], "prozesse": []}
    punkte = fall.get("einhaengepunkte")
    if punkte is not None and not zwei:
        return "entfernen_form"
    if zwei and punkte is not None:
        if rest["dateien"] or not einhaengepunkte_form(punkte, fall.get("findmnt_fehler", False)):
            return "entfernen_form"
        if suche == "wurzel":
            vorher, nachher = ({e for e in menge if not unter_fremdem_punkt(e, punkte)} for menge in (vorher, nachher))
        gefunden, ungesucht = mount_rest(punkte, fall.get("findmnt_fehler", False), suche)
        rest = {"dateien": gefunden, "prozesse": rest["prozesse"]}
    else:
        ungesucht = False
    if any(not EINTRAG.match(e) for e in vorher | nachher | set(rest_eintraege(rest))):
        return "entfernen_form"
    frei = set()
    hat_rest = bool(rest["dateien"] or rest["prozesse"] or ungesucht)
    if zwei and hat_rest:
        # Weg a mit Rest: Konto und Gruppen bleiben, ebenso genau der gemeldete Rest außerhalb der Ablage.
        frei = set(SYSTEMNUTZER_EINTRAEGE) | {e for e in rest_eintraege(rest)
                                              if not e.startswith(("datei:" + HALTER_SYSTEM_ABLAGE + "/", "prozess:werkbank-"))}
    if any(e not in vorher and e not in frei and not purge_freigestellt(e, fall) for e in nachher):
        return "entfernen_rueckstand"
    if any(werkbank_profil(e) for e in nachher):
        return "entfernen_rueckstand"
    if any(e not in nachher for e in vorher):
        return "entfernen_verloren"
    if frei and any(e not in nachher for e in SYSTEMNUTZER_EINTRAEGE):
        return "entfernen_konto_trotz_rest"
    if frei and KONTO_RESTZEILE not in fall.get("journal", ""):
        return "entfernen_rest_ohne_meldung"
    return None


# Schritt II: Systemnutzer und Gruppen, die preinst anlegt (Entscheid des Betreibers, 23.11.2026), und der Ort ihrer Ablage.
SYSTEMNUTZER = "wb-halter"
SYSTEMNUTZER_EINTRAEGE = ("konto:wb-halter", "gruppe:wb-halter") + tuple("gruppe:" + g for g in ROLLENGRUPPEN)
HALTER_SYSTEM_ABLAGE = "/var/lib/wb-halter"
KONTO_RESTZEILE = "werkbank: Konto wb-halter und Rollengruppen bleiben, Code 23"


def rest_eintraege(rest):
    """Der Rest beim purge (Weg a): Dateien, die `find -uid`/`-gid` fand, und Prozesse des Nutzers, als Einträge der Form `entfernen`."""
    return ["datei:" + p for p in rest["dateien"]] + ["prozess:" + p for p in rest["prozesse"]]


# Suche nach Resten in Schritt II (Teil A2, `purge` Schritt 3): die Liste der Einhängepunkte kommt aus `findmnt`; je Punkt läuft `find -xdev`.
# Typen, die nie durchsucht werden: Kernel-Schnittstellen, Arbeitsspeicher, Netz, Abbilder. Jeder Typ steht in der README mit Grund.
MOUNT_AUSGENOMMEN = frozenset({
    "proc", "sysfs", "devtmpfs", "devpts", "tmpfs", "ramfs", "cgroup", "cgroup2", "securityfs", "debugfs", "tracefs", "configfs", "fusectl",
    "pstore", "bpf", "mqueue", "hugetlbfs", "binfmt_misc", "efivarfs", "autofs", "rpc_pipefs", "nsfs", "overlay",
    "fuse.gvfsd-fuse", "fuse.portal", "squashfs", "erofs", "iso9660", "udf",
    "nfs", "nfs4", "cifs", "smb3", "9p", "fuse.sshfs",
})
# Typen lokaler Datenträger, die durchsucht werden. Jeder andere Typ gilt als unbekannt und zählt als Rest (fail closed).
MOUNT_DURCHSUCHT = frozenset({"ext2", "ext3", "ext4", "xfs", "btrfs", "f2fs", "jfs", "zfs", "vfat", "exfat", "ntfs3", "fuseblk"})
MOUNT_PFAD = re.compile(r"^/([A-Za-z0-9._@+-]+(/[A-Za-z0-9._@+-]+)*)?$")
MOUNT_TYP = re.compile(r"^[a-z0-9][a-z0-9._-]{0,31}$")


def einhaengepunkte_form(punkte, findmnt_fehler):
    """Die Liste der Einhängepunkte hat Form: Pfade absolut und einmalig, Typ ein Dateisystemname, `lesbar` ein Wahrheitswert, `dateien` absolute Pfade
    unter dem Punkt und nur auf einem durchsuchten, lesbaren Punkt (andere kann `purge` nicht finden und der Fall darf sie nicht behaupten).
    Die Wurzel steht immer da, außer `findmnt` ist gescheitert (dann ist die Liste leer)."""
    if not isinstance(punkte, list) or not isinstance(findmnt_fehler, bool):
        return False
    pfade = [p.get("pfad") for p in punkte if isinstance(p, dict)]
    if len(pfade) != len(punkte) or len(set(pfade)) != len(pfade) or bool(punkte) == findmnt_fehler or (punkte and "/" not in pfade):
        return False
    for p in punkte:
        if set(p) != {"pfad", "typ", "lesbar", "dateien"}:
            return False
        if not isinstance(p["pfad"], str) or not MOUNT_PFAD.match(p["pfad"]) or not isinstance(p["typ"], str) or not MOUNT_TYP.match(p["typ"]):
            return False
        if not isinstance(p["lesbar"], bool) or not isinstance(p["dateien"], list):
            return False
        for d in p["dateien"]:
            if not isinstance(d, str) or not MOUNT_PFAD.match(d) or d == p["pfad"] or not (p["pfad"] == "/" or d.startswith(p["pfad"] + "/")):
                return False
        if p["dateien"] and (p["typ"] not in MOUNT_DURCHSUCHT or not p["lesbar"]):
            return False
    return True


def mount_rest(punkte, findmnt_fehler, suche="einhaengepunkte"):
    """Was die Suche nach Resten ergibt: (gefundene Dateien, ob etwas nicht durchsuchbar war). Nicht durchsuchbar (fail closed) sind: ein Fehler von
    `findmnt`, ein Punkt mit unbekanntem Typ, ein nicht lesbarer Punkt. Ausgenommene Typen zählen nie. Regel `wurzel`: nur der Punkt `/`, nie ein Fehler."""
    if suche == "wurzel":
        return [d for p in punkte if p["pfad"] == "/" for d in p["dateien"]], False
    gefunden, ungesucht = [], bool(findmnt_fehler)
    for p in punkte:
        if p["typ"] in MOUNT_AUSGENOMMEN:
            continue
        if p["typ"] not in MOUNT_DURCHSUCHT or not p["lesbar"]:
            ungesucht = True
            continue
        gefunden += p["dateien"]
    return gefunden, ungesucht


def unter_fremdem_punkt(eintrag, punkte):
    """Ein Eintrag `datei:`/`ordner:` liegt unter einem Einhängepunkt außer der Wurzel."""
    art, _, wert = eintrag.partition(":")
    if art not in ("datei", "ordner"):
        return False
    return any(p["pfad"] != "/" and (wert == p["pfad"] or wert.startswith(p["pfad"] + "/")) for p in punkte)


# ---------------------------------------------------------------------------------------------------- Entfernen mit `remove`

RESTZEILE = "pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie."
COPILOT_RESTZEILE = "pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie."


ABLAGE_REL = ".local/state/agent-workbench/halter/geheim"  # the store of the holder below the home (halter README 2.3, Nachtrag Halter)


def remove_code(fall):
    """Fall der Form `remove`: Zustände nach der Installation und nach `apt remove`, dazu die Ausgabe des Entfernens.
    With `heimwurzel` (optional, Nachtrag Halter) the same two rules hold for `anmeldung.copilot` in the store of the holder."""
    paare = [(f"datei:/var/lib/werkbank/{fall['nutzer']}/modellweg.schluessel", RESTZEILE)]
    zwei = fall.get("schritt") == 2
    if zwei:
        # Schritt II: die Ablage liegt unter /var/lib/wb-halter; jede Datei dort ist Nutzerdaten und bleibt.
        paare.append((f"datei:{HALTER_SYSTEM_ABLAGE}/geheim/anmeldung.copilot", COPILOT_RESTZEILE))
        paare += [(e, None) for e in fall["nach_installation"]
                  if e.startswith((f"datei:{HALTER_SYSTEM_ABLAGE}/", f"ordner:{HALTER_SYSTEM_ABLAGE}/")) or e == f"ordner:{HALTER_SYSTEM_ABLAGE}"]
    elif "heimwurzel" in fall:
        paare.append((f"datei:{fall['heimwurzel']}/{fall['nutzer']}/{ABLAGE_REL}/anmeldung.copilot", COPILOT_RESTZEILE))
    for eintrag, _ in paare:
        if eintrag in fall["nach_installation"] and eintrag not in fall["nach_remove"]:
            return "remove_schluessel_verloren"
    for eintrag, zeile in paare:
        if zeile is not None and eintrag in fall["nach_remove"] and zeile not in fall["meldung"]:
            return "remove_hinweis_fehlt"
    if fall["marke"] in fall["meldung"] or fall["marke"] in fall["journal"]:
        return "remove_hinweis_inhalt"
    if zwei and any(e in fall["nach_installation"] and e not in fall["nach_remove"] for e in SYSTEMNUTZER_EINTRAEGE):
        return "remove_konto_verloren"
    return None


# ---------------------------------------------------------------------------------------------------- Einrichten (Schlüssel)

def einricht_code(fall):
    """Fall der Form `einrichten`: Beobachtungen beim ersten Aufruf von `werkbank-probelauf` mit der Marke als Schlüssel."""
    marke = fall["marke"]
    stellen = [fall["stdout"], fall["stderr"], fall["journal"], fall["argv"], fall["umgebung"], fall["ergebnisdatei"]] + list(fall["protokolle"])
    if any(marke in st for st in stellen):
        return "schluessel_in_ausgabe"
    if fall["echo"]:
        return "echo_an"
    d = fall["datei"]
    if d["modus"] != "0600":
        return "datei_modus"
    if d["eigentuemer"] != fall["nutzer"]:
        return "datei_eigentuemer"
    if d["inhalt"] != marke:
        return "datei_inhalt"
    return None


# ---------------------------------------------------------------------------------------------------- Prüfbefehl

MAX_DATEIZEILEN = 20
ART_REIHE = {"fehlt": 0, "abweichend": 1, "fremd": 2}
STELLE_SATZ = "Installation fertig, Stelle einrichten. Befehl: wb stelle einrichten"


def _windows_dienste(lage, befunde):
    h = lage["halter"]
    if h != "ok":
        befunde.append({
            "fehlt": "pruef.halter_fehlt: Der Dienst wb-halter ist nicht eingetragen.",
            "konto": "pruef.halter_konto: Der Dienst wb-halter laeuft nicht unter dem Konto NT SERVICE\\wb-halter.",
            "gestoppt": "pruef.halter_gestoppt: Der Dienst wb-halter ist nicht gestartet.",
            "nicht_pruefbar": "pruef.halter_nicht_pruefbar: Der Dienst wb-halter war nicht pruefbar.",
        }[h])

    w = lage["wfp"]
    if w != "ok":
        befunde.append({
            "dienst_fehlt": "pruef.wfp_dienst_fehlt: Der Filterdienst wb-plattform laeuft nicht.",
            "filter_fehlt": "pruef.wfp_filter_fehlt: Die Filtergruppe der Werkbank fehlt in der Filterplattform.",
            "sperre_fehlt": "pruef.wfp_sperre_fehlt: Fuer einen Agentencontainer fehlt die Sperre fuer 127.0.0.0/8 und ::1.",
            "nicht_pruefbar": "pruef.wfp_nicht_pruefbar: Die Filter der Werkbank waren nicht pruefbar.",
        }[w])


ANZAHL = {"windows": 5, "linux": 8, "macos": 6}
LAUNCHD_SATZ = {
    "fehlt": "pruef.launchd_fehlt: Der Dienst werkbank.halter ist nicht eingetragen.",
    "gestoppt": "pruef.launchd_gestoppt: Der Dienst werkbank.halter laeuft nicht.",
    "konto": "pruef.launchd_konto: Der Dienst werkbank.halter laeuft nicht unter seinem eigenen Konto.",
    "nicht_pruefbar": "pruef.launchd_nicht_pruefbar: Der Dienst werkbank.halter war nicht pruefbar.",
}
SCHLUESSELBUND_SATZ = {
    "fehlt": "pruef.schluesselbund_fehlt: Der Schluesselbund-Eintrag des Modellwegs fehlt.",
    "nicht_pruefbar": "pruef.schluesselbund_nicht_pruefbar: Der Schluesselbund-Eintrag war nicht pruefbar.",
}
EINSCHLUSS_SATZ = {
    "fehlt": "pruef.einschluss_fehlt: Der Einschluss ist nicht verfuegbar.",
    "probelauf": "pruef.einschluss_probelauf: Der Probelauf des Einschlusses scheitert.",
    "nicht_pruefbar": "pruef.einschluss_nicht_pruefbar: Der Einschluss war nicht pruefbar.",
}
DIENSTE_LINUX = ("werkbank-kern.service", "werkbank-bruecke.service", "wb-halter.service")  # the holder since the Nachtrag Halter, last
DIENST_SATZ = {
    "fehlt": "pruef.dienst_fehlt: Der Dienst {n} ist nicht eingetragen.",
    "inaktiv": "pruef.dienst_inaktiv: Der Dienst {n} ist nicht aktiv.",
    "fehlgeschlagen": "pruef.dienst_fehlgeschlagen: Der Dienst {n} ist fehlgeschlagen.",
        "nicht_pruefbar": "pruef.dienst_nicht_pruefbar: Der Dienst {n} war nicht pruefbar.",
}
APPARMOR_SATZ = {
    "bwrap_fehlt": "pruef.apparmor_bwrap_fehlt: Das Programm /usr/bin/bwrap fehlt.",
    "datei_fehlt": "pruef.apparmor_datei_fehlt: Die Datei des AppArmor-Profils fehlt.",
    "nicht_geladen": "pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen.",
    "abweichend": "pruef.apparmor_abweichend: Das AppArmor-Profil weicht vom Paket ab.",
    "fremd": "pruef.apparmor_fremd: Ein anderes AppArmor-Profil gilt fuer /usr/bin/bwrap.",
    "probelauf": "pruef.apparmor_probelauf: Der Probelauf des Einschlusses scheitert.",
    "unconfined": "pruef.apparmor_unconfined: Ein Profil der Werkbank ist ohne Einschraenkung geladen.",
    "system_bwrap_frei": "pruef.apparmor_system_bwrap_frei: Das System-bwrap ist fuer jeden Nutzer freigegeben.",
    "kopie_abweichend": "pruef.apparmor_kopie_abweichend: Die bwrap-Kopie des Produkts weicht vom System-bwrap ab.",
    "nicht_pruefbar": "pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.",
}
ORDNER_SATZ = {
    "fehlt": "pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt.",
    "rechte": "pruef.nutzerordner_rechte: Der Ordner des Nutzers hat falsche Rechte oder einen falschen Eigentuemer.",
    "nicht_pruefbar": "pruef.nutzerordner_nicht_pruefbar: Der Ordner des Nutzers war nicht pruefbar.",
}
HINWEIS_SYSTEM_BWRAP_UBUNTU = ("pruef.hinweis_system_bwrap_ubuntu: Das System-bwrap ist durch das Ubuntu-Profil frei; "
                              "die Firma kann dieses Profil entfernen.")
HINWEIS_UBUNTU_PROFIL = {
    "geladen": "pruef.hinweis_ubuntu_profil_geladen: Das Ubuntu-Profil bwrap-userns-restrict ist geladen.",
    "entladen": "pruef.hinweis_ubuntu_profil_entladen: Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen.",
}
# The state of the approval of the local CA (`ca_zulassung` of /etc/werkbank/lauf.json), a note and no finding: `offen` and `abgelehnt` close the cloud route and are
# no error of the installation (rc stays 0). The field `ca_zulassung` of the `lage` is optional: absent, there is no line (the package wrote no file for it).
# `unbekannt`: the file is there but unreadable or without one of the three values.
HINWEIS_CA = {
    "zugelassen": "pruef.hinweis_ca_zugelassen: Die Zulassung der lokalen CA ist erteilt; die Cloud-Route ist nicht gesperrt.",
    "offen": "pruef.hinweis_ca_offen: Die Zulassung der lokalen CA ist offen; die Cloud-Route ist gesperrt. Das ist kein Fehler der Installation.",
    "abgelehnt": "pruef.hinweis_ca_abgelehnt: Die Zulassung der lokalen CA ist abgelehnt; die Cloud-Route ist gesperrt.",
    "unbekannt": "pruef.hinweis_ca_unbekannt: Die Zulassung der lokalen CA war nicht lesbar; die Cloud-Route ist gesperrt.",
    # `doppelt`: the key `ca_zulassung` stands more than once in lauf.json; the package reads that as `offen` (like `postinst`, `config` and the command).
    "doppelt": "pruef.hinweis_ca_doppelt: lauf.json nennt ca_zulassung mehr als einmal; das gilt als offen, die Cloud-Route ist gesperrt.",
}
# The program folders of lauf.json: a folder that the enclosure refuses (secret segment, login store) is a finding. The field `programmordner` of the `lage` is optional (absent: `ok`).
# A harness that postinst did not enter (`programmordner_abgelehnt` of the `lage`: a list of {harness, ordner}, from the state file of the package): one finding each, in the
# order copilot, pi; the folder stands last. The harness has no note "not entered" besides.
ABGELEHNT_SATZ = ("pruef.programmordner_verboten: Harness {n} nicht eingetragen. "
                  "Abhilfe: Harness unter einem anderen Praefix installieren, dann dpkg-reconfigure werkbank. Ordner: {o}")
PROGRAMMORDNER_SATZ = ("pruef.programmordner_verboten: Ein Programmordner in lauf.json beruehrt den Anmeldespeicher oder ein Geheimnis; "
                       "der Einschluss lehnt jede Sitzung ab (5 verboten).")
# A harness that lauf.json does not list: a note and no finding (a machine may have only one). `{n}` is `copilot` or `pi`; `~` stands for the home of the user.
HARNESS_HINWEIS = {
    "nicht_gefunden": "pruef.hinweis_harness_nicht_gefunden: {n} nicht gefunden in ~/.nvm/versions/node/v*/bin, ~/.local/bin, ~/.npm-global/bin, ~/.volta/bin, /usr/local/bin, /usr/bin; andere Orte werden nicht gesucht.",
    "ausserhalb": "pruef.hinweis_harness_ausserhalb: {n} liegt an einem gesuchten Ort, der echte Pfad aber nicht unter /usr, /opt, ~/.nvm, ~/.local, ~/.npm-global oder ~/.volta; er wird nicht genommen.",
    "nicht_eingetragen": "pruef.hinweis_harness_nicht_eingetragen: {n} liegt an einem gesuchten Ort, steht aber nicht in lauf.json; sudo dpkg-reconfigure werkbank.",
}
COPILOT_SATZ = {
    "ablage": "pruef.copilot_ablage: Die Ablage des Halters hat falsche Rechte, einen falschen Eigentuemer oder einen Verweis.",
    "fehlt": "pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt.",
    "art": "pruef.copilot_art: Die Anmeldedatei der Copilot CLI ist keine gewoehnliche Datei.",
    "eigentuemer": "pruef.copilot_eigentuemer: Die Anmeldedatei der Copilot CLI gehoert nicht dem Nutzer.",
    "modus": "pruef.copilot_modus: Die Anmeldedatei der Copilot CLI hat nicht den Modus 0600.",
    "leer": "pruef.copilot_leer: Die Anmeldedatei der Copilot CLI ist leer.",
    "form": "pruef.copilot_form: Die Anmeldedatei der Copilot CLI hat falsche Form.",
    "nicht_pruefbar": "pruef.copilot_nicht_pruefbar: Die Anmeldedatei der Copilot CLI war nicht pruefbar.",
}
# Nachtrag Halter: the file of the former target /var/lib/werkbank/<nutzer>/copilot.schluessel is still there (field `copilot_alt` of the `lage`).
COPILOT_ALTER_ORT = "pruef.copilot_alter_ort: Am frueheren Ort liegt noch eine Anmeldedatei der Copilot CLI; sie wird nicht mehr gelesen und muss weg."
# Nachtrag Rolle aus der Gruppe (25.11.2026): die Rollengruppe wb-bruecke und das setgid-Rollenprogramm, Teil der Prüfung `dienste`. Beide
# Lagefelder sind optional (fehlt eines, gilt `ok`); ihre Zeilen stehen nach den Zeilen der Einheiten, die Gruppe zuerst.
ROLLENGRUPPE_SATZ = {
    "fehlt": "pruef.rollengruppe_fehlt: Die Rollengruppe wb-bruecke fehlt; der Modellweg bekommt beim Halter keine Rolle.",
    "mitglied": "pruef.rollengruppe_mitglied: Die Rollengruppe wb-bruecke hat ein Mitglied, ein Passwort oder eine falsche Form; "
                "ein Prozess eines Kontos koennte die Rolle bruecke annehmen.",
    "nicht_pruefbar": "pruef.rollengruppe_nicht_pruefbar: Die Rollengruppe wb-bruecke war nicht pruefbar.",
}
ROLLENPROGRAMM_SATZ = {
    "fehlt": "pruef.rollenprogramm_fehlt: Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg fehlt.",
    "modus": "pruef.rollenprogramm_modus: Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg hat nicht root:wb-bruecke mit Modus 2755.",
    "nicht_pruefbar": "pruef.rollenprogramm_nicht_pruefbar: Das Rollenprogramm war nicht pruefbar.",
}
SCHLUESSEL_SATZ = {
    "fehlt": "pruef.schluessel_fehlt: Die Schluesseldatei fehlt.",
    "art": "pruef.schluessel_art: Die Schluesseldatei ist keine gewoehnliche Datei.",
    "eigentuemer": "pruef.schluessel_eigentuemer: Die Schluesseldatei gehoert nicht dem Nutzer.",
    "modus": "pruef.schluessel_modus: Die Schluesseldatei hat nicht den Modus 0600.",
    "leer": "pruef.schluessel_leer: Die Schluesseldatei ist leer.",
    "nicht_pruefbar": "pruef.schluessel_nicht_pruefbar: Die Schluesseldatei war nicht pruefbar.",
}


HINWEIS_SUDO = "pruef.hinweis_sudo: Ohne root waren nicht alle Pruefungen moeglich; fuer diese Pruefungen mit sudo aufrufen."


def _nicht_pruefbar(lage):
    """Ubuntu: meldet irgendeine der acht Prüfungen `nicht_pruefbar`?"""
    zustaende = [d["zustand"] for d in lage["dienste"]] + [lage.get("rollengruppe", "ok"), lage.get("rollenprogramm", "ok"), lage["apparmor"], lage["einstellungen"]["zustand"], lage["nutzerordner"],
                                                           lage["schluessel"], lage["copilot"], lage["vault"], lage["paketstand"]["manifest"]]
    return "nicht_pruefbar" in zustaende


def pruefbefehl(aufruf, lage):
    """Gibt (rc, strom, zeilen) zurück. `aufruf` ist die Liste der Argumente nach `wb pruefen`."""
    plattform = lage.get("plattform", "windows")
    if aufruf == ["--vor-entfernen"] and plattform == "linux":
        # Nur `prerm` ruft so auf. Der Befehl gibt eine Information und scheitert nie (rc 0), damit das Entfernen nicht scheitert.
        zeilen = []
        for z, tabelle, rest in ((lage["schluessel"], SCHLUESSEL_SATZ, RESTZEILE), (lage["copilot"], COPILOT_SATZ, COPILOT_RESTZEILE)):
            if z == "nicht_pruefbar":
                zeilen.append(tabelle["nicht_pruefbar"])
            elif z != "fehlt":
                zeilen.append(rest)
        return 0, "stdout", zeilen
    if aufruf:
        return 64, "stderr", ["pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht."]
    linux = plattform == "linux"
    befunde = []

    if plattform == "windows":
        _windows_dienste(lage, befunde)
    elif plattform == "macos":
        for schluessel, tabelle in (("launchd", LAUNCHD_SATZ), ("schluesselbund", SCHLUESSELBUND_SATZ), ("einschluss", EINSCHLUSS_SATZ)):
            if lage[schluessel] != "ok":
                befunde.append(tabelle[lage[schluessel]])
    else:
        dienste = {d["name"]: d["zustand"] for d in lage["dienste"]}
        for n in DIENSTE_LINUX:
            z = dienste.get(n, "ok")
            if z == "ok":
                continue
            befunde.append(DIENST_SATZ[z].format(n=n))
        for feld, tabelle in (("rollengruppe", ROLLENGRUPPE_SATZ), ("rollenprogramm", ROLLENPROGRAMM_SATZ)):
            if lage.get(feld, "ok") != "ok":
                befunde.append(tabelle[lage[feld]])
        if lage["apparmor"] != "ok":
            befunde.append(APPARMOR_SATZ[lage["apparmor"]])

    e = lage["einstellungen"]
    if e["zustand"] == "fehlt":
        befunde.append("pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar.")
    elif e["zustand"] == "ungueltig":
        befunde.append(f"pruef.einstellungen_ungueltig: Die Einstellungsdatei ist ungueltig ({e['code']}).")
    elif e["zustand"] == "nicht_pruefbar":
        befunde.append("pruef.einstellungen_nicht_pruefbar: Die Einstellungsdatei war nicht pruefbar.")

    if linux and lage["nutzerordner"] != "ok":
        befunde.append(ORDNER_SATZ[lage["nutzerordner"]])
    if linux and lage.get("programmordner", "ok") == "verboten":
        befunde.append(PROGRAMMORDNER_SATZ)
    if linux:
        for a in sorted(lage.get("programmordner_abgelehnt", []), key=lambda e: LAUF_HARNESS.index(e["harness"])):
            befunde.append(ABGELEHNT_SATZ.format(n=a["harness"], o=a["ordner"]))
    if linux and lage["schluessel"] != "ok":
        befunde.append(SCHLUESSEL_SATZ[lage["schluessel"]])
    if linux and lage["copilot"] != "ok":
        befunde.append(COPILOT_SATZ[lage["copilot"]])
    if linux and lage.get("copilot_alt", False):
        befunde.append(COPILOT_ALTER_ORT)

    v = lage["vault"]
    if v != "ok":
        befunde.append({
            "fehlt": "pruef.vault_fehlt: Der Vault-Ordner fehlt.",
            "kein_ordner": "pruef.vault_kein_ordner: Der Vault-Pfad ist kein Ordner.",
            "rechte": "pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte.",
            "nicht_pruefbar": "pruef.vault_nicht_pruefbar: Der Vault-Ordner war nicht pruefbar.",
        }[v])

    p = lage["paketstand"]
    if p["manifest"] != "ok":
        befunde.append({
            "fehlt": "pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt.",
            "ungueltig": "pruef.paketstand_manifest_ungueltig: Das Manifest des Pakets ist ungueltig.",
            "nicht_pruefbar": "pruef.paketstand_nicht_pruefbar: Der Paketstand war nicht pruefbar.",
        }[p["manifest"]])
    else:
        if p["bau"] == "test":
            befunde.append("pruef.paketstand_testbau: Das Paket ist ein Testbau.")
        if p["version"] == "abweichend":
            befunde.append("pruef.paketstand_version: Die installierte Version weicht vom Manifest ab.")
        dateien = sorted(p["dateien"], key=lambda d: (d["pfad"].encode("ascii"), ART_REIHE[d["art"]]))
        for d in dateien[:MAX_DATEIZEILEN]:
            befunde.append({
                "fehlt": f"pruef.paketstand_datei_fehlt: Die Datei {d['pfad']} fehlt.",
                "abweichend": f"pruef.paketstand_datei_abweichend: Die Datei {d['pfad']} weicht vom Manifest ab.",
                "fremd": f"pruef.paketstand_datei_fremd: Die Datei {d['pfad']} steht nicht im Manifest.",
            }[d["art"]])
        if len(dateien) > MAX_DATEIZEILEN:
            befunde.append(f"pruef.paketstand_weitere: {len(dateien) - MAX_DATEIZEILEN} weitere Dateien weichen ab.")

    # Der Hinweis ist kein Befund: er färbt nichts rot und zählt nicht in `pruef.rot`.
    hinweis = []
    if linux and lage.get("ubuntu_profil") in HINWEIS_UBUNTU_PROFIL:
        hinweis.append(HINWEIS_UBUNTU_PROFIL[lage["ubuntu_profil"]])
    if linux and lage.get("system_bwrap", "gesperrt") == "ubuntu":
        hinweis.append(HINWEIS_SYSTEM_BWRAP_UBUNTU)
    if linux and lage.get("ca_zulassung") in HINWEIS_CA:
        hinweis.append(HINWEIS_CA[lage["ca_zulassung"]])
    if linux:
        for n in ("copilot", "pi"):
            if n in lage.get("harness", {}) and n not in [a["harness"] for a in lage.get("programmordner_abgelehnt", [])]:
                hinweis.append(HARNESS_HINWEIS[lage["harness"][n]].format(n=n))
    if linux and lage.get("ohne_root") and _nicht_pruefbar(lage):
        hinweis.append(HINWEIS_SUDO)
    if befunde:
        return 1, "stdout", befunde + hinweis + [f"pruef.rot: {len(befunde)} Befunde."]
    if lage["stelle"] == "fehlt" and lage["konten"] >= 2:
        return 20, "stdout", hinweis + ["pruef.stelle_einrichten: " + STELLE_SATZ]
    return 0, "stdout", hinweis + [f"pruef.ok: Alle {ANZAHL[plattform]} Pruefungen bestanden."]


# ---------------------------------------------------------------------------------------------------- Bauwerkzeuge

BAUWERKZEUG_NAME = re.compile(r"^[a-z0-9][a-z0-9.+-]{0,63}$")
BAUWERKZEUG_FASSUNG = re.compile(r"^[0-9A-Za-z.+~:-]{1,64}$")
BAUWERKZEUG_LIZENZ = re.compile(r"^[A-Za-z0-9.+() -]{1,128}$")
BAUWERKZEUG_SUMME = re.compile(r"^sha256:[0-9a-f]{64}$")
BAUWERKZEUG_PFLICHT = {"windows-x64": ("wix",), "ubuntu-26.04-x64": ("dpkg-dev", "build-essential"), "macos-arm64": ()}


def bauwerkzeuge_code(fall):
    """Fall der Form `bauwerkzeuge`: Text der Liste und `plattform`."""
    text = fall["text"]
    if not text.endswith("\n") or "\r" in text or any(ord(c) > 0x7E or (ord(c) < 0x20 and c not in "\n\t") for c in text):
        return "bauwerkzeuge_form"
    zeilen = [z.split("\t") for z in text[:-1].split("\n")]
    for f in zeilen:
        if len(f) != 5 or not BAUWERKZEUG_NAME.match(f[0]) or not BAUWERKZEUG_FASSUNG.match(f[1]) \
                or not BAUWERKZEUG_LIZENZ.match(f[2]) or not BAUWERKZEUG_SUMME.match(f[3]) or not f[4]:
            return "bauwerkzeuge_form"
    namen = [f[0] for f in zeilen]
    if len(set(namen)) != len(namen):
        return "bauwerkzeug_doppelt"
    if any(f[4] != "nein" for f in zeilen):
        return "bauwerkzeug_ausgeliefert"
    if any(n not in namen for n in BAUWERKZEUG_PFLICHT[fall["plattform"]]):
        return "bauwerkzeug_fehlt"
    return None


# ---------------------------------------------------------------------------------------------------- Schritt II: Systemnutzer (preinst)

SYS_UID_MIN, SYS_UID_MAX = 100, 999  # Bereich der Systemkonten nach adduser (FIRST_SYSTEM_UID, LAST_SYSTEM_UID); Annahme bis AL-15
SYSTEMNUTZER_HEIM = "/nonexistent"
SYSTEMNUTZER_SHELL = "/usr/sbin/nologin"
SYSTEMGRUPPEN = ("wb-halter",) + ROLLENGRUPPEN
PREINST_FREMD = 24


def _konto_gut(k):
    return (SYS_UID_MIN <= k["uid"] <= SYS_UID_MAX and k["gruppe"] == SYSTEMNUTZER and k["heim"] == SYSTEMNUTZER_HEIM
            and k["shell"] == SYSTEMNUTZER_SHELL)


def _gruppe_gut(g):
    return SYS_UID_MIN <= g["gid"] <= SYS_UID_MAX and g["mitglieder"] == []


def _systemnutzer_form(stand):
    if not isinstance(stand, dict) or set(stand) != {"konten", "gruppen"}:
        return False
    for k in stand["konten"]:
        if not isinstance(k, dict) or set(k) != {"name", "uid", "gruppe", "heim", "shell"} or not ist_ganz(k["uid"]):
            return False
    for g in stand["gruppen"]:
        if not isinstance(g, dict) or set(g) != {"name", "gid", "mitglieder"} or not ist_ganz(g["gid"]) \
                or not isinstance(g["mitglieder"], list):
            return False
    namen_k = [k["name"] for k in stand["konten"]]
    namen_g = [g["name"] for g in stand["gruppen"]]
    return len(set(namen_k)) == len(namen_k) and len(set(namen_g)) == len(namen_g)


def systemnutzer_code(fall):
    """Fall der Form `systemnutzer`: Konten und Gruppen vor und nach `preinst install|upgrade` und dessen Rückgabe."""
    vorher, nachher = fall["vorher"], fall["nachher"]
    if not _systemnutzer_form(vorher) or not _systemnutzer_form(nachher) or not ist_ganz(fall["preinst_rc"]):
        return "systemnutzer_form"
    vk = {k["name"]: k for k in vorher["konten"]}
    vg = {g["name"]: g for g in vorher["gruppen"]}
    nk = {k["name"]: k for k in nachher["konten"]}
    ng = {g["name"]: g for g in nachher["gruppen"]}
    fremd = (SYSTEMNUTZER in vk and not _konto_gut(vk[SYSTEMNUTZER])) or any(n in vg and not _gruppe_gut(vg[n]) for n in SYSTEMGRUPPEN)
    if fremd:
        # fail closed: ein fremdes Konto oder eine fremde Gruppe dieses Namens wird nie übernommen und nie geändert
        return None if (fall["preinst_rc"] == PREINST_FREMD and nachher == vorher) else "konto_fremd"
    if fall["preinst_rc"] != 0 or SYSTEMNUTZER not in nk or any(n not in ng for n in SYSTEMGRUPPEN):
        return "konto_fehlt"
    if not _konto_gut(nk[SYSTEMNUTZER]) or any(not SYS_UID_MIN <= ng[n]["gid"] <= SYS_UID_MAX for n in SYSTEMGRUPPEN):
        return "konto_form"
    if any(ng[n]["mitglieder"] for n in SYSTEMGRUPPEN):
        return "rollengruppe_mitglied"
    if any(nk.get(n) != k for n, k in vk.items()) or any(ng.get(n) != g for n, g in vg.items()):
        return "konto_verloren"
    return None


# ---------------------------------------------------------------------------------------------------- Schritt II: Umzug der Ablage (postinst, Update)

UMZUG_ZEILE = "werkbank: Umzug der Ablage gescheitert, Code 21"
UMZUG_FORM_SCHLUESSEL = {"name", "sha256", "modus", "eigentuemer"}


def _ablage_form(liste):
    if not isinstance(liste, list):
        return False
    for d in liste:
        if not isinstance(d, dict) or set(d) != UMZUG_FORM_SCHLUESSEL or not KENNUNG.match(d["name"]) or not SHA.match(d["sha256"]):
            return False
    namen = [d["name"] for d in liste]
    return len(set(namen)) == len(namen)


def umzug_code(fall):
    """Fall der Form `umzug`: die Ablage der Person (Schritt I) und die Ablage unter /var/lib/wb-halter vor und nach `postinst configure`
    beim Update auf Schritt II, dazu Journal und Rückgabe."""
    teile = ("quelle_vorher", "quelle_nachher", "ziel_vorher", "ziel_nachher")
    if any(not _ablage_form(fall[t]) for t in teile) or not ist_ganz(fall["postinst_rc"]) or not isinstance(fall["journal"], str):
        return "umzug_form"
    if fall["postinst_rc"] != 0:
        return "umzug_abbruch"  # Entscheidung 7: ein gescheiterter Umzug bricht die Installation nicht ab
    qv = {d["name"]: d for d in fall["quelle_vorher"]}
    zv = {d["name"]: d for d in fall["ziel_vorher"]}
    zn = {d["name"]: d for d in fall["ziel_nachher"]}
    sortiert = lambda l: sorted(l, key=lambda d: d["name"])  # noqa: E731
    unberuehrt = sortiert(fall["quelle_nachher"]) == sortiert(fall["quelle_vorher"]) and sortiert(fall["ziel_nachher"]) == sortiert(fall["ziel_vorher"])
    # Ein Name, der im Ziel schon mit anderem Inhalt liegt, ist ein Konflikt: dann bleibt alles, wie es war.
    konflikt = any(n in zv and zv[n]["sha256"] != d["sha256"] for n, d in qv.items())
    erwartet = dict(zv)
    for n, d in qv.items():
        erwartet[n] = {"name": n, "sha256": d["sha256"], "modus": "0600", "eigentuemer": SYSTEMNUTZER}
    fertig = not konflikt and zn == erwartet
    if konflikt and not unberuehrt:
        return "umzug_konflikt"
    for n, d in qv.items():
        z = zn.get(n)
        if z is not None and n not in zv and (z["sha256"] != d["sha256"] or z["modus"] != "0600" or z["eigentuemer"] != SYSTEMNUTZER):
            return "umzug_inhalt"
    if not fertig and not unberuehrt:
        return "umzug_halb"
    if fertig and fall["quelle_nachher"]:
        return "umzug_quelle_bleibt"
    gescheitert = unberuehrt and bool(qv) and not fertig
    if gescheitert and UMZUG_ZEILE not in fall["journal"]:
        return "umzug_code_fehlt"
    if not gescheitert and UMZUG_ZEILE in fall["journal"]:
        return "umzug_code_falsch"
    return None


# ---------------------------------------------------------------------------------------------------- Nachtrag Rolle aus der Gruppe: halter.json

HALTER_ROLLEN = {"bruecke": "wb-bruecke"}
HALTER_KENNUNG = KENNUNG


def halter_konfig(vorlage, uid):
    """Der Inhalt von /etc/werkbank/halter.json, den `postinst` Schritt 2c für die uid schreibt (Abschnitt „halter.json“): ein Ziel je Netzziel
    mit `schluessel: true` einer Route der Art `cloud`, Name = Host (nur in Kennungsform), Regeln nur GET und POST. Gibt (wert, weggelassen) zurück;
    `weggelassen` zählt die Netzziele ohne Kennungsform oder ohne Regel (je eine Journalzeile)."""
    ziele, weg = {}, 0
    for route in vorlage.get("routen", []):
        if route.get("art") != "cloud":
            continue
        for nz in route.get("netzziele", []):
            if nz.get("schluessel") is not True:
                continue
            regeln = [f"{r['methode']} {r['pfad']}" for r in nz.get("pfade", []) if r.get("methode") in ("GET", "POST")]
            host = nz.get("host", "")
            if not HALTER_KENNUNG.match(host) or not regeln or host in ziele:
                weg += 1
                continue
            ziele[host] = {"host": host, "port": nz["port"], "tls": True, "praefix": "", "kopf": "authorization", "vorsatz": "Bearer ",
                           "schluessel": "anmeldung." + route["schluessel"], "pfade": regeln}
    wert = {"fassung": 1, "konten": {str(uid): {"person": None}}, "rollen": dict(HALTER_ROLLEN), "ziele": ziele, "server": {}, "kids": {}}
    return wert, weg
