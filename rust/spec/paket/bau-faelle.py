#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Vertrags Paketbau und Prüfbefehl (faelle/, erwartet/).

Aufruf: python3 -I rust/spec/paket/bau-faelle.py
Die erwarteten Codes und Zeilen stehen hier von Hand, unabhängig von referenz.py; pruefen.py vergleicht beide.
Formen: manifest (Bytes des Manifests), liste (Positiv- und Ausschlussliste mit Baum), stueckliste (Ausgabe von
`wb lizenzen`), pruefung (Aufruf und Lage des Rechners für `wb pruefen`).
"""
import base64
import copy
import hashlib
import json
import os
import shutil
from pathlib import Path

HIER = Path(__file__).resolve().parent


def sha(name):
    return hashlib.sha256(name.encode("utf-8")).hexdigest()


def datei(pfad, art, groesse=1000):
    return {"pfad": pfad, "groesse": groesse, "sha256": sha(pfad), "art": art}


MANIFEST = {
    "fassung": 1,
    "paket": "werkbank",
    "version": "0.1.0",
    "plattform": "windows-x64",
    "bau": "auslieferung",
    "merkmale": [],
    "dateien": [
        datei("bin/wb.exe", "programm", 4200000),
        datei("bin/wb-halter.exe", "dienst", 3100000),
        datei("bin/wb-plattform.exe", "dienst", 3300000),
        datei("lizenzen/sbom.cdx.json", "daten", 90000),
        datei("lizenzen/notice-rangkern.txt", "lizenz", 11000),
    ],
    "bibliotheken": 148,
}


def text(obj):
    return json.dumps(obj, ensure_ascii=False, indent=1) + "\n"


def kompakt(obj):
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":")) + "\n"


def mit(aendern):
    obj = copy.deepcopy(MANIFEST)
    aendern(obj)
    return obj


FAELLE = []


def fall(name, eintrag, erwartet):
    FAELLE.append((name, eintrag, erwartet))


def fall_manifest(name, beschreibung, inhalt, code=None):
    eintrag = {"beschreibung": beschreibung, "form": "manifest"}
    if isinstance(inhalt, bytes):
        eintrag["b64"] = base64.b64encode(inhalt).decode("ascii")
    elif isinstance(inhalt, str):
        eintrag["text"] = inhalt
    else:
        eintrag["text"] = text(inhalt)
    fall("manifest-" + name, eintrag,
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


# ------------------------------------------------------------------------------------------ Manifest: gültig
fall_manifest("gueltig-windows", "Windows-Manifest mit den Pflichtdateien", MANIFEST)
LINUX_DATEIEN = [
    datei("usr/bin/werkbank-probelauf", "programm", 1000), datei("usr/bin/werkbank-pruefen", "programm", 1000), datei("usr/bin/werkbank-ordnermarke", "programm", 1200000), datei("usr/bin/werkbank-ca-zulassung", "programm", 4000),
    datei("usr/lib/werkbank/werkbank-lauf", "programm", 5000000), datei("usr/lib/werkbank/werkbank-lauf-kind", "programm", 900000), datei("usr/lib/werkbank/werkbank-kern", "dienst", 4200000), datei("usr/lib/werkbank/werkbank-bruecke", "dienst", 3100000),
    datei("usr/lib/werkbank/werkbank-modellweg", "dienst", 2900000), datei("usr/lib/werkbank/werkbank-halter", "dienst", 2500000), datei("usr/lib/werkbank/rangkern", "programm", 6000000),
    datei("usr/lib/werkbank/kette", "programm", 900000), datei("usr/lib/werkbank/such-mcp", "programm", 1500000), datei("usr/lib/werkbank/probelauf/drehbuch.py", "programm", 90000),
    datei("usr/lib/werkbank/probelauf/drehbuchlib/ablauf.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/bericht.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/bruecke.py", "programm", 20000),
    datei("usr/lib/werkbank/probelauf/drehbuchlib/fremd.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/konfig.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/lauf.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/messen.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/modell.py", "programm", 20000),
    datei("usr/lib/werkbank/probelauf/drehbuchlib/probelauf.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/schritte.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/seite.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/weiterreicher.py", "programm", 20000), datei("usr/lib/werkbank/probelauf/drehbuchlib/ziel.py", "programm", 20000),
    datei("usr/share/werkbank/probelauf/einstellungen.json", "daten", 900), datei("usr/share/werkbank/probelauf/konfig.json", "daten", 3000),
    datei("usr/lib/werkbank/werkbank-einschluss", "programm", 1800000),
    datei("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss", "daten", 1300),
    datei("usr/lib/systemd/user/werkbank-kern.service", "daten", 500), datei("usr/lib/systemd/user/werkbank-bruecke.service", "daten", 500),
    datei("usr/lib/systemd/user/wb-halter.service", "daten", 400),
    datei("usr/share/werkbank/sbom.cdx.json", "daten", 90000),
    datei("usr/share/werkbank/probelauf/vorfuehr-vault/betrieb/mi-4f9-wartungsfenster-presslinie.md", "daten", 900),
]


def ohne_datei(pfad):
    def a(o):
        o["dateien"] = [d for d in o["dateien"] if d["pfad"] != pfad]
    return a


def linux(aendern=None):
    obj = mit(lambda o: o.update(plattform="ubuntu-26.04-x64", dateien=copy.deepcopy(LINUX_DATEIEN)))
    if aendern:
        aendern(obj)
    return obj


fall_manifest("gueltig-linux", "Manifest des deb für Ubuntu 26.04: Pfade relativ zu `/`, alle Pflichtdateien", linux())
fall_manifest("gueltig-linux-manifest-name-fremd", "`manifest.json` ist unter Linux ein gewöhnlicher Pfad; nur `usr/share/werkbank/manifest.json` ist verboten",
              linux(lambda o: o["dateien"].append(datei("manifest.json", "daten"))))
fall_manifest("gueltig-testbau-mit-attrappe", "Testbau darf das Merkmal attrappe tragen",
              mit(lambda o: o.update(bau="test", merkmale=["attrappe", "test-harness"])))
fall_manifest("gueltig-merkmal-ohne-verbot", "Auslieferungsbau mit einem harmlosen Merkmal",
              mit(lambda o: o.update(merkmale=["windows-dienst"])))
fall_manifest("gueltig-version-grenze", "Version 255.255.65535 ist die Grenze von Windows Installer",
              mit(lambda o: o.update(version="255.255.65535")))
fall_manifest("gueltig-pfad-con-bindestrich", "Segment `con-x.txt` ist kein reservierter Name",
              mit(lambda o: o["dateien"].append(datei("daten/con-x.txt", "daten"))))
fall_manifest("gueltig-2000-dateien", "genau 2000 Dateien", kompakt(mit(lambda o: o["dateien"].extend(
    datei(f"daten/d{i:04d}.bin", "daten") for i in range(2000 - len(o["dateien"]))))))
fall_manifest("gueltig-groesse-grenze", "Größe 2147483647 und 0",
              mit(lambda o: o["dateien"].extend([datei("daten/gross.bin", "daten", 2147483647),
                                                 datei("daten/leer.bin", "daten", 0)])))
fall_manifest("gueltig-pfad-200", "Pfad mit genau 200 Zeichen",
              mit(lambda o: o["dateien"].append(datei("daten/" + "a" * 194, "daten"))))
fall_manifest("gueltig-reihenfolge-schluessel", "andere Reihenfolge der Schlüssel",
              json.dumps(dict(reversed(list(MANIFEST.items()))), indent=1) + "\n")

# ------------------------------------------------------------------------------------------ Manifest: Stufe 1, 2
fall_manifest("zu-gross", "ein gültiges Manifest, mit 1048576 Leerzeichen aufgefüllt: mehr als 1048576 Byte", text(MANIFEST), "zu_gross")
FAELLE[-1][1]["auffuellen"] = 1048576
fall_manifest("json-bom", "BOM vor dem Objekt", b"\xef\xbb\xbf" + text(MANIFEST).encode(), "json_ungueltig")
fall_manifest("json-kein-utf8", "Byte 0xFF im Text", text(MANIFEST).encode().replace(b"wb", b"w\xffb", 1), "json_ungueltig")
fall_manifest("json-syntax", "fehlendes Komma", text(MANIFEST).replace('"paket": "werkbank",', '"paket": "werkbank"', 1), "json_ungueltig")
fall_manifest("json-kein-objekt", "oberste Ebene ist eine Liste", "[1]\n", "json_ungueltig")
fall_manifest("json-nachlauf", "Text nach dem Objekt", text(MANIFEST) + "{}\n", "json_ungueltig")
fall_manifest("json-doppelter-schluessel-oben", "`bau` zweimal auf der obersten Ebene",
              text(MANIFEST).replace('"bau": "auslieferung",', '"bau": "test",\n "bau": "auslieferung",', 1), "json_ungueltig")
fall_manifest("json-doppelter-schluessel-datei", "`art` zweimal in einer Datei",
              text(MANIFEST).replace('"art": "programm"', '"art": "daten", "art": "programm"', 1), "json_ungueltig")
fall_manifest("json-doppelter-schluessel-escape", "`bau` und `\\u0062au` sind derselbe Schlüssel",
              text(MANIFEST).replace('"bau": "auslieferung",', '"bau": "auslieferung", "\\u0062au": "auslieferung",', 1), "json_ungueltig")

# ------------------------------------------------------------------------------------------ Manifest: Stufen 3 bis 7
fall_manifest("schluessel-unbekannt-oben", "zusätzlicher Schlüssel", mit(lambda o: o.update(signatur="x")), "schluessel_unbekannt")
fall_manifest("schluessel-unbekannt-datei", "zusätzlicher Schlüssel in einer Datei",
              mit(lambda o: o["dateien"][0].update(modus="0755")), "schluessel_unbekannt")
fall_manifest("schluessel-fehlt-oben", "`bau` fehlt", mit(lambda o: o.pop("bau")), "schluessel_fehlt")
fall_manifest("schluessel-fehlt-datei", "`sha256` fehlt in einer Datei", mit(lambda o: o["dateien"][1].pop("sha256")), "schluessel_fehlt")
fall_manifest("schluessel-fehlt-bibliotheken", "`bibliotheken` fehlt (A75: Zahl der Stückliste)", mit(lambda o: o.pop("bibliotheken")), "schluessel_fehlt")
fall_manifest("typ-fassung-text", "`fassung` als Text", mit(lambda o: o.update(fassung="1")), "typ_falsch")
fall_manifest("typ-fassung-bruch", "`fassung` 1.0 ist keine Ganzzahl (das Schema hält es für 1)",
              text(MANIFEST).replace('"fassung": 1,', '"fassung": 1.0,', 1), "typ_falsch")
fall_manifest("typ-groesse-text", "`groesse` als Text", mit(lambda o: o["dateien"][0].update(groesse="9")), "typ_falsch")
fall_manifest("typ-merkmale-text", "`merkmale` als Text", mit(lambda o: o.update(merkmale="attrappe")), "typ_falsch")
fall_manifest("typ-datei-kein-objekt", "ein Eintrag von `dateien` ist ein Text", mit(lambda o: o["dateien"].append("x")), "typ_falsch")
fall_manifest("fassung-2", "`fassung` 2", mit(lambda o: o.update(fassung=2)), "fassung_unbekannt")
fall_manifest("anzahl-dateien-leer", "keine Datei", mit(lambda o: o.update(dateien=[])), "anzahl")
fall_manifest("anzahl-dateien-2001", "2001 Dateien", kompakt(mit(lambda o: o["dateien"].extend(
    datei(f"daten/d{i:04d}.bin", "daten") for i in range(2001 - len(o["dateien"]))))), "anzahl")
fall_manifest("anzahl-merkmale-17", "17 Merkmale", mit(lambda o: o.update(bau="test", merkmale=[f"m{i}" for i in range(17)])), "anzahl")
fall_manifest("anzahl-bibliotheken-0", "0 Bibliotheken", mit(lambda o: o.update(bibliotheken=0)), "anzahl")

# ------------------------------------------------------------------------------------------ Manifest: Stufen 8 bis 13
fall_manifest("version-form-zwei-teile", "Version `1.0`", mit(lambda o: o.update(version="1.0")), "version_form")
fall_manifest("version-form-vorabversion", "Version `1.2.3-rc1` (Windows Installer kennt keine Vorabversion)",
              mit(lambda o: o.update(version="1.2.3-rc1")), "version_form")
fall_manifest("version-form-fuehrende-null", "Version `01.2.3`", mit(lambda o: o.update(version="01.2.3")), "version_form")
fall_manifest("version-bereich-256", "erste Zahl 256 (das Schema sieht nur die Stellenzahl)", mit(lambda o: o.update(version="256.0.0")), "version_form")
fall_manifest("version-bereich-65536", "dritte Zahl 65536", mit(lambda o: o.update(version="1.0.65536")), "version_form")
fall_manifest("wert-plattform", "Plattform `freebsd-x64`", mit(lambda o: o.update(plattform="freebsd-x64")), "wert_unbekannt")
fall_manifest("wert-bau", "Bauart `entwurf`", mit(lambda o: o.update(bau="entwurf")), "wert_unbekannt")
fall_manifest("wert-paket", "Paketname `anders`", mit(lambda o: o.update(paket="anders")), "wert_unbekannt")
fall_manifest("wert-art", "Dateiart `skript`", mit(lambda o: o["dateien"][4].update(art="skript")), "wert_unbekannt")
fall_manifest("kennung-form-merkmal-gross", "Merkmal `Attrappe` verletzt die Kennungsform", mit(lambda o: o.update(bau="test", merkmale=["Attrappe"])), "kennung_form")
fall_manifest("pfad-form-gross", "Pfad mit Großbuchstaben", mit(lambda o: o["dateien"].append(datei("Bin/Wb.exe", "daten"))), "pfad_form")
fall_manifest("pfad-form-rueckwaerts", "Pfad mit Rückwärtsstrich", mit(lambda o: o["dateien"].append(datei("bin\\x.exe", "daten"))), "pfad_form")
fall_manifest("pfad-form-absolut", "Pfad beginnt mit `/`", mit(lambda o: o["dateien"].append(datei("/bin/x", "daten"))), "pfad_form")
fall_manifest("pfad-form-laufwerk", "Pfad mit Laufwerk", mit(lambda o: o["dateien"].append(datei("c:/x", "daten"))), "pfad_form")
fall_manifest("pfad-form-doppelter-strich", "Pfad mit `//`", mit(lambda o: o["dateien"].append(datei("bin//x", "daten"))), "pfad_form")
fall_manifest("pfad-form-punkt-punkt", "Segment `..` (das Schema lässt die Zeichen zu)", mit(lambda o: o["dateien"].append(datei("bin/../x", "daten"))), "pfad_form")
fall_manifest("pfad-form-punkt-ende", "Segment endet mit einem Punkt (Windows streicht ihn)", mit(lambda o: o["dateien"].append(datei("bin/x.", "daten"))), "pfad_form")
fall_manifest("pfad-form-reserviert", "Segment `nul.txt` ist ein reservierter Windows-Name", mit(lambda o: o["dateien"].append(datei("daten/nul.txt", "daten"))), "pfad_form")
fall_manifest("pfad-form-manifest", "`manifest.json` steht nie in der eigenen Liste", mit(lambda o: o["dateien"].append(datei("manifest.json", "daten"))), "pfad_form")
fall_manifest("pfad-form-201", "Pfad mit 201 Zeichen", mit(lambda o: o["dateien"].append(datei("daten/" + "a" * 195, "daten"))), "pfad_form")
fall_manifest("pruefsumme-gross", "Prüfsumme in Großbuchstaben", mit(lambda o: o["dateien"][0].update(sha256=sha("x").upper())), "pruefsumme_form")
fall_manifest("pruefsumme-kurz", "Prüfsumme mit 63 Zeichen", mit(lambda o: o["dateien"][0].update(sha256=sha("x")[:63])), "pruefsumme_form")
fall_manifest("groesse-negativ", "Größe -1", mit(lambda o: o["dateien"][0].update(groesse=-1)), "groesse_bereich")
fall_manifest("groesse-zu-gross", "Größe 2147483648", mit(lambda o: o["dateien"][0].update(groesse=2147483648)), "groesse_bereich")

# ------------------------------------------------------------------------------------------ Manifest: Stufen 14 bis 17
fall_manifest("pfad-doppelt", "derselbe Pfad zweimal", mit(lambda o: o["dateien"].append(copy.deepcopy(o["dateien"][4]))), "pfad_doppelt")
fall_manifest("merkmal-doppelt", "Merkmal zweimal", mit(lambda o: o.update(merkmale=["a", "a"])), "merkmal_doppelt")
fall_manifest("merkmal-verboten-attrappe", "Auslieferungsbau mit dem Merkmal attrappe (A: Attrappe nur in Testbauten)",
              mit(lambda o: o.update(merkmale=["attrappe"])), "merkmal_verboten")
fall_manifest("merkmal-verboten-test-anfang", "Auslieferungsbau mit einem Merkmal, das mit `test` beginnt",
              mit(lambda o: o.update(merkmale=["test-harness"])), "merkmal_verboten")
fall_manifest("pflichtdatei-fehlt", "`bin/wb-halter.exe` fehlt", mit(lambda o: o["dateien"].pop(1)), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-falsche-art", "`bin/wb-halter.exe` ist als `programm` eingetragen", mit(lambda o: o["dateien"][1].update(art="programm")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-sbom-fehlt", "die Stückliste fehlt im Paket (A75)", mit(lambda o: o["dateien"].pop(3)), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-andere-plattform", "Ubuntu-Manifest mit den Dateinamen von Windows", mit(lambda o: o.update(plattform="ubuntu-26.04-x64")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-einstellung", "die Vorführ-Einstellungen (Vorlage) fehlen",
              linux(ohne_datei("usr/share/werkbank/probelauf/einstellungen.json")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-apparmor", "das AppArmor-Profil fehlt", linux(ohne_datei("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-starter", "der feste Starter `werkbank-einschluss` fehlt (Form A besteht aus Profil und Starter)", linux(ohne_datei("usr/lib/werkbank/werkbank-einschluss")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-profil-altname-lauf", "das Profil am Pfad `werkbank-lauf` (früherer Entwurf der Form A) zählt nicht mehr: `werkbank-lauf` hat kein Profil",
              linux(lambda o: (ohne_datei("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss")(o), o["dateien"].append(datei("etc/apparmor.d/usr.lib.werkbank.werkbank-lauf", "daten")))), "pflichtdatei_fehlt")
fall_manifest("gueltig-linux-profil-form-b", "Form B des Profils: eigene bwrap-Kopie mit Anhang auf genau diesen Pfad statt der Form A",
              linux(lambda o: (ohne_datei("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss")(o), o["dateien"].append(datei("etc/apparmor.d/usr.lib.werkbank.bwrap", "daten")))))
fall_manifest("pflichtdatei-linux-profil-altname", "das alte Profil `werkbank-einschluss-bwrap` (Anhang auf das System-bwrap) zählt nicht mehr als Form",
              linux(lambda o: (ohne_datei("etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss")(o), o["dateien"].append(datei("etc/apparmor.d/werkbank-einschluss-bwrap", "daten")))), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-kette", "das Programm `kette` (Prüfbefehl der Kette, Vertrag 5) fehlt", linux(ohne_datei("usr/lib/werkbank/kette")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-bruecke", "die Brücke zum Browser (`werkbank-bruecke`, Nachtrag 21.11.) fehlt", linux(ohne_datei("usr/lib/werkbank/werkbank-bruecke")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-einheit-bruecke", "die Einheit der Brücke fehlt", linux(ohne_datei("usr/lib/systemd/user/werkbank-bruecke.service")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-einheit", "die Einheit des Kern-Dienstes fehlt", linux(ohne_datei("usr/lib/systemd/user/werkbank-kern.service")), "pflichtdatei_fehlt")
# Nachtrag Halter (23.11.): der Halter und seine Benutzereinheit sind Pflichtdateien des deb.
fall_manifest("pflichtdatei-linux-halter", "Nachtrag Halter: das Programm `werkbank-halter` fehlt", linux(ohne_datei("usr/lib/werkbank/werkbank-halter")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-halter-andere-art", "Nachtrag Halter: `werkbank-halter` ist als `programm` statt `dienst` eingetragen: eine Pflichtdatei mit anderer `art` gilt als fehlend",
              linux(lambda o: [d.update(art="programm") for d in o["dateien"] if d["pfad"] == "usr/lib/werkbank/werkbank-halter"]), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-einheit-halter", "Nachtrag Halter: die Einheit `wb-halter.service` fehlt", linux(ohne_datei("usr/lib/systemd/user/wb-halter.service")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-laeufer", "der Läufer des Probelaufs fehlt", linux(ohne_datei("usr/lib/werkbank/probelauf/drehbuch.py")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-drehbuchlib", "ein Modul des Pakets drehbuchlib/ fehlt (drehbuch.py importiert es und kann ohne es nicht laufen)",
              linux(ohne_datei("usr/lib/werkbank/probelauf/drehbuchlib/ablauf.py")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-drehbuchlib-ganz", "das ganze Paket drehbuchlib/ fehlt, drehbuch.py liegt allein im deb",
              linux(lambda o: o.update(dateien=[d for d in o["dateien"] if "/drehbuchlib/" not in d["pfad"]])), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-lauf-kind", "das Trampolin `werkbank-lauf-kind` fehlt (ohne es startet `werkbank-lauf` kein Kind)", linux(ohne_datei("usr/lib/werkbank/werkbank-lauf-kind")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-lauf-kind-andere-art", "das Trampolin ist als `daten` eingetragen: eine Pflichtdatei mit anderer `art` gilt als fehlend",
              linux(lambda o: [d.update(art="daten") for d in o["dateien"] if d["pfad"] == "usr/lib/werkbank/werkbank-lauf-kind"]), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-ordnermarke", "der Befehl `werkbank-ordnermarke` fehlt (Pflichtdatei seit der Entscheidung des Paketeigners 21.11.)", linux(ohne_datei("usr/bin/werkbank-ordnermarke")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-ordnermarke-andere-art", "der Befehl `werkbank-ordnermarke` ist als `daten` eingetragen: eine Pflichtdatei mit anderer `art` gilt als fehlend",
              linux(lambda o: [d.update(art="daten") for d in o["dateien"] if d["pfad"] == "usr/bin/werkbank-ordnermarke"]), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-ca-zulassung", "der Befehl `werkbank-ca-zulassung` fehlt (Pflichtdatei seit der Entscheidung vom 21.11.2026)", linux(ohne_datei("usr/bin/werkbank-ca-zulassung")), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-ca-zulassung-andere-art", "der Befehl `werkbank-ca-zulassung` ist als `daten` eingetragen: eine Pflichtdatei mit anderer `art` gilt als fehlend",
              linux(lambda o: [d.update(art="daten") for d in o["dateien"] if d["pfad"] == "usr/bin/werkbank-ca-zulassung"]), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-linux-suche", "die Suche (`rangkern`) fehlt", linux(ohne_datei("usr/lib/werkbank/rangkern")), "pflichtdatei_fehlt")
fall_manifest("pfad-form-manifest-linux", "`usr/share/werkbank/manifest.json` steht nie in der eigenen Liste (Linux)",
              linux(lambda o: o["dateien"].append(datei("usr/share/werkbank/manifest.json", "daten"))), "pfad_form")
fall_manifest("merkmal-verboten-linux", "Auslieferungsbau des deb mit dem Merkmal attrappe", linux(lambda o: o.update(merkmale=["attrappe"])), "merkmal_verboten")
fall_manifest("wert-plattform-alt", "der alte Name `linux-x64` gilt nicht mehr", linux(lambda o: o.update(plattform="linux-x64")), "wert_unbekannt")

MAC_DATEIEN = [
    datei("Library/Werkbank/bin/werkbank-einrichten", "programm"), datei("Library/Werkbank/bin/werkbank-pruefen", "programm"),
    datei("Library/Werkbank/lib/werkbank-halter", "dienst"), datei("Library/Werkbank/lib/werkbank-lauf", "programm"),
    datei("Library/LaunchDaemons/werkbank.halter.plist", "daten"), datei("Library/Werkbank/sbom.cdx.json", "daten"),
]


def mac(aendern=None):
    obj = mit(lambda o: o.update(plattform="macos-arm64", dateien=copy.deepcopy(MAC_DATEIEN)))
    if aendern:
        aendern(obj)
    return obj


fall_manifest("gueltig-macos", "Manifest des pkg für macOS (Apple-Silicon): Pfade relativ zu `/`, alle Pflichtdateien", mac())
fall_manifest("pflichtdatei-macos-halter", "das Programm des Halters fehlt", mac(lambda o: o["dateien"].pop(2)), "pflichtdatei_fehlt")
fall_manifest("pflichtdatei-macos-plist", "die launchd-Datei des Halters fehlt", mac(lambda o: o["dateien"].pop(4)), "pflichtdatei_fehlt")
fall_manifest("pfad-form-manifest-macos", "`Library/Werkbank/manifest.json` steht nie in der eigenen Liste (macOS)",
              mac(lambda o: o["dateien"].append(datei("Library/Werkbank/manifest.json", "daten"))), "pfad_form")


# ------------------------------------------------------------------------------------------ Manifest: Reihenfolge
fall_manifest("reihenfolge-unbekannt-vor-fehlt", "unbekannter Schlüssel und fehlender Pflichtschlüssel: Stufe 3",
              mit(lambda o: (o.pop("bau"), o.update(x=1))), "schluessel_unbekannt")
fall_manifest("reihenfolge-pfad-vor-pruefsumme", "Pfad und Prüfsumme falsch: Stufe 11",
              mit(lambda o: o["dateien"].append({"pfad": "Bin/X", "groesse": 1, "sha256": "x", "art": "daten"})), "pfad_form")
fall_manifest("reihenfolge-doppelt-vor-verboten", "doppelter Pfad und verbotenes Merkmal: Stufe 14",
              mit(lambda o: (o["dateien"].append(copy.deepcopy(o["dateien"][0])), o.update(merkmale=["attrappe"]))), "pfad_doppelt")
fall_manifest("reihenfolge-verboten-vor-pflicht", "verbotenes Merkmal und fehlende Pflichtdatei: Stufe 16",
              mit(lambda o: (o["dateien"].pop(1), o.update(merkmale=["attrappe"]))), "merkmal_verboten")

# ------------------------------------------------------------------------------------------ Listen
LISTE = """# Positivliste des Windows-Pakets: jede Datei im Paket steht hier, sonst kommt sie nicht mit.
programm bin/wb.exe bin:wb:wb
dienst bin/wb-halter.exe bin:halter:wb-halter
dienst bin/wb-plattform.exe bin:plattform:wb-plattform
daten lizenzen/sbom.cdx.json erzeugt:sbom
lizenz lizenzen/notice-rangkern.txt datei:crates/rangkern/NOTICE
"""
AUSSCHLUSS = """# Ausschlussliste: nichts davon kommt ins Paket, auch wenn es im Baum liegt.
*.pdb  # Debug-Symbole sind kein Teil der Auslieferung
*.py  # Kein Python im Paket
*.sh  # Kein Shell-Werkzeug im Paket
*.rs  # Kein Quelltext im Paket
tests/  # Kein Testwerkzeug im Paket
bin/wb-attrappe.exe  # Die Attrappe gehoert nur in Testbauten
crate:testwerkzeug-beispiel  # Testwerkzeug, kein Teil der Auslieferung
"""
QUELLEN = ["bin:wb:wb", "bin:halter:wb-halter", "bin:plattform:wb-plattform", "erzeugt:sbom",
           "datei:crates/rangkern/NOTICE", "bin:testwerkzeug-beispiel:pruefer"]
BAUM = ["bin/wb.exe", "bin/wb-halter.exe", "bin/wb-plattform.exe", "lizenzen/sbom.cdx.json",
        "lizenzen/notice-rangkern.txt", "manifest.json"]


def fall_liste(name, beschreibung, code=None, liste=LISTE, ausschluss=AUSSCHLUSS, quellen=None, baum=None, modi=None):
    eintrag = {"beschreibung": beschreibung, "form": "liste", "liste": liste, "ausschluss": ausschluss,
               "quellen": QUELLEN if quellen is None else quellen, "baum": BAUM if baum is None else baum}
    if modi is not None:
        eintrag["modi"] = modi
    fall("liste-" + name, eintrag, {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_liste("gueltig", "Liste, Ausschluss und Baum stimmen; manifest.json im Baum ist erlaubt")
fall_liste("gueltig-gross-im-baum", "Der Baum darf Großbuchstaben tragen (Windows); verglichen wird in Kleinschrift",
           baum=["Bin/WB.exe", "bin/wb-halter.exe", "bin/wb-plattform.exe", "lizenzen/sbom.cdx.json",
                 "lizenzen/notice-rangkern.txt", "MANIFEST.JSON"])
LISTE_LINUX = """# Positivliste des deb-Pakets für Ubuntu 26.04 (Ausschlussliste des deb: ohne `*.py`, der Läufer ist ein Python-Skript)
programm usr/bin/werkbank-probelauf datei:paket/deb/werkbank-probelauf
programm usr/bin/werkbank-pruefen bin:pruefen:werkbank-pruefen
programm usr/lib/werkbank/werkbank-lauf bin:lauf:werkbank-lauf
programm usr/lib/werkbank/werkbank-einschluss bin:einschluss:werkbank-einschluss
programm usr/lib/werkbank/kette bin:formate:kette
daten etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss datei:paket/ubuntu-26.04-x64/usr.lib.werkbank.werkbank-einschluss
dienst usr/lib/werkbank/werkbank-kern bin:kern:werkbank-kern
dienst usr/lib/werkbank/werkbank-modellweg bin:modellweg:werkbank-modellweg
programm usr/lib/werkbank/probelauf/drehbuch.py datei:spec/vorfuehrung/laeufer/drehbuch.py
daten usr/share/werkbank/probelauf/einstellungen.json datei:spec/einstellungen/vorfuehrung.json
daten usr/share/werkbank/sbom.cdx.json erzeugt:sbom
"""
QUELLEN_LINUX = ["datei:paket/deb/werkbank-probelauf", "bin:pruefen:werkbank-pruefen", "bin:lauf:werkbank-lauf", "bin:kern:werkbank-kern",
                 "bin:einschluss:werkbank-einschluss", "bin:formate:kette", "datei:paket/ubuntu-26.04-x64/usr.lib.werkbank.werkbank-einschluss",
                 "bin:modellweg:werkbank-modellweg", "datei:spec/vorfuehrung/laeufer/drehbuch.py",
                 "datei:spec/einstellungen/vorfuehrung.json", "erzeugt:sbom", "datei:paket/andere-einstellungen.json"]
BAUM_LINUX = ["usr/bin/werkbank-probelauf", "usr/bin/werkbank-pruefen", "usr/lib/werkbank/werkbank-lauf", "usr/lib/werkbank/werkbank-kern",
              "usr/lib/werkbank/werkbank-einschluss", "usr/lib/werkbank/kette", "etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss",
              "usr/lib/werkbank/werkbank-modellweg", "usr/lib/werkbank/probelauf/drehbuch.py",
              "usr/share/werkbank/probelauf/einstellungen.json", "usr/share/werkbank/sbom.cdx.json", "usr/share/werkbank/manifest.json"]
AUSSCHLUSS_LINUX = AUSSCHLUSS.replace("*.py  # Kein Python im Paket\n", "*.pyc  # Kein uebersetzter Python-Code im Paket\n")
fall_liste("gueltig-linux", "deb-Liste: Die Vorlage der Einstellungen kommt vom Läufer; `usr/share/werkbank/manifest.json` im Baum ist erlaubt; `drehbuch.py` ist erlaubt, weil die Ausschlussliste des deb kein `*.py` führt",
           liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX)
fall_liste("linux-python-im-windows-paket", "dieselbe Liste mit der Ausschlussliste des Windows-Pakets: `drehbuch.py` trifft `*.py`", "ausschluss_getroffen",
           liste=LISTE_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX)
fall_liste("linux-pyc-im-baum", "ein übersetzter Python-Baustein im Baum des deb", "ausschluss_getroffen",
           liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX + ["usr/lib/werkbank/probelauf/__pycache__/drehbuch.pyc"])
fall_liste("vorfuehrung-einstellung-quelle", "die Vorlage der Einstellungen hat nicht die Datei des Läufers als Quelle", "vorfuehrung_einstellung",
           liste=LISTE_LINUX.replace("datei:spec/einstellungen/vorfuehrung.json", "datei:paket/andere-einstellungen.json"),
           ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX)
fall_liste("vorfuehrung-einstellung-quelle-alt", "die Quelle der Vorlage ist die frühere, nicht vorhandene Datei des Läufers", "vorfuehrung_einstellung",
           liste=LISTE_LINUX.replace("datei:spec/einstellungen/vorfuehrung.json", "datei:paket/laeufer/einstellungen.json"),
           ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX + ["datei:paket/laeufer/einstellungen.json"], baum=BAUM_LINUX)

# Modi im Baum (Entscheidung 20.11.): kein Eintrag ist für Gruppe oder Welt schreibbar, egal unter welcher umask gebaut wurde
MODI_GUT = {"usr/": "0755", "usr/lib/": "0755", "usr/lib/werkbank/": "0755", "usr/lib/werkbank/probelauf/": "0755",
            "usr/lib/werkbank/probelauf/drehbuchlib/": "0755", "usr/share/werkbank/probelauf/vorfuehr-vault/": "0755",
            "usr/bin/werkbank-pruefen": "0755", "usr/lib/werkbank/werkbank-einschluss": "0755",
            "usr/share/werkbank/probelauf/einstellungen.json": "0644", "usr/share/werkbank/manifest.json": "0644"}


def fall_modi(name, beschreibung, aenderung, code=None, baum=None):
    m = dict(MODI_GUT)
    m.update(aenderung)
    fall_liste("modus-" + name, beschreibung, code, liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX,
               baum=BAUM_LINUX if baum is None else baum, modi=m)


fall_modi("gueltig", "Ordner 0755, Programme 0755, Daten 0644: gültig", {})
fall_modi("ordner-gruppe-schreibbar", "ein Ordner mit 0775 (Bau unter umask 002 ohne Normalisierung)", {"usr/lib/werkbank/": "0775"}, "modus_schreibbar")
fall_modi("unterordner-gruppe-schreibbar", "der Ordner des Läufers mit 0775", {"usr/lib/werkbank/probelauf/": "0775"}, "modus_schreibbar")
fall_modi("vault-ordner-gruppe-schreibbar", "der Ordner der Vorführ-Vorlage mit 0775", {"usr/share/werkbank/probelauf/vorfuehr-vault/": "0775"}, "modus_schreibbar")
fall_modi("ordner-welt-schreibbar", "ein Ordner mit 0757 (nur Welt schreibbar)", {"usr/lib/": "0757"}, "modus_schreibbar")
fall_modi("programm-gruppe-schreibbar", "ein Programm mit 0775", {"usr/bin/werkbank-pruefen": "0775"}, "modus_schreibbar")
fall_modi("datei-gruppe-schreibbar", "eine Datei mit 0664", {"usr/share/werkbank/probelauf/einstellungen.json": "0664"}, "modus_schreibbar")
fall_modi("datei-welt-schreibbar", "eine Datei mit 0646 (nur Welt schreibbar)", {"usr/share/werkbank/manifest.json": "0646"}, "modus_schreibbar")
fall_modi("gruppe-nur-lesbar-ist-gut", "0750 und 0640 (Gruppe ohne Schreibrecht) sind nicht schreibbar: gültig; die Sollmodi fordert dieser Fall nicht",
          {"usr/lib/werkbank/": "0750", "usr/share/werkbank/manifest.json": "0640"})
fall_modi("ungelistet-vor-modus", "Stufe 7 vor Stufe 9: ein ungelisteter Eintrag und ein schreibbarer Ordner: `ungelistet`",
          {"usr/lib/werkbank/": "0775"}, "ungelistet", baum=BAUM_LINUX + ["usr/lib/werkbank/fremd.bin"])

# Das Paket drehbuchlib/ gehört zum Läufer: drehbuch.py importiert es (Nachtrag 20.11.)
MODULE_DL = ['ablauf', 'bericht', 'bruecke', 'fremd', 'konfig', 'lauf', 'messen', 'modell', 'probelauf', 'schritte', 'seite', 'weiterreicher', 'ziel']
ZIEL_DL = [f"usr/lib/werkbank/probelauf/drehbuchlib/{m}.py" for m in MODULE_DL]
QUELLE_DL = [f"datei:spec/vorfuehrung/laeufer/drehbuchlib/{m}.py" for m in MODULE_DL]
LISTE_LINUX_DL = LISTE_LINUX + "".join(f"programm {z} {q}\n" for z, q in zip(ZIEL_DL, QUELLE_DL))
QUELLEN_LINUX_DL = QUELLEN_LINUX + QUELLE_DL
BAUM_LINUX_DL = BAUM_LINUX + ZIEL_DL
fall_liste("gueltig-linux-drehbuchlib", "deb-Liste mit dem Paket drehbuchlib/ des Läufers: jedes Modul steht namentlich in der Positivliste",
           liste=LISTE_LINUX_DL, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX_DL, baum=BAUM_LINUX_DL)
fall_liste("drehbuchlib-modul-ungelistet", "ein Modul von drehbuchlib/ liegt im Baum, steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LINUX_DL.replace(f"programm {ZIEL_DL[0]} {QUELLE_DL[0]}\n", ""), ausschluss=AUSSCHLUSS_LINUX,
           quellen=QUELLEN_LINUX_DL, baum=BAUM_LINUX_DL)
fall_liste("drehbuchlib-modul-fehlt-im-baum", "ein gelistetes Modul von drehbuchlib/ fehlt im Baum", "fehlt_im_baum",
           liste=LISTE_LINUX_DL, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX_DL, baum=BAUM_LINUX_DL[:-1])
fall_liste("drehbuchlib-quelle-fehlt", "die Quelle eines Moduls gibt es im Arbeitsbaum nicht", "quelle_fehlt",
           liste=LISTE_LINUX_DL, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX_DL[:-1], baum=BAUM_LINUX_DL)
fall_liste("drehbuchlib-pyc-im-baum", "ein übersetzter Baustein unter drehbuchlib/ trifft die Ausschlussliste", "ausschluss_getroffen",
           liste=LISTE_LINUX_DL, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX_DL,
           baum=BAUM_LINUX_DL + ["usr/lib/werkbank/probelauf/drehbuchlib/__pycache__/ablauf.cpython-314.pyc"])

# Die Anmeldung der Copilot CLI (Nachtrag 20.11.): `/usr/bin/werkbank-anmelden`, Quelle `bin:anmelden:werkbank-anmelden`
ANMELDEN = ("usr/bin/werkbank-anmelden", "bin:anmelden:werkbank-anmelden")
LISTE_LINUX_AN = LISTE_LINUX + f"programm {ANMELDEN[0]} {ANMELDEN[1]}\n"
fall_liste("gueltig-linux-anmeldung", "deb-Liste mit der Anmeldung der Copilot CLI: `/usr/bin/werkbank-anmelden` steht namentlich in der Positivliste",
           liste=LISTE_LINUX_AN, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX + [ANMELDEN[1]], baum=BAUM_LINUX + [ANMELDEN[0]])
fall_liste("anmeldung-ungelistet", "`/usr/bin/werkbank-anmelden` liegt im Baum, steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX + [ANMELDEN[0]])
# Nachtrag Halter (23.11.): `/usr/lib/werkbank/werkbank-halter` und die Einheit `wb-halter.service` stehen namentlich in der Positivliste.
HALTER = (("dienst", "usr/lib/werkbank/werkbank-halter", "bin:halter:werkbank-halter"),
          ("daten", "usr/lib/systemd/user/wb-halter.service", "datei:paket/ubuntu-26.04-x64/wb-halter.service"))
LISTE_LINUX_HA = LISTE_LINUX + "".join(f"{a} {z} {q}\n" for a, z, q in HALTER)
fall_liste("gueltig-linux-halter", "Nachtrag Halter: deb-Liste mit dem Halter und seiner Benutzereinheit",
           liste=LISTE_LINUX_HA, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX + [q for _, _, q in HALTER], baum=BAUM_LINUX + [z for _, z, _ in HALTER])
fall_liste("halter-ungelistet", "Nachtrag Halter: `werkbank-halter` liegt im Baum, steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX + [HALTER[0][1]])
fall_liste("halter-quelle-fehlt", "Nachtrag Halter: die Quelle `bin:halter:werkbank-halter` gibt es im Arbeitsbaum nicht (kein Crate `halter`)", "quelle_fehlt",
           liste=LISTE_LINUX_HA, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX + [HALTER[1][2]], baum=BAUM_LINUX + [z for _, z, _ in HALTER])
fall_liste("anmeldung-fehlt-im-baum", "`/usr/bin/werkbank-anmelden` ist gelistet und fehlt im Baum", "fehlt_im_baum",
           liste=LISTE_LINUX_AN, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX + [ANMELDEN[1]], baum=BAUM_LINUX)
fall_liste("anmeldung-quelle-fehlt", "die Quelle der Anmeldung gibt es im Arbeitsbaum nicht", "quelle_fehlt",
           liste=LISTE_LINUX_AN, ausschluss=AUSSCHLUSS_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX + [ANMELDEN[0]])

# Der Läufer im Paket (Nachtrag 20.11., Befund eines Testlaufs): Quellen des Läufers, der Konfiguration und des Vaults
import glob as _glob
VAULT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "such-socket", "vorfuehr-vault")
VAULT_REL = sorted(os.path.relpath(f, VAULT_DIR).replace(os.sep, "/") for f in _glob.glob(VAULT_DIR + "/**/*", recursive=True) if os.path.isfile(f))
assert len(VAULT_REL) == 21
ZIEL_VAULT = ["usr/share/werkbank/probelauf/vorfuehr-vault/" + v.lower() for v in VAULT_REL]
QUELLE_VAULT = ["datei:spec/such-socket/vorfuehr-vault/" + v for v in VAULT_REL]
LAEUFER_QUELLE = "datei:spec/vorfuehrung/laeufer/werkbank-probelauf"
KONFIG_QUELLE = "datei:spec/vorfuehrung/laeufer/konfig.probelauf.json"
LISTE_LAEUFER = (LISTE_LINUX_DL.replace("datei:paket/deb/werkbank-probelauf", LAEUFER_QUELLE)
                 + f"daten usr/share/werkbank/probelauf/konfig.json {KONFIG_QUELLE}\n"
                 + "".join(f"daten {z} {q}\n" for z, q in zip(ZIEL_VAULT, QUELLE_VAULT)))
QUELLEN_LAEUFER = [q for q in QUELLEN_LINUX_DL if q != "datei:paket/deb/werkbank-probelauf"] + [LAEUFER_QUELLE, KONFIG_QUELLE] + QUELLE_VAULT
BAUM_LAEUFER = BAUM_LINUX_DL + ["usr/share/werkbank/probelauf/konfig.json"] + ZIEL_VAULT
AUSSCHLUSS_LAEUFER = AUSSCHLUSS_LINUX + (
    "usr/lib/werkbank/probelauf/attrappen/  # Attrappen des Laeufers nie im Paket\n"
    "usr/lib/werkbank/probelauf/browser/  # Browserwerkzeug des Laeufers nie im Paket\n"
    "usr/lib/werkbank/probelauf/mutationsprobe.py  # Mutationsprobe nur im Arbeitsbaum\n"
    "usr/lib/werkbank/probelauf/test_werkbank_probelauf.py  # Test des Laeufers nur im Arbeitsbaum\n"
    "usr/bin/test_werkbank_probelauf.py  # Test des Laeufers nur im Arbeitsbaum\n")
fall_liste("gueltig-linux-laeufer", "deb-Liste mit Läufer aus spec/vorfuehrung/laeufer, konfig.probelauf.json als konfig.json und den 21 Dateien des Vaults einzeln",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER)
fall_liste("laeufer-quelle-fehlt", "die Quelle des Läufers gibt es im Arbeitsbaum nicht (noch nicht geliefert)", "quelle_fehlt",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=[q for q in QUELLEN_LAEUFER if q != LAEUFER_QUELLE], baum=BAUM_LAEUFER)
fall_liste("laeufer-konfig-quelle-fehlt", "die Konfiguration hat keine vorhandene Quelle (konfig.json gibt es im Arbeitsbaum nicht)", "quelle_fehlt",
           liste=LISTE_LAEUFER.replace(KONFIG_QUELLE, "datei:paket/laeufer/konfig.json"), ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER)
fall_liste("laeufer-konfig-attrappe-im-baum", "konfig.attrappe.json liegt im Baum des deb, steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER,
           baum=BAUM_LAEUFER + ["usr/share/werkbank/probelauf/konfig.attrappe.json"])
fall_liste("laeufer-vault-datei-ungelistet", "eine Datei der Vault-Vorlage liegt im Baum, steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LAEUFER.replace(f"daten {ZIEL_VAULT[0]} {QUELLE_VAULT[0]}\n", ""), ausschluss=AUSSCHLUSS_LAEUFER,
           quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER)
fall_liste("laeufer-vault-datei-fehlt-im-baum", "eine gelistete Datei der Vault-Vorlage fehlt im Baum", "fehlt_im_baum",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER[:-1])
fall_liste("laeufer-vault-quelle-fehlt", "die Quelle einer Datei der Vault-Vorlage gibt es nicht", "quelle_fehlt",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER[:-1], baum=BAUM_LAEUFER)
fall_liste("laeufer-vault-ordner-als-ziel", "ein Ordner statt einzelner Dateien: das Ziel des Ordners ist kein Baumpfad, die Dateien darunter sind ungelistet", "ungelistet",
           liste=LISTE_LAEUFER.replace("".join(f"daten {z} {q}\n" for z, q in zip(ZIEL_VAULT, QUELLE_VAULT)),
                                       "daten usr/share/werkbank/probelauf/vorfuehr-vault datei:spec/such-socket/vorfuehr-vault\n"),
           ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER + ["datei:spec/such-socket/vorfuehr-vault"], baum=BAUM_LAEUFER)
for _n, _p in (("test-im-laeuferordner", "usr/lib/werkbank/probelauf/test_werkbank_probelauf.py"), ("test-neben-dem-laeufer", "usr/bin/test_werkbank_probelauf.py"),
               ("attrappen", "usr/lib/werkbank/probelauf/attrappen/test.py"), ("browser", "usr/lib/werkbank/probelauf/browser/seite.html"),
               ("mutationsprobe", "usr/lib/werkbank/probelauf/mutationsprobe.py")):
    fall_liste("laeufer-" + _n + "-im-baum", f"{_n} des Läufers liegt im Baum des deb und trifft die Ausschlussliste", "ausschluss_getroffen",
               liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER + [_p])
fall_liste("profil-am-pfad-lauf-ungelistet", "ein Profil am Pfad von `werkbank-lauf` liegt im Baum (früherer Entwurf der Form A), steht aber nicht in der Positivliste", "ungelistet",
           liste=LISTE_LAEUFER, ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER,
           baum=BAUM_LAEUFER + ["etc/apparmor.d/usr.lib.werkbank.werkbank-lauf"])
fall_liste("profil-am-pfad-lauf-gelistet-ohne-quelle", "die Positivliste führt ein Profil am Pfad von `werkbank-lauf`, dessen Quelle es nicht gibt", "quelle_fehlt",
           liste=LISTE_LAEUFER + "daten etc/apparmor.d/usr.lib.werkbank.werkbank-lauf datei:paket/ubuntu-26.04-x64/usr.lib.werkbank.werkbank-lauf\n",
           ausschluss=AUSSCHLUSS_LAEUFER, quellen=QUELLEN_LAEUFER, baum=BAUM_LAEUFER + ["etc/apparmor.d/usr.lib.werkbank.werkbank-lauf"])

# Bauwerkzeuge liegen nie im Paket (Entscheidung): ihre Dateien stehen in der Ausschlussliste der Plattform
AUSSCHLUSS_BW_LINUX = AUSSCHLUSS_LINUX + (
    "usr/bin/dpkg-deb  # Bauwerkzeug dpkg-dev, nie im Paket\n"
    "usr/bin/dpkg-shlibdeps  # Bauwerkzeug dpkg-dev, nie im Paket\n"
    "usr/bin/dpkg-buildpackage  # Bauwerkzeug dpkg-dev, nie im Paket\n"
    "usr/bin/gcc  # Bauwerkzeug build-essential, nie im Paket\n"
    "usr/bin/make  # Bauwerkzeug build-essential, nie im Paket\n")
AUSSCHLUSS_BW_WINDOWS = AUSSCHLUSS + (
    "bin/wix.exe  # Bauwerkzeug WiX, nie im Paket\n"
    "wix/  # Bauwerkzeug WiX, nie im Paket\n")
fall_liste("gueltig-linux-bauwerkzeug-ausschluss", "deb-Liste mit den Bauwerkzeug-Mustern in der Ausschlussliste: nichts davon liegt im Baum",
           liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_BW_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX)
for _name, _datei in [("dpkg-deb", "usr/bin/dpkg-deb"), ("dpkg-shlibdeps", "usr/bin/dpkg-shlibdeps"), ("gcc", "usr/bin/gcc")]:
    fall_liste("bauwerkzeug-im-baum-" + _name, f"eine Datei des Bauwerkzeugs ({_datei}) liegt im Baum des deb", "ausschluss_getroffen",
               liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_BW_LINUX, quellen=QUELLEN_LINUX, baum=BAUM_LINUX + [_datei])
fall_liste("bauwerkzeug-gelistet", "eine Datei eines Bauwerkzeugs steht in der Positivliste", "ausschluss_getroffen",
           liste=LISTE_LINUX + "programm usr/bin/dpkg-deb datei:paket/dpkg-deb\n", ausschluss=AUSSCHLUSS_BW_LINUX,
           quellen=QUELLEN_LINUX + ["datei:paket/dpkg-deb"], baum=BAUM_LINUX + ["usr/bin/dpkg-deb"])
fall_liste("bauwerkzeug-wix-im-baum", "eine Datei von WiX liegt im Baum der MSI", "ausschluss_getroffen",
           ausschluss=AUSSCHLUSS_BW_WINDOWS, baum=BAUM + ["bin/wix.exe"])
fall_liste("bauwerkzeug-wix-ordner-im-baum", "ein Ordner von WiX liegt im Baum der MSI", "ausschluss_getroffen",
           ausschluss=AUSSCHLUSS_BW_WINDOWS, baum=BAUM + ["wix/extensions/wixtoolset.util.wixext.dll"])
fall_liste("bauwerkzeug-ausschluss-vor-ungelistet", "ein ausgeschlossenes Bauwerkzeug ergibt ausschluss_getroffen, nicht ungelistet (Stufe 6 vor 7)",
           "ausschluss_getroffen", liste=LISTE_LINUX, ausschluss=AUSSCHLUSS_BW_LINUX, quellen=QUELLEN_LINUX,
           baum=BAUM_LINUX + ["usr/bin/make"])
fall_liste("liste-form-absolut", "Ziel beginnt mit `/`", "liste_form", liste=LISTE + "daten /etc/x datei:x\n")
fall_liste("liste-form-art", "Art `skript`", "liste_form", liste=LISTE + "skript bin/x.exe bin:wb:x\n")
fall_liste("liste-form-zwei-felder", "Zeile mit zwei Feldern", "liste_form", liste=LISTE + "daten bin/x.exe\n")
fall_liste("liste-form-quelle-hoch", "Quelle mit `..`", "liste_form", liste=LISTE + "daten daten/x.txt datei:../x.txt\n")
fall_liste("liste-form-quelle-art", "Quelle ohne bekannte Form", "liste_form", liste=LISTE + "daten daten/x.txt url:x\n")
fall_liste("liste-form-erzeugt-ziel", "`erzeugt:sbom` mit fremdem Ziel", "liste_form", liste=LISTE + "daten daten/x.json erzeugt:sbom\n")
fall_liste("liste-form-ziel-gross", "Ziel mit Großbuchstaben", "liste_form", liste=LISTE + "daten Daten/X erzeugt:sbom\n")
fall_liste("ausschluss-form-ohne-grund", "Muster ohne Grund", "ausschluss_form", ausschluss=AUSSCHLUSS + "*.log\n")
fall_liste("ausschluss-form-grund-kurz", "Grund mit 11 Zeichen", "ausschluss_form", ausschluss=AUSSCHLUSS + "*.log  # elf Zeichen\n")
fall_liste("ausschluss-form-muster", "Muster mit Platzhalter mitten im Namen", "ausschluss_form", ausschluss=AUSSCHLUSS + "bin/*.exe  # kein solches Muster erlaubt\n")
fall_liste("liste-doppelt", "dasselbe Ziel zweimal", "liste_doppelt", liste=LISTE + "daten bin/wb.exe erzeugt:sbom\n".replace("erzeugt:sbom", "datei:crates/rangkern/NOTICE"))
fall_liste("quelle-fehlt", "eine gelistete Quelle gibt es im Arbeitsbaum nicht", "quelle_fehlt",
           quellen=[q for q in QUELLEN if q != "bin:halter:wb-halter"])
fall_liste("ausschluss-getroffen-ziel", "ein gelistetes Ziel trifft `*.sh`", "ausschluss_getroffen",
           liste=LISTE + "daten tools/gates.sh datei:gates.sh\n", quellen=QUELLEN + ["datei:gates.sh"],
           baum=BAUM + ["tools/gates.sh"])
fall_liste("ausschluss-getroffen-crate", "eine gelistete Binärdatei stammt aus einem ausgeschlossenen Crate", "ausschluss_getroffen",
           liste=LISTE + "programm bin/pruefer.exe bin:testwerkzeug-beispiel:pruefer\n", baum=BAUM + ["bin/pruefer.exe"])
fall_liste("ausschluss-getroffen-attrappe", "die Attrappe liegt im Baum, ist aber nicht gelistet: Ausschluss, nicht `ungelistet`", "ausschluss_getroffen",
           baum=BAUM + ["bin/wb-attrappe.exe"])
fall_liste("ausschluss-getroffen-pdb-gross", "`BIN/WB.PDB` im Baum trifft `*.pdb` in Kleinschrift", "ausschluss_getroffen",
           baum=BAUM + ["BIN/WB.PDB"])
fall_liste("ausschluss-getroffen-testordner", "Datei unter `tests/` im Baum", "ausschluss_getroffen", baum=BAUM + ["tests/x.exe"])
fall_liste("ungelistet", "eine Datei im Baum ohne Eintrag", "ungelistet", baum=BAUM + ["bin/wb-neu.exe"])
fall_liste("ungelistet-in-neuem-ordner", "eine Datei in einem neuen Ordner ohne Eintrag", "ungelistet", baum=BAUM + ["neu/x.dat"])
fall_liste("fehlt-im-baum", "ein gelistetes Ziel fehlt im Baum (kein Teilpaket)", "fehlt_im_baum", baum=[p for p in BAUM if p != "bin/wb-halter.exe"])
fall_liste("reihenfolge-form-vor-quelle", "Formfehler und fehlende Quelle: Stufe `liste_form`", "liste_form",
           liste=LISTE + "skript bin/x.exe bin:wb:x\n", quellen=[])
fall_liste("reihenfolge-ausschluss-vor-ungelistet", "ausgeschlossene und ungelistete Datei im Baum: Ausschluss zuerst", "ausschluss_getroffen",
           baum=BAUM + ["bin/wb-neu.exe", "bin/wb-attrappe.exe"])
fall_liste("reihenfolge-ungelistet-vor-fehlt", "ungelistete Datei und fehlendes Ziel: `ungelistet` zuerst", "ungelistet",
           baum=[p for p in BAUM if p != "bin/wb.exe"] + ["bin/wb-neu.exe"])

# ------------------------------------------------------------------------------------------ Stückliste
SL = ("anyhow\t1.0.100\tMIT OR Apache-2.0\tcrates.io\n"
      "serde\t1.0.228\tMIT OR Apache-2.0\tcrates.io\n"
      "zlib-rs\t0.5.2\tZlib\tcrates.io\n")
IM_PAKET = [{"name": "serde", "version": "1.0.228"}, {"name": "zlib-rs", "version": "0.5.2"}]


def fall_sl(name, beschreibung, inhalt, code=None, im_paket=None):
    fall("stueckliste-" + name,
         {"beschreibung": beschreibung, "form": "stueckliste", "text": inhalt,
          "im_paket": IM_PAKET if im_paket is None else im_paket},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_sl("gueltig", "drei Zeilen, sortiert, jede mit vier Feldern", SL)
fall_sl("gueltig-mit-ausnahme", "Ausdruck mit WITH", SL.replace("Zlib", "Apache-2.0 WITH LLVM-exception"))
fall_sl("form-leer", "leere Ausgabe", "", "stueckliste_form")
fall_sl("form-ohne-zeilenende", "letzte Zeile ohne LF", SL.rstrip("\n"), "stueckliste_form")
fall_sl("form-cr", "CRLF statt LF", SL.replace("\n", "\r\n"), "stueckliste_form")
fall_sl("form-nicht-ascii", "Zeichen außerhalb ASCII", SL.replace("Zlib", "Zlib\u00e4"), "stueckliste_form")
fall_sl("leer-feld-lizenz", "Lizenz leer", SL.replace("Zlib", ""), "stueckliste_leer_feld")
fall_sl("leer-feld-herkunft", "Herkunft leer", SL.replace("\tcrates.io\n", "\t\n", 1), "stueckliste_leer_feld")
fall_sl("leer-feld-drei-felder", "nur drei Felder", "anyhow\t1.0.100\tMIT\n", "stueckliste_leer_feld", im_paket=[])
fall_sl("leer-feld-fuenf-felder", "fünf Felder", "anyhow\t1.0.100\tMIT\tcrates.io\tx\n", "stueckliste_leer_feld", im_paket=[])
fall_sl("feld-form-lizenz", "Lizenz mit Semikolon", SL.replace("Zlib", "Zlib;Apache"), "stueckliste_feld_form")
fall_sl("feld-form-name", "Name mit Leerzeichen", SL.replace("anyhow", "any how"), "stueckliste_feld_form")
fall_sl("herkunft-git", "Herkunft ist ein Git-Link statt crates.io", SL.replace("zlib-rs\t0.5.2\tZlib\tcrates.io", "zlib-rs\t0.5.2\tZlib\tgit+https"), "stueckliste_herkunft")
fall_sl("gueltig-bwrap-kopie", "Form B: das `bubblewrap` aus dem Ubuntu-Archiv steht mit Lizenz und Herkunft `ubuntu-archiv` in der Liste",
        "anyhow\t1.0.100\tMIT OR Apache-2.0\tcrates.io\nbubblewrap\t0.11.0\tLGPL-2.0-or-later\tubuntu-archiv\nserde\t1.0.228\tMIT OR Apache-2.0\tcrates.io\n",
        im_paket=[{"name": "serde", "version": "1.0.228"}, {"name": "bubblewrap", "version": "0.11.0"}])
fall_sl("herkunft-ubuntu-fremder-name", "`ubuntu-archiv` gilt nur für `bubblewrap`", SL.replace("serde\t1.0.228\tMIT OR Apache-2.0\tcrates.io", "serde\t1.0.228\tMIT OR Apache-2.0\tubuntu-archiv"), "stueckliste_herkunft")
fall_sl("fehlt-bwrap-kopie", "die bwrap-Kopie ist im Paket, aber nicht in der Liste", SL, "stueckliste_fehlt",
        im_paket=IM_PAKET + [{"name": "bubblewrap", "version": "0.11.0"}])
fall_sl("verboten-webpki-roots", "`webpki-roots` bringt ein festes CA-Bündel mit; der System-Truststore gilt immer",
        SL.replace("zlib-rs\t0.5.2\tZlib", "webpki-roots\t1.0.0\tCDLA-Permissive-2.0"), "stueckliste_verboten",
        im_paket=[])
fall_sl("verboten-webpki-root-certs", "`webpki-root-certs` ebenso", SL.replace("zlib-rs\t0.5.2\tZlib", "webpki-root-certs\t1.0.0\tCDLA-Permissive-2.0"),
        "stueckliste_verboten", im_paket=[])
fall_sl("rustls-webpki-erlaubt", "`rustls-webpki` prüft Ketten, enthält aber keine Wurzeln: erlaubt",
        "rustls-webpki\t0.103.0\tISC\tcrates.io\n", im_paket=[])
fall_sl("doppelt", "dieselbe Bibliothek in derselben Version zweimal",
        "anyhow\t1.0.100\tMIT OR Apache-2.0\tcrates.io\nanyhow\t1.0.100\tMIT OR Apache-2.0\tcrates.io\n", "stueckliste_doppelt", im_paket=[])
fall_sl("reihenfolge", "serde vor anyhow", "serde\t1.0.228\tMIT OR Apache-2.0\tcrates.io\nanyhow\t1.0.100\tMIT OR Apache-2.0\tcrates.io\n", "stueckliste_reihenfolge", im_paket=[])
fall_sl("zwei-versionen", "dieselbe Bibliothek in zwei Versionen ist erlaubt, aufsteigend",
        "serde\t1.0.200\tMIT\tcrates.io\nserde\t1.0.228\tMIT\tcrates.io\n", im_paket=[{"name": "serde", "version": "1.0.200"}])
fall_sl("fehlt", "eine Bibliothek im Paket steht nicht in der Liste (A75)", SL, "stueckliste_fehlt",
        im_paket=IM_PAKET + [{"name": "tokio", "version": "1.40.0"}])
fall_sl("fehlt-version", "gleicher Name, andere Version als in der Liste", SL, "stueckliste_fehlt",
        im_paket=[{"name": "serde", "version": "1.0.229"}])

# ------------------------------------------------------------------------------------------ AppArmor-Profil (zwei Formen)
FORM_A = (
    "abi <abi/4.0>,\ninclude <tunables/global>\n\n"
    "profile werkbank-einschluss /usr/lib/werkbank/werkbank-einschluss {\n  include <abstractions/base>\n"
    "  /usr/lib/werkbank/werkbank-einschluss mr,\n  /proc/sys/dev/tty/legacy_tiocsti r,\n  /proc/sys/kernel/osrelease r,\n  /{**,} r,\n  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,\n}\n\n"
    "profile werkbank-einschluss-bwrap flags=(attach_disconnected,mediate_deleted) {\n  allow capability,\n  allow file rwlkm /{**,},\n"
    "  allow network,\n  allow unix,\n  allow ptrace,\n  allow signal,\n  allow userns,\n  allow mount,\n  allow umount,\n"
    "  allow pivot_root,\n  allow pix /** -> &werkbank-einschluss-bwrap//&werkbank-einschluss-kind,\n}\n\n"
    "profile werkbank-einschluss-kind flags=(attach_disconnected,mediate_deleted) {\n  allow file rwlkm /{**,},\n"
    "  allow network,\n  allow unix,\n  allow ptrace,\n  allow signal,\n  allow userns,\n  allow mount,\n  allow umount,\n"
    "  allow pivot_root,\n  allow pix /** -> &werkbank-einschluss-kind,\n  audit deny capability,\n}\n"
)
FORM_B = (
    "abi <abi/4.0>,\ninclude <tunables/global>\n\n"
    "profile werkbank-bwrap /usr/lib/werkbank/bwrap {\n  userns,\n}\n"
)
ALT_FEHLER = (
    "abi <abi/4.0>,\ninclude <tunables/global>\n\n"
    "profile werkbank-einschluss-bwrap /usr/bin/bwrap flags=(unconfined) {\n  userns,\n}\n"
)


def fall_profil(name, beschreibung, inhalt, code=None):
    fall("profil-" + name, {"beschreibung": beschreibung, "form": "profil", "text": inhalt},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_profil("gueltig-form-a", "Form A (V1): Anhang auf den festen Starter, Übergang in ein Kindprofil ohne Pfadanhang, `userns` nur in den Profilen ohne Anhang, kein `unconfined`", FORM_A)
fall_profil("gueltig-form-b", "Form B: eigene Kopie `/usr/lib/werkbank/bwrap` mit Anhang auf genau diesen Pfad", FORM_B)
fall_profil("system-bwrap-direkt", "das alte Profil mit Anhang `/usr/bin/bwrap`: Es gäbe jedem Aufrufer von bwrap einen Benutzer-Namensraum", ALT_FEHLER, "profil_system_bwrap")
fall_profil("system-bwrap-ordnerglob", "Anhang `/usr/bin/**` trifft auch das System-bwrap", FORM_B.replace("/usr/lib/werkbank/bwrap", "/usr/bin/**"), "profil_system_bwrap")
fall_profil("system-bwrap-namensglob", "Anhang `/usr/bin/bw*`", FORM_B.replace("/usr/lib/werkbank/bwrap", "/usr/bin/bw*"), "profil_system_bwrap")
fall_profil("system-bwrap-alles", "Anhang `/**`", FORM_B.replace("/usr/lib/werkbank/bwrap", "/**"), "profil_system_bwrap")
fall_profil("system-bwrap-menge", "Anhang `/usr/bin/{bwrap,x}`", FORM_B.replace("/usr/lib/werkbank/bwrap", "/usr/bin/{bwrap,x}"), "profil_system_bwrap")
fall_profil("system-bwrap-als-name", "ein Profil, dessen Name der Pfad `/usr/bin/bwrap` ist, hängt am System-bwrap", ALT_FEHLER.replace("werkbank-einschluss-bwrap /usr/bin/bwrap", "/usr/bin/bwrap"), "profil_system_bwrap")
fall_profil("userns-im-laeufer", "`userns` im Profil des Starters wäre für den ganzen Starter frei", FORM_A.replace("  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,\n", "  userns,\n  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,\n"), "profil_userns_unbegrenzt")
fall_profil("userns-allow-im-starter", "`allow userns,` im Profil des Starters zählt wie `userns,`", FORM_A.replace("  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,\n", "  allow userns,\n  /usr/bin/bwrap Px -> werkbank-einschluss-bwrap,\n"), "profil_userns_unbegrenzt")
fall_profil("unconfined-elternprofil", "das Elternprofil des Starters trägt `flags=(unconfined)` (früherer Entwurf): in keinem Paket erlaubt",
            FORM_A.replace("werkbank-einschluss {\n  include", "werkbank-einschluss flags=(unconfined) {\n  include"), "profil_unconfined")
fall_profil("unconfined-kindprofil", "das Kindprofil ohne Anhang trägt `flags=(attach_disconnected,unconfined)`",
            FORM_A.replace("profile werkbank-einschluss-kind flags=(attach_disconnected,mediate_deleted)", "profile werkbank-einschluss-kind flags=(attach_disconnected,unconfined)"), "profil_unconfined")
fall_profil("unconfined-ohne-userns", "ein Profil mit `unconfined` ohne `userns`: Stufe 4 greift allein",
            "abi <abi/4.0>,\ninclude <tunables/global>\n\nprofile werkbank-x /usr/lib/werkbank/x flags=(unconfined) {\n  /usr/lib/werkbank/x mr,\n}\n", "profil_unconfined")
fall_profil("unconfined-form-b", "Form B mit `flags=(unconfined)` (früherer Entwurf)", FORM_B.replace("/usr/lib/werkbank/bwrap {", "/usr/lib/werkbank/bwrap flags=(unconfined) {"), "profil_unconfined")
fall_profil("reihenfolge-userns-vor-unconfined", "Anhang an anderem Pfad mit `userns` und `unconfined`: `profil_userns_unbegrenzt` zuerst",
            FORM_B.replace("/usr/lib/werkbank/bwrap {", "/usr/lib/werkbank/anderes flags=(unconfined) {"), "profil_userns_unbegrenzt")
fall_profil("userns-fremder-anhang", "`userns` in einem Profil mit einem anderen Anhang als der Kopie", FORM_B.replace("/usr/lib/werkbank/bwrap", "/usr/lib/werkbank/anderes"), "profil_userns_unbegrenzt")
fall_profil("form-leer", "leere Datei", "", "profil_form")
fall_profil("form-ohne-profil", "nur Kopfzeilen, kein Profil", "abi <abi/4.0>,\ninclude <tunables/global>\n", "profil_form")
fall_profil("form-klammer", "eine Klammer fehlt", FORM_B.rstrip("\n").rstrip("}") + "\n", "profil_form")
fall_profil("form-nicht-ascii", "Zeichen außerhalb ASCII", FORM_B.replace("userns,", "userns, # \u00e4"), "profil_form")
fall_profil("reihenfolge-system-bwrap-vor-userns", "Anhang am System-bwrap und `userns`: `profil_system_bwrap` zuerst", ALT_FEHLER, "profil_system_bwrap")

# ------------------------------------------------------------------------------------------ Steuerdatei des deb
STEUER = (
    "Package: werkbank\nVersion: 0.1.0\nArchitecture: amd64\nMaintainer: werkbank-paket <paket@example.invalid>\n"
    "Depends: libc6 (>= 2.40), libssl3t64 (>= 3.0.0), python3, bubblewrap, apparmor, ca-certificates, systemd\n"
    "Description: Werkbank, Probelauf\n Arbeitsplatz fuer Agenten mit Einschluss und Tor.\n"
)


def fall_steuer(name, beschreibung, inhalt, code=None, version="0.1.0", geruest=None):
    eintrag = {"beschreibung": beschreibung, "form": "steuerdatei", "text": inhalt, "manifest_version": version}
    if geruest is not None:
        eintrag["geruest"] = geruest
    fall("steuerdatei-" + name, eintrag,
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_steuer("gueltig", "Steuerdatei des deb: Paket werkbank, amd64, die sechs Abhängigkeiten", STEUER)
fall_steuer("gueltig-geruest", "Gerüstbau (`--geruest`): die Version trägt sichtbar `~geruest` hinter der Version des Manifests",
            STEUER.replace("Version: 0.1.0", "Version: 0.1.0~geruest"), geruest=True)
fall_steuer("version-geruest-fehlt", "Gerüstbau, aber die Version trägt `~geruest` nicht", STEUER, "steuer_wert", geruest=True)
fall_steuer("version-geruest-im-auslieferungsbau", "Auslieferungsbau mit `~geruest` in der Version: ein Gerüst wäre nicht zu erkennen als Auslieferung",
            STEUER.replace("Version: 0.1.0", "Version: 0.1.0~geruest"), "steuer_wert")
fall_steuer("version-geruest-falsche-schreibweise", "Gerüstbau mit `~Geruest` statt `~geruest`", STEUER.replace("Version: 0.1.0", "Version: 0.1.0~Geruest"), "steuer_wert", geruest=True)
fall_steuer("version-geruest-plus", "Gerüstbau mit `+geruest` statt `~geruest` (`+` sortiert nach der Auslieferung, `~` davor)",
            STEUER.replace("Version: 0.1.0", "Version: 0.1.0+geruest"), "steuer_wert", geruest=True)
fall_steuer("gueltig-mit-alternative", "eine Abhängigkeit mit Alternative und Versionsangabe", STEUER.replace("bubblewrap,", "bubblewrap (>= 0.8) | bwrap-ersatz,"))
fall_steuer("form-ohne-zeilenende", "letzte Zeile ohne LF", STEUER.rstrip("\n"), "steuer_form")
fall_steuer("form-doppeltes-feld", "`Depends` zweimal", STEUER + "Depends: x\n", "steuer_form")
fall_steuer("form-kein-doppelpunkt", "Zeile ohne Doppelpunkt", STEUER + "Fehler\n", "steuer_form")
fall_steuer("form-nicht-ascii", "Zeichen außerhalb ASCII", STEUER.replace("Probelauf", "Probel\u00e4uf"), "steuer_form")
fall_steuer("feld-fehlt-depends", "`Depends` fehlt", STEUER.replace("Depends: libc6 (>= 2.40), libssl3t64 (>= 3.0.0), python3, bubblewrap, apparmor, ca-certificates, systemd\n", ""), "steuer_feld_fehlt")
fall_steuer("feld-leer-description", "`Description` leer", STEUER.replace("Description: Werkbank, Probelauf\n Arbeitsplatz fuer Agenten mit Einschluss und Tor.\n", "Description:\n"), "steuer_feld_fehlt")
fall_steuer("wert-architektur", "Architektur arm64", STEUER.replace("amd64", "arm64"), "steuer_wert")
fall_steuer("wert-paketname", "Paketname anders", STEUER.replace("Package: werkbank", "Package: wb"), "steuer_wert")
fall_steuer("wert-version", "Version weicht vom Manifest ab", STEUER, "steuer_wert", version="0.1.1")
fall_steuer("abhaengigkeit-ca-certificates", "ohne `ca-certificates` (System-Truststore)", STEUER.replace(" ca-certificates,", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-python3", "ohne `python3` (der Läufer ist ein Python-Skript)", STEUER.replace(" python3,", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-bubblewrap", "ohne `bubblewrap`", STEUER.replace(" bubblewrap,", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-apparmor", "ohne `apparmor`", STEUER.replace(" apparmor,", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-systemd", "ohne `systemd`", STEUER.replace(", systemd", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-libssl3t64", "ohne `libssl3t64` (`werkbank-modellweg` bindet libssl.so.3 und libcrypto.so.3)",
            STEUER.replace(" libssl3t64 (>= 3.0.0),", ""), "steuer_abhaengigkeit")
fall_steuer("abhaengigkeit-libssl3-ohne-t64", "`libssl3` statt `libssl3t64`: der Name stimmt nicht", STEUER.replace("libssl3t64", "libssl3"), "steuer_abhaengigkeit")
fall_steuer("gueltig-libssl3t64-ohne-version", "`libssl3t64` ohne Versionsangabe", STEUER.replace("libssl3t64 (>= 3.0.0)", "libssl3t64"))
fall_steuer("gueltig-libssl3t64-mit-alternative", "`libssl3t64` mit Alternative", STEUER.replace("libssl3t64 (>= 3.0.0)", "libssl3t64 (>= 3.0.0) | libssl3t64-ersatz"))
fall_steuer("reihenfolge-wert-vor-abhaengigkeit", "falsche Architektur und fehlende Abhängigkeit: `steuer_wert` zuerst", STEUER.replace("amd64", "arm64").replace(" apparmor,", ""), "steuer_wert")

# ------------------------------------------------------------------------------------------ Prüfbefehl
LAGE = {
    "plattform": "windows",
    "konten": 1,
    "halter": "ok",
    "wfp": "ok",
    "einstellungen": {"zustand": "ok"},
    "vault": "ok",
    "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []},
    "stelle": "ok",
}
OK_ZEILE = "pruef.ok: Alle 5 Pruefungen bestanden."
STELLE_ZEILE = "pruef.stelle_einrichten: Installation fertig, Stelle einrichten. Befehl: wb stelle einrichten"


def lage(aendern=None):
    obj = copy.deepcopy(LAGE)
    if aendern:
        aendern(obj)
    return obj


def fall_pruef(name, beschreibung, rc, zeilen, lage_obj=None, aufruf=None, strom="stdout"):
    fall("pruefung-" + name,
         {"beschreibung": beschreibung, "form": "pruefung", "aufruf": aufruf or [], "lage": lage_obj or lage()},
         {"rc": rc, "strom": strom, "zeilen": zeilen})


def rot(zeilen):
    return zeilen + [f"pruef.rot: {len(zeilen)} Befunde."]


fall_pruef("gruen", "alles in Ordnung, ein Konto", 0, [OK_ZEILE])
fall_pruef("gruen-stelle-fehlt-ein-konto", "A108 Gegenprobe 1: Stelle fehlt, aber nur ein Konto: Code 0", 0, [OK_ZEILE],
           lage(lambda o: o.update(stelle="fehlt", konten=1)))
fall_pruef("gruen-mehrere-konten-stelle-da", "mehrere Konten, Stelle da: Code 0", 0, [OK_ZEILE],
           lage(lambda o: o.update(konten=3)))
fall_pruef("stelle-einrichten", "A108: nur die Stelle fehlt, drei Konten: eigener Code 20, ein Satz als letzte Zeile", 20,
           [STELLE_ZEILE], lage(lambda o: o.update(stelle="fehlt", konten=3)))
fall_pruef("stelle-einrichten-zwei-konten", "A108 an der Grenze: zwei Konten", 20, [STELLE_ZEILE],
           lage(lambda o: o.update(stelle="fehlt", konten=2)))
fall_pruef("stelle-einrichten-nicht-bei-rotem-befund", "A108 Gegenprobe 2: ein echter Fehler gibt Code 1, die Stellenzeile entfällt", 1,
           rot(["pruef.halter_fehlt: Der Dienst wb-halter ist nicht eingetragen."]),
           lage(lambda o: o.update(stelle="fehlt", konten=3, halter="fehlt")))
fall_pruef("halter-fehlt", "Dienst wb-halter nicht eingetragen", 1,
           rot(["pruef.halter_fehlt: Der Dienst wb-halter ist nicht eingetragen."]), lage(lambda o: o.update(halter="fehlt")))
fall_pruef("halter-konto", "Dienst läuft unter einem anderen Konto", 1,
           rot(["pruef.halter_konto: Der Dienst wb-halter laeuft nicht unter dem Konto NT SERVICE\\wb-halter."]),
           lage(lambda o: o.update(halter="konto")))
fall_pruef("halter-gestoppt", "Dienst nicht gestartet", 1,
           rot(["pruef.halter_gestoppt: Der Dienst wb-halter ist nicht gestartet."]), lage(lambda o: o.update(halter="gestoppt")))
fall_pruef("halter-nicht-pruefbar", "Prüfung ohne das nötige Recht: rot, nie still grün (fail closed)", 1,
           rot(["pruef.halter_nicht_pruefbar: Der Dienst wb-halter war nicht pruefbar."]), lage(lambda o: o.update(halter="nicht_pruefbar")))
fall_pruef("wfp-dienst-fehlt", "Filterdienst läuft nicht", 1,
           rot(["pruef.wfp_dienst_fehlt: Der Filterdienst wb-plattform laeuft nicht."]), lage(lambda o: o.update(wfp="dienst_fehlt")))
fall_pruef("wfp-filter-fehlt", "Filtergruppe fehlt", 1,
           rot(["pruef.wfp_filter_fehlt: Die Filtergruppe der Werkbank fehlt in der Filterplattform."]), lage(lambda o: o.update(wfp="filter_fehlt")))
fall_pruef("wfp-sperre-fehlt", "Sperre eines Containers fehlt", 1,
           rot(["pruef.wfp_sperre_fehlt: Fuer einen Agentencontainer fehlt die Sperre fuer 127.0.0.0/8 und ::1."]),
           lage(lambda o: o.update(wfp="sperre_fehlt")))
fall_pruef("wfp-nicht-pruefbar", "Filter nicht lesbar (Recht fehlt)", 1,
           rot(["pruef.wfp_nicht_pruefbar: Die Filter der Werkbank waren nicht pruefbar."]), lage(lambda o: o.update(wfp="nicht_pruefbar")))
fall_pruef("einstellungen-fehlt", "Einstellungsdatei fehlt", 1,
           rot(["pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar."]),
           lage(lambda o: o.update(einstellungen={"zustand": "fehlt"})))
fall_pruef("einstellungen-ungueltig", "Einstellungsdatei ungültig, der Code aus Vertrag 2 steht in der Zeile", 1,
           rot(["pruef.einstellungen_ungueltig: Die Einstellungsdatei ist ungueltig (einst.anzahl)."]),
           lage(lambda o: o.update(einstellungen={"zustand": "ungueltig", "code": "einst.anzahl"})))
fall_pruef("einstellungen-nicht-pruefbar", "Einstellungsdatei nicht prüfbar", 1,
           rot(["pruef.einstellungen_nicht_pruefbar: Die Einstellungsdatei war nicht pruefbar."]),
           lage(lambda o: o.update(einstellungen={"zustand": "nicht_pruefbar"})))
fall_pruef("vault-fehlt", "Vault-Ordner fehlt", 1, rot(["pruef.vault_fehlt: Der Vault-Ordner fehlt."]), lage(lambda o: o.update(vault="fehlt")))
fall_pruef("vault-kein-ordner", "Vault-Pfad ist eine Datei", 1, rot(["pruef.vault_kein_ordner: Der Vault-Pfad ist kein Ordner."]),
           lage(lambda o: o.update(vault="kein_ordner")))
fall_pruef("vault-rechte", "Vault-Ordner mit offenen Rechten", 1, rot(["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte."]),
           lage(lambda o: o.update(vault="rechte")))
fall_pruef("vault-nicht-pruefbar", "Vault-Ordner nicht prüfbar", 1, rot(["pruef.vault_nicht_pruefbar: Der Vault-Ordner war nicht pruefbar."]),
           lage(lambda o: o.update(vault="nicht_pruefbar")))


def paketstand(**kw):
    def aendern(o):
        o["paketstand"].update(kw)
    return lage(aendern)


fall_pruef("paketstand-manifest-fehlt", "Manifest fehlt; Dateien werden dann nicht geprüft", 1,
           rot(["pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt."]),
           paketstand(manifest="fehlt", dateien=[{"pfad": "bin/wb.exe", "art": "fehlt"}]))
fall_pruef("paketstand-manifest-ungueltig", "Manifest ungültig nach diesem Vertrag", 1,
           rot(["pruef.paketstand_manifest_ungueltig: Das Manifest des Pakets ist ungueltig."]), paketstand(manifest="ungueltig"))
fall_pruef("paketstand-nicht-pruefbar", "Paketstand nicht prüfbar", 1,
           rot(["pruef.paketstand_nicht_pruefbar: Der Paketstand war nicht pruefbar."]), paketstand(manifest="nicht_pruefbar"))
fall_pruef("paketstand-testbau", "installiertes Paket ist ein Testbau", 1,
           rot(["pruef.paketstand_testbau: Das Paket ist ein Testbau."]), paketstand(bau="test"))
fall_pruef("paketstand-version", "installierte Version gleicht dem Manifest nicht", 1,
           rot(["pruef.paketstand_version: Die installierte Version weicht vom Manifest ab."]), paketstand(version="abweichend"))
fall_pruef("paketstand-datei-fehlt", "eine Datei fehlt", 1,
           rot(["pruef.paketstand_datei_fehlt: Die Datei bin/wb.exe fehlt."]), paketstand(dateien=[{"pfad": "bin/wb.exe", "art": "fehlt"}]))
fall_pruef("paketstand-datei-abweichend", "eine Datei hat eine andere Prüfsumme", 1,
           rot(["pruef.paketstand_datei_abweichend: Die Datei bin/wb.exe weicht vom Manifest ab."]),
           paketstand(dateien=[{"pfad": "bin/wb.exe", "art": "abweichend"}]))
fall_pruef("paketstand-datei-fremd", "eine Datei steht nicht im Manifest", 1,
           rot(["pruef.paketstand_datei_fremd: Die Datei bin/fremd.dll steht nicht im Manifest."]),
           paketstand(dateien=[{"pfad": "bin/fremd.dll", "art": "fremd"}]))
fall_pruef("paketstand-sortiert", "Dateizeilen nach Pfad, dann nach Art (fehlt, abweichend, fremd), egal in welcher Reihenfolge sie anfallen", 1,
           rot(["pruef.paketstand_datei_fehlt: Die Datei bin/a.exe fehlt.",
                "pruef.paketstand_datei_fremd: Die Datei bin/a.exe steht nicht im Manifest.",
                "pruef.paketstand_datei_abweichend: Die Datei bin/b.exe weicht vom Manifest ab."]),
           paketstand(dateien=[{"pfad": "bin/b.exe", "art": "abweichend"}, {"pfad": "bin/a.exe", "art": "fremd"},
                               {"pfad": "bin/a.exe", "art": "fehlt"}]))
fall_pruef("paketstand-genau-20", "genau 20 abweichende Dateien: 20 Zeilen, keine Sammelzeile", 1,
           rot([f"pruef.paketstand_datei_fehlt: Die Datei daten/d{i:02d}.bin fehlt." for i in range(20)]),
           paketstand(dateien=[{"pfad": f"daten/d{i:02d}.bin", "art": "fehlt"} for i in range(20)]))
fall_pruef("paketstand-22-dateien", "22 abweichende Dateien: 20 Zeilen, dann `weitere`", 1,
           rot([f"pruef.paketstand_datei_fehlt: Die Datei daten/d{i:02d}.bin fehlt." for i in range(20)]
               + ["pruef.paketstand_weitere: 2 weitere Dateien weichen ab."]),
           paketstand(dateien=[{"pfad": f"daten/d{i:02d}.bin", "art": "fehlt"} for i in range(22)]))
fall_pruef("paketstand-version-und-datei", "Reihenfolge innerhalb des Paketstands: Testbau, Version, Dateien", 1,
           rot(["pruef.paketstand_testbau: Das Paket ist ein Testbau.",
                "pruef.paketstand_version: Die installierte Version weicht vom Manifest ab.",
                "pruef.paketstand_datei_fehlt: Die Datei bin/wb.exe fehlt."]),
           paketstand(bau="test", version="abweichend", dateien=[{"pfad": "bin/wb.exe", "art": "fehlt"}]))
fall_pruef("alles-rot", "jede Prüfung rot: je ein Satz in fester Reihenfolge, dann die Summe", 1,
           rot(["pruef.halter_fehlt: Der Dienst wb-halter ist nicht eingetragen.",
                "pruef.wfp_dienst_fehlt: Der Filterdienst wb-plattform laeuft nicht.",
                "pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar.",
                "pruef.vault_fehlt: Der Vault-Ordner fehlt.",
                "pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt."]),
           lage(lambda o: o.update(halter="fehlt", wfp="dienst_fehlt", einstellungen={"zustand": "fehlt"}, vault="fehlt",
                                   paketstand={"manifest": "fehlt", "bau": "auslieferung", "version": "gleich", "dateien": []})))
fall_pruef("aufruf-argument", "ein Argument: Code 64, eine Zeile auf stderr", 64,
           ["pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht."], aufruf=["--alles"], strom="stderr")


LAGE_LINUX = {
    "plattform": "linux",
    "konten": 1,
    "dienste": [{"name": "werkbank-kern.service", "zustand": "ok"}],
    "apparmor": "ok",
    "einstellungen": {"zustand": "ok"},
    "nutzerordner": "ok",
    "schluessel": "ok",
    "copilot": "ok",
    "vault": "ok",
    "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []},
    "stelle": "ok",
}
OK7 = "pruef.ok: Alle 8 Pruefungen bestanden."


def lx(aendern=None):
    obj = copy.deepcopy(LAGE_LINUX)
    if aendern:
        aendern(obj)
    return obj


def dienst(kern=None):
    def a(o):
        if kern:
            o["dienste"][0]["zustand"] = kern
    return lx(a)


fall_pruef("linux-gruen", "Ubuntu: alle acht Prüfungen bestanden", 0, [OK7], lx())
fall_pruef("linux-stelle-einrichten", "A108 unter Linux: nur die Stelle fehlt, zwei Konten: rc 20", 20, [STELLE_ZEILE],
           lx(lambda o: o.update(stelle="fehlt", konten=2)))
fall_pruef("linux-stelle-ein-konto", "A108 Gegenprobe 1 unter Linux: ein Konto, rc 0", 0, [OK7], lx(lambda o: o.update(stelle="fehlt")))
fall_pruef("linux-stelle-nicht-bei-befund", "A108 Gegenprobe 2 unter Linux: ein Befund gibt rc 1 ohne Stellenzeile", 1,
           rot(["pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen."]),
           lx(lambda o: o.update(stelle="fehlt", konten=3, apparmor="nicht_geladen")))
def bruecke(zustand, kern=None):
    def a(o):
        o["dienste"].append({"name": "werkbank-bruecke.service", "zustand": zustand})
        if kern:
            o["dienste"][0]["zustand"] = kern
    return lx(a)


fall_pruef("linux-dienst-bruecke-gruen", "beide Einheiten ok (die Brücke ist eingetragen und läuft)", 0, [OK7], bruecke("ok"))
fall_pruef("linux-dienst-bruecke-fehlt", "werkbank-bruecke.service nicht eingetragen", 1,
           rot(["pruef.dienst_fehlt: Der Dienst werkbank-bruecke.service ist nicht eingetragen."]), bruecke("fehlt"))
fall_pruef("linux-dienst-bruecke-inaktiv", "werkbank-bruecke.service nicht aktiv", 1,
           rot(["pruef.dienst_inaktiv: Der Dienst werkbank-bruecke.service ist nicht aktiv."]), bruecke("inaktiv"))
fall_pruef("linux-dienst-bruecke-fehlgeschlagen", "werkbank-bruecke.service fehlgeschlagen", 1,
           rot(["pruef.dienst_fehlgeschlagen: Der Dienst werkbank-bruecke.service ist fehlgeschlagen."]), bruecke("fehlgeschlagen"))
fall_pruef("linux-dienst-bruecke-nicht-pruefbar", "werkbank-bruecke.service war nicht pruefbar", 1,
           rot(["pruef.dienst_nicht_pruefbar: Der Dienst werkbank-bruecke.service war nicht pruefbar."]), bruecke("nicht_pruefbar"))
fall_pruef("linux-dienst-kern-und-bruecke-fehlen", "beide Einheiten fehlen: zwei Zeilen, erst Kern, dann Brücke", 1,
           rot(["pruef.dienst_fehlt: Der Dienst werkbank-kern.service ist nicht eingetragen.",
                "pruef.dienst_fehlt: Der Dienst werkbank-bruecke.service ist nicht eingetragen."]), bruecke("fehlt", kern="fehlt"))
fall_pruef("linux-dienst-kern-fehlt", "werkbank-kern.service nicht eingetragen", 1,
           rot(["pruef.dienst_fehlt: Der Dienst werkbank-kern.service ist nicht eingetragen."]), dienst(kern="fehlt"))
fall_pruef("linux-dienst-kern-inaktiv", "werkbank-kern.service nicht aktiv", 1,
           rot(["pruef.dienst_inaktiv: Der Dienst werkbank-kern.service ist nicht aktiv."]), dienst(kern="inaktiv"))
fall_pruef("linux-dienst-kern-fehlgeschlagen", "werkbank-kern.service fehlgeschlagen", 1,
           rot(["pruef.dienst_fehlgeschlagen: Der Dienst werkbank-kern.service ist fehlgeschlagen."]), dienst(kern="fehlgeschlagen"))
fall_pruef("linux-dienst-nicht-pruefbar", "Benutzerdienste nicht abfragbar (etwa als root ohne Nutzer): rot", 1,
           rot(["pruef.dienst_nicht_pruefbar: Der Dienst werkbank-kern.service war nicht pruefbar."]), dienst(kern="nicht_pruefbar"))
fall_pruef("linux-dienst-beide", "beide Dienste rot: zwei Zeilen in fester Reihenfolge", 1,
           rot(["pruef.dienst_inaktiv: Der Dienst werkbank-kern.service ist nicht aktiv.",
                "pruef.dienst_fehlgeschlagen: Der Dienst werkbank-bruecke.service ist fehlgeschlagen."]),
           bruecke("fehlgeschlagen", kern="inaktiv"))
fall_pruef("linux-schluessel-fehlt-dienste-ok", "Kern und Brücke laufen, die Schlüsseldatei des Modellwegs fehlt (keine Einheit des Modellwegs im Paket): nur diese Zeile, rc 1", 1,
           rot(["pruef.schluessel_fehlt: Die Schluesseldatei fehlt."]), bruecke("ok", kern="ok") | {"schluessel": "fehlt"})
fall_pruef("linux-nur-copilot-anmeldung-fehlt", "Kern und Brücke laufen, die Schlüsseldatei ist da, nur die Anmeldung der Copilot CLI fehlt (frischer Probelauf ohne Anmeldung): nur diese Zeile, rc 1", 1,
           rot(["pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt."]), bruecke("ok", kern="ok") | {"copilot": "fehlt"})
for z, satz in (("bwrap_fehlt", "pruef.apparmor_bwrap_fehlt: Das Programm /usr/bin/bwrap fehlt."),
                ("datei_fehlt", "pruef.apparmor_datei_fehlt: Die Datei des AppArmor-Profils fehlt."),
                ("nicht_geladen", "pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen."),
                ("abweichend", "pruef.apparmor_abweichend: Das AppArmor-Profil weicht vom Paket ab."),
                ("fremd", "pruef.apparmor_fremd: Ein anderes AppArmor-Profil gilt fuer /usr/bin/bwrap."),
                ("probelauf", "pruef.apparmor_probelauf: Der Probelauf des Einschlusses scheitert."),
                ("system_bwrap_frei", "pruef.apparmor_system_bwrap_frei: Das System-bwrap ist fuer jeden Nutzer freigegeben."),
                ("unconfined", "pruef.apparmor_unconfined: Ein Profil der Werkbank ist ohne Einschraenkung geladen."),
                ("kopie_abweichend", "pruef.apparmor_kopie_abweichend: Die bwrap-Kopie des Produkts weicht vom System-bwrap ab."),
                ("nicht_pruefbar", "pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.")):
    fall_pruef("linux-apparmor-" + z.replace("_", "-"), f"AppArmor-Zustand {z}", 1, rot([satz]), lx(lambda o, z=z: o.update(apparmor=z)))
for z, satz in (("fehlt", "pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt."),
                ("rechte", "pruef.nutzerordner_rechte: Der Ordner des Nutzers hat falsche Rechte oder einen falschen Eigentuemer."),
                ("nicht_pruefbar", "pruef.nutzerordner_nicht_pruefbar: Der Ordner des Nutzers war nicht pruefbar.")):
    fall_pruef("linux-nutzerordner-" + z.replace("_", "-"), f"Ordner `/var/lib/werkbank/<nutzer>/` im Zustand {z}", 1, rot([satz]),
               lx(lambda o, z=z: o.update(nutzerordner=z)))
for z, satz in (("fehlt", "pruef.schluessel_fehlt: Die Schluesseldatei fehlt."),
                ("art", "pruef.schluessel_art: Die Schluesseldatei ist keine gewoehnliche Datei."),
                ("eigentuemer", "pruef.schluessel_eigentuemer: Die Schluesseldatei gehoert nicht dem Nutzer."),
                ("modus", "pruef.schluessel_modus: Die Schluesseldatei hat nicht den Modus 0600."),
                ("leer", "pruef.schluessel_leer: Die Schluesseldatei ist leer."),
                ("nicht_pruefbar", "pruef.schluessel_nicht_pruefbar: Die Schluesseldatei war nicht pruefbar.")):
    fall_pruef("linux-schluessel-" + z.replace("_", "-"), f"Schlüsseldatei im Zustand {z}; die Zeile nennt weder Pfad noch Inhalt", 1,
               rot([satz]), lx(lambda o, z=z: o.update(schluessel=z)))
for z, satz in (("fehlt", "pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt."),
                ("art", "pruef.copilot_art: Die Anmeldedatei der Copilot CLI ist keine gewoehnliche Datei."),
                ("eigentuemer", "pruef.copilot_eigentuemer: Die Anmeldedatei der Copilot CLI gehoert nicht dem Nutzer."),
                ("modus", "pruef.copilot_modus: Die Anmeldedatei der Copilot CLI hat nicht den Modus 0600."),
                ("leer", "pruef.copilot_leer: Die Anmeldedatei der Copilot CLI ist leer."),
                ("form", "pruef.copilot_form: Die Anmeldedatei der Copilot CLI hat falsche Form."),
                ("nicht_pruefbar", "pruef.copilot_nicht_pruefbar: Die Anmeldedatei der Copilot CLI war nicht pruefbar.")):
    fall_pruef("linux-copilot-" + z.replace("_", "-"), f"Anmeldedatei der Copilot CLI im Zustand {z}; die Zeile nennt weder Pfad noch Länge noch Inhalt", 1,
               rot([satz]), lx(lambda o, z=z: o.update(copilot=z)))
HINWEIS_UBUNTU = ("pruef.hinweis_system_bwrap_ubuntu: Das System-bwrap ist durch das Ubuntu-Profil frei; "
                  "die Firma kann dieses Profil entfernen.")
fall_pruef("linux-system-bwrap-ubuntu", "das System-bwrap ist nur durch das Ubuntu-Profil frei (Label `bwrap`, `unpriv_bwrap`): ein Hinweis, kein Befund, rc 0", 0,
           [HINWEIS_UBUNTU, OK7], lx(lambda o: o.update(system_bwrap="ubuntu")))
fall_pruef("linux-system-bwrap-ubuntu-mit-befund", "Hinweis und ein echter Befund: der Hinweis steht vor `pruef.rot` und zählt dort nicht mit", 1,
           ["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte.", HINWEIS_UBUNTU, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(system_bwrap="ubuntu", vault="rechte")))
fall_pruef("linux-system-bwrap-ubuntu-stelle-einrichten", "Hinweis und A108: der Hinweis steht vor der Stellenzeile, die Stellenzeile bleibt die letzte, rc 20", 20,
           [HINWEIS_UBUNTU, STELLE_ZEILE], lx(lambda o: o.update(system_bwrap="ubuntu", stelle="fehlt", konten=2)))
fall_pruef("linux-system-bwrap-gesperrt-ausdruecklich", "`system_bwrap: gesperrt` ist die Vorgabe: kein Hinweis", 0, [OK7], lx(lambda o: o.update(system_bwrap="gesperrt")))
fall_pruef("linux-system-bwrap-ubuntu-und-frei", "Hinweis und der rote Zustand `system_bwrap_frei` (Freigabe von einem Profil der Werkbank oder ohne Profil) zusammen: ein Befund", 1,
           ["pruef.apparmor_system_bwrap_frei: Das System-bwrap ist fuer jeden Nutzer freigegeben.", HINWEIS_UBUNTU, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(system_bwrap="ubuntu", apparmor="system_bwrap_frei")))
UBUNTU_GELADEN = "pruef.hinweis_ubuntu_profil_geladen: Das Ubuntu-Profil bwrap-userns-restrict ist geladen."
UBUNTU_ENTLADEN = "pruef.hinweis_ubuntu_profil_entladen: Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen."
fall_pruef("linux-ubuntu-profil-geladen-s1", "S1 (Ubuntus Profil `bwrap-userns-restrict` geladen, Auslieferung): der Einschluss läuft, zwei Hinweiszeilen (Zustand, dann System-bwrap), rc 0", 0,
           [UBUNTU_GELADEN, HINWEIS_UBUNTU, OK7], lx(lambda o: o.update(ubuntu_profil="geladen", system_bwrap="ubuntu")))
fall_pruef("linux-ubuntu-profil-entladen-s0", "S0 (Ubuntus Profil entladen): der Einschluss läuft nur mit unserem Profil; eine Hinweiszeile (Zustand), das System-bwrap bleibt gesperrt, rc 0", 0,
           [UBUNTU_ENTLADEN, OK7], lx(lambda o: o.update(ubuntu_profil="entladen", system_bwrap="gesperrt")))
fall_pruef("linux-ubuntu-profil-geladen-bwrap-gesperrt", "Ubuntus Profil ist geladen, das System-bwrap ist aber gesperrt (Probe scheiterte): nur die Zustandszeile", 0,
           [UBUNTU_GELADEN, OK7], lx(lambda o: o.update(ubuntu_profil="geladen")))
fall_pruef("linux-ubuntu-profil-entladen-mit-befund", "der Zustand steht vor `pruef.rot` und zählt dort nicht mit", 1,
           ["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte.", UBUNTU_ENTLADEN, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ubuntu_profil="entladen", vault="rechte")))
fall_pruef("linux-ubuntu-profil-geladen-stelle-einrichten", "Zustand und Hinweis stehen vor der Stellenzeile, sie bleibt die letzte, rc 20", 20,
           [UBUNTU_GELADEN, HINWEIS_UBUNTU, STELLE_ZEILE],
           lx(lambda o: o.update(ubuntu_profil="geladen", system_bwrap="ubuntu", stelle="fehlt", konten=2)))
fall_pruef("linux-ubuntu-profil-ohne-feld", "ohne das Feld `ubuntu_profil` (Sperre aus oder Liste der Profile nicht lesbar) steht keine Zeile da", 0, [OK7], lx())
HINWEIS_SUDO = "pruef.hinweis_sudo: Ohne root waren nicht alle Pruefungen moeglich; fuer diese Pruefungen mit sudo aufrufen."
fall_pruef("linux-ohne-root-gruen", "Aufruf ohne root, alles prüfbar und in Ordnung: kein Hinweis, rc 0", 0, [OK7], lx(lambda o: o.update(ohne_root=True)))
fall_pruef("linux-ohne-root-nicht-pruefbar-hinweis", "Aufruf ohne root: AppArmor und Dienste nicht prüfbar. Der Hinweis steht nach den Befunden, vor `pruef.rot`, und zählt dort nicht mit", 1,
           ["pruef.dienst_nicht_pruefbar: Der Dienst werkbank-kern.service war nicht pruefbar.",
            "pruef.dienst_nicht_pruefbar: Der Dienst werkbank-bruecke.service war nicht pruefbar.",
            "pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.", HINWEIS_SUDO, "pruef.rot: 3 Befunde."],
           lx(lambda o: (o.update(ohne_root=True, apparmor="nicht_pruefbar"), o["dienste"][0].update(zustand="nicht_pruefbar"),
                         o["dienste"].append({"name": "werkbank-bruecke.service", "zustand": "nicht_pruefbar"}))))
fall_pruef("linux-ohne-root-nur-apparmor-nicht-pruefbar", "Aufruf ohne root: nur AppArmor nicht prüfbar (Liste der Profile nicht lesbar), alles andere ok: ein Befund und der Hinweis", 1,
           ["pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.", HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, apparmor="nicht_pruefbar")))
fall_pruef("linux-ohne-root-befund-ohne-nicht-pruefbar", "Aufruf ohne root, ein echter Befund, nichts nicht prüfbar: kein Hinweis", 1,
           rot(["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte."]), lx(lambda o: o.update(ohne_root=True, vault="rechte")))
fall_pruef("linux-ohne-root-copilot-ok-keine-copilot-zeile", "Aufruf ohne root, die Anmeldedatei ist lesbar und in Ordnung: keine Zeile der Prüfung `copilot`", 0, [OK7],
           lx(lambda o: o.update(ohne_root=True, copilot="ok")))
fall_pruef("linux-mit-root-nicht-pruefbar-ohne-hinweis", "Aufruf mit root (`ohne_root` fehlt oder falsch): `sudo` hülfe nicht, kein Hinweis", 1,
           rot(["pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar."]), lx(lambda o: o.update(apparmor="nicht_pruefbar")))
fall_pruef("linux-ohne-root-ubuntu-profil-geladen", "Aufruf ohne root: die Liste der Profile ist nicht lesbar, Ubuntus Profil gilt nach der Probe des System-bwrap als geladen (S1); unsere Labels bleiben nicht prüfbar", 1,
           ["pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.", UBUNTU_GELADEN, HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, apparmor="nicht_pruefbar", ubuntu_profil="geladen")))
fall_pruef("linux-ohne-root-ubuntu-profil-entladen", "Aufruf ohne root: die Probe des System-bwrap scheitert mit `uid map: Permission denied`: Ubuntus Profil gilt als nicht geladen (S0)", 1,
           ["pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.", UBUNTU_ENTLADEN, HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, apparmor="nicht_pruefbar", ubuntu_profil="entladen")))
fall_pruef("linux-ohne-root-ubuntu-profil-unbekannt", "Aufruf ohne root: die Probe sagt nichts (anderes Label, anderer Fehler): keine Zeile zu Ubuntus Profil", 1,
           ["pruef.apparmor_nicht_pruefbar: Das AppArmor-Profil war nicht pruefbar.", HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, apparmor="nicht_pruefbar")))
fall_pruef("linux-ohne-root-hinweis-nach-ubuntu-hinweisen", "die Reihenfolge der Hinweise: Zustand von Ubuntus Profil, System-bwrap, dann `hinweis_sudo`", 1,
           ["pruef.vault_nicht_pruefbar: Der Vault-Ordner war nicht pruefbar.", UBUNTU_GELADEN, HINWEIS_UBUNTU, HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, vault="nicht_pruefbar", ubuntu_profil="geladen", system_bwrap="ubuntu")))
# Nachtrag Halter (23.11.): die Einheit des Halters (nach der Brücke), die Ebenen der Ablage und der frühere Ort der Anmeldung.
def halter_einheit(zustand, bruecke_zustand=None):
    def a(o):
        if bruecke_zustand:
            o["dienste"].append({"name": "werkbank-bruecke.service", "zustand": bruecke_zustand})
        o["dienste"].append({"name": "wb-halter.service", "zustand": zustand})
    return lx(a)


fall_pruef("linux-dienst-halter-gruen", "Nachtrag Halter: Kern, Brücke und Halter ok", 0, [OK7], halter_einheit("ok", "ok"))
for z, satz in (("fehlt", "pruef.dienst_fehlt: Der Dienst wb-halter.service ist nicht eingetragen."),
                ("inaktiv", "pruef.dienst_inaktiv: Der Dienst wb-halter.service ist nicht aktiv."),
                ("fehlgeschlagen", "pruef.dienst_fehlgeschlagen: Der Dienst wb-halter.service ist fehlgeschlagen."),
                ("nicht_pruefbar", "pruef.dienst_nicht_pruefbar: Der Dienst wb-halter.service war nicht pruefbar.")):
    fall_pruef("linux-dienst-halter-" + z.replace("_", "-"), f"Nachtrag Halter: wb-halter.service im Zustand {z} (etwa Exit 78 bei falscher Ablage oder halter.json)", 1,
               rot([satz]), halter_einheit(z))
fall_pruef("linux-dienst-bruecke-und-halter-reihenfolge", "Nachtrag Halter: Brücke und Halter rot: der Halter steht nach der Brücke", 1,
           rot(["pruef.dienst_inaktiv: Der Dienst werkbank-bruecke.service ist nicht aktiv.",
                "pruef.dienst_fehlgeschlagen: Der Dienst wb-halter.service ist fehlgeschlagen."]), halter_einheit("fehlgeschlagen", "inaktiv"))
fall_pruef("linux-copilot-ablage", "Nachtrag Halter: eine Ebene der Ablage ist ein Verweis oder geheim/ ist nicht 0700; die Zeile nennt keinen Pfad", 1,
           rot(["pruef.copilot_ablage: Die Ablage des Halters hat falsche Rechte, einen falschen Eigentuemer oder einen Verweis."]), lx(lambda o: o.update(copilot="ablage")))
# Nachtrag 1 zum Entscheid des Betreibers (28.11.2026): je Ebene, die für eine nicht private Gruppe schreibbar ist, ein Hinweis mit chmod g-w und dem
# Pfad; die Hinweise zählen nicht in pruef.rot und stehen vor den anderen Hinweisen.
ABLAGE_SATZ = "pruef.copilot_ablage: Die Ablage des Halters hat falsche Rechte, einen falschen Eigentuemer oder einen Verweis."
GRUPPE_HINWEIS = "pruef.hinweis_copilot_gruppe: Die Gruppe ist nicht als private Gruppe pruefbar. Abhilfe: chmod g-w "
fall_pruef("linux-copilot-ablage-gruppe", "Nachtrag 1: zwei Ebenen für eine nicht private Gruppe schreibbar (umask 002 mit sss oder zweitem Mitglied): Befund, je ein Hinweis mit Pfad, rc 1", 1,
           [ABLAGE_SATZ, GRUPPE_HINWEIS + "/srv/beispiel/heim/nutzer1/.local", GRUPPE_HINWEIS + "/srv/beispiel/heim/nutzer1/.local/state",
            "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(copilot="ablage", copilot_gruppe=["/srv/beispiel/heim/nutzer1/.local", "/srv/beispiel/heim/nutzer1/.local/state"])))
fall_pruef("linux-copilot-ablage-gruppe-anfuehrung", "Nachtrag 1: ein Pfad mit Leerzeichen und Apostroph steht in einfachen Anführungszeichen", 1,
           [ABLAGE_SATZ, GRUPPE_HINWEIS + "'/srv/beispiel/heim/an na'\\''s/.local'", "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(copilot="ablage", copilot_gruppe=["/srv/beispiel/heim/an na's/.local"])))
fall_pruef("linux-copilot-gruppe-leer", "Nachtrag 1: copilot_gruppe leer ist dasselbe wie ein fehlendes Feld: grün", 0, [OK7],
           lx(lambda o: o.update(copilot_gruppe=[])))
# Nachtrag Rolle aus der Gruppe (25.11.): Rollengruppe wb-bruecke und Rollenprogramm, Teil der Prüfung `dienste`, nach den Zeilen der Einheiten.
RG = {"fehlt": "pruef.rollengruppe_fehlt: Die Rollengruppe wb-bruecke fehlt; der Modellweg bekommt beim Halter keine Rolle.",
      "mitglied": "pruef.rollengruppe_mitglied: Die Rollengruppe wb-bruecke hat ein Mitglied, ein Passwort oder eine falsche Form; "
                  "ein Prozess eines Kontos koennte die Rolle bruecke annehmen.",
      "nicht_pruefbar": "pruef.rollengruppe_nicht_pruefbar: Die Rollengruppe wb-bruecke war nicht pruefbar."}
RP = {"fehlt": "pruef.rollenprogramm_fehlt: Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg fehlt.",
      "modus": "pruef.rollenprogramm_modus: Das Rollenprogramm /usr/lib/werkbank/werkbank-modellweg hat nicht root:wb-bruecke mit Modus 2755.",
      "nicht_pruefbar": "pruef.rollenprogramm_nicht_pruefbar: Das Rollenprogramm war nicht pruefbar."}
fall_pruef("linux-rolle-gruen-mit-feldern", "Nachtrag Rolle aus der Gruppe: beide Felder ausdrücklich ok: grün, acht Prüfungen", 0, [OK7],
           lx(lambda o: o.update(rollengruppe="ok", rollenprogramm="ok")))
for z in ("fehlt", "mitglied", "nicht_pruefbar"):
    fall_pruef("linux-rolle-gruppe-" + z.replace("_", "-"), f"Nachtrag Rolle aus der Gruppe: Rollengruppe im Zustand {z}", 1, rot([RG[z]]),
               lx(lambda o, z=z: o.update(rollengruppe=z)))
for z in ("fehlt", "modus", "nicht_pruefbar"):
    fall_pruef("linux-rolle-programm-" + z.replace("_", "-"), f"Nachtrag Rolle aus der Gruppe: Rollenprogramm im Zustand {z}", 1, rot([RP[z]]),
               lx(lambda o, z=z: o.update(rollenprogramm=z)))
fall_pruef("linux-rolle-beide-nach-den-einheiten", "Nachtrag Rolle aus der Gruppe: Einheit des Halters, Gruppe und Programm rot: erst die Einheit, dann die Gruppe, dann das Programm, dann AppArmor", 1,
           rot(["pruef.dienst_fehlgeschlagen: Der Dienst wb-halter.service ist fehlgeschlagen.", RG["mitglied"], RP["modus"],
                "pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen."]),
           lx(lambda o: (o["dienste"].append({"name": "wb-halter.service", "zustand": "fehlgeschlagen"}),
                         o.update(rollengruppe="mitglied", rollenprogramm="modus", apparmor="nicht_geladen"))))
fall_pruef("linux-rolle-ohne-root-nicht-pruefbar-mit-hinweis", "Nachtrag Rolle aus der Gruppe: ohne root ist /etc/gshadow nicht lesbar: nicht_pruefbar und der Hinweis auf sudo", 1,
           [RG["nicht_pruefbar"], HINWEIS_SUDO, "pruef.rot: 1 Befunde."], lx(lambda o: o.update(ohne_root=True, rollengruppe="nicht_pruefbar")))
fall_pruef("linux-rolle-vor-entfernen-still", "Nachtrag Rolle aus der Gruppe: `prerm` sagt zu Gruppe und Programm nichts", 0, [],
           lx(lambda o: o.update(schluessel="fehlt", copilot="fehlt", rollengruppe="mitglied", rollenprogramm="modus")), aufruf=["--vor-entfernen"])
ALTER_ORT = "pruef.copilot_alter_ort: Am frueheren Ort liegt noch eine Anmeldedatei der Copilot CLI; sie wird nicht mehr gelesen und muss weg."
fall_pruef("linux-copilot-alter-ort", "Nachtrag Halter: anmeldung.copilot ist in Ordnung, die Datei am früheren Ort liegt noch da: ein eigener Befund", 1,
           rot([ALTER_ORT]), lx(lambda o: o.update(copilot_alt=True)))
fall_pruef("linux-copilot-alter-ort-und-fehlt", "Nachtrag Halter: neue Anmeldung fehlt, alte Datei liegt noch: erst die Zeile der Prüfung copilot, dann der alte Ort", 1,
           rot(["pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt.", ALTER_ORT]), lx(lambda o: o.update(copilot="fehlt", copilot_alt=True)))
fall_pruef("linux-copilot-alter-ort-falsch-ist-still", "Nachtrag Halter: copilot_alt false ist dasselbe wie ein fehlendes Feld: grün", 0, [OK7],
           lx(lambda o: o.update(copilot_alt=False)))
fall_pruef("linux-vor-entfernen-ablage", "Nachtrag Halter: `prerm` bei falschen Rechten der Ablage: die Anmeldung liegt noch, der feste Satz, rc 0", 0,
           ["pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie."],
           lx(lambda o: o.update(schluessel="fehlt", copilot="ablage")), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-alter-ort-still", "Nachtrag Halter: `prerm` sagt zum früheren Ort nichts (purge löscht ihn mit /var/lib/werkbank)", 0, [],
           lx(lambda o: o.update(schluessel="fehlt", copilot="fehlt", copilot_alt=True)), aufruf=["--vor-entfernen"])
fall_pruef("linux-copilot-und-schluessel-reihenfolge", "beide Dateien fehlen: erst die Zeile des Modellweg-Schlüssels, dann die der Anmeldung", 1,
           rot(["pruef.schluessel_fehlt: Die Schluesseldatei fehlt.", "pruef.copilot_fehlt: Die Anmeldedatei der Copilot CLI fehlt."]),
           lx(lambda o: o.update(schluessel="fehlt", copilot="fehlt")))
fall_pruef("linux-vor-entfernen-copilot-liegt", "`prerm`: nur die Anmeldedatei der Copilot CLI liegt da, ein fester Satz ohne Inhalt, rc 0", 0,
           ["pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie."],
           lx(lambda o: o.update(schluessel="fehlt")), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-beide-liegen", "`prerm`: beide Dateien liegen da, zwei feste Sätze, rc 0", 0,
           ["pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie.",
            "pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie."], lx(), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-copilot-nicht-pruefbar", "`prerm`: die Anmeldedatei war nicht prüfbar: der feste Satz der Prüfung, rc 0", 0,
           ["pruef.copilot_nicht_pruefbar: Die Anmeldedatei der Copilot CLI war nicht pruefbar."],
           lx(lambda o: o.update(schluessel="fehlt", copilot="nicht_pruefbar")), aufruf=["--vor-entfernen"])
fall_pruef("linux-einstellungen-ungueltig", "Einstellungsdatei ungültig nach Vertrag 2", 1,
           rot(["pruef.einstellungen_ungueltig: Die Einstellungsdatei ist ungueltig (einst.route_unbekannt)."]),
           lx(lambda o: o.update(einstellungen={"zustand": "ungueltig", "code": "einst.route_unbekannt"})))
fall_pruef("linux-vault-rechte", "Vorführ-Vault mit falschen Rechten", 1, rot(["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte."]),
           lx(lambda o: o.update(vault="rechte")))
fall_pruef("linux-paketstand-datei-abweichend", "das AppArmor-Profil im System weicht vom Manifest ab", 1,
           rot(["pruef.paketstand_datei_abweichend: Die Datei etc/apparmor.d/usr.lib.werkbank.werkbank-lauf weicht vom Manifest ab."]),
           lx(lambda o: o["paketstand"].update(dateien=[{"pfad": "etc/apparmor.d/usr.lib.werkbank.werkbank-lauf", "art": "abweichend"}])))
fall_pruef("linux-alles-rot", "jede Prüfung rot: Reihenfolge Dienste, AppArmor, Einstellungen, Nutzerordner, Schlüssel, Vault, Paketstand", 1,
           rot(["pruef.dienst_inaktiv: Der Dienst werkbank-kern.service ist nicht aktiv.",
                "pruef.dienst_fehlgeschlagen: Der Dienst werkbank-bruecke.service ist fehlgeschlagen.",
                "pruef.apparmor_nicht_geladen: Das AppArmor-Profil ist nicht geladen.",
                "pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar.",
                "pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt.",
                "pruef.schluessel_modus: Die Schluesseldatei hat nicht den Modus 0600.",
                "pruef.vault_fehlt: Der Vault-Ordner fehlt.",
                "pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt."]),
           lx(lambda o: o.update(dienste=[{"name": "werkbank-kern.service", "zustand": "inaktiv"},
                                          {"name": "werkbank-bruecke.service", "zustand": "fehlgeschlagen"}],
                                 apparmor="nicht_geladen", einstellungen={"zustand": "fehlt"}, nutzerordner="fehlt", schluessel="modus",
                                 vault="fehlt", paketstand={"manifest": "fehlt", "bau": "auslieferung", "version": "gleich", "dateien": []})))
fall_pruef("linux-vor-entfernen-schluessel-liegt", "`prerm` ruft den Prüfbefehl vor dem Entfernen: Eine Schlüsseldatei liegt da, ein fester Satz ohne Inhalt, rc 0", 0,
           ["pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie."], lx(lambda o: o.update(copilot="fehlt")), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-schluessel-falscher-modus", "die Datei liegt mit falschem Modus da: derselbe Satz, kein Modus und kein Pfad darin", 0,
           ["pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie."], lx(lambda o: o.update(schluessel="modus", copilot="fehlt")), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-ohne-schluessel", "keine Schlüsseldatei: nichts zu melden, rc 0, keine Zeile", 0, [], lx(lambda o: o.update(schluessel="fehlt", copilot="fehlt")), aufruf=["--vor-entfernen"])
fall_pruef("linux-vor-entfernen-nicht-pruefbar", "nicht prüfbar: der feste Satz der Prüfung, rc 0 (das Entfernen scheitert nie)", 0,
           ["pruef.schluessel_nicht_pruefbar: Die Schluesseldatei war nicht pruefbar."], lx(lambda o: o.update(schluessel="nicht_pruefbar", copilot="fehlt")), aufruf=["--vor-entfernen"])
fall_pruef("windows-vor-entfernen-argument", "`--vor-entfernen` gibt es nur unter Ubuntu: unter Windows ein unbekanntes Argument", 64,
           ["pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht."], aufruf=["--vor-entfernen"], strom="stderr")
fall_pruef("linux-aufruf-argument", "ein Argument unter Linux: Code 64", 64, ["pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht."],
           lx(), aufruf=["x"], strom="stderr")

# Zulassung der lokalen CA (Nachtrag 21.11.): ein Hinweis, kein Befund; die Zeile fehlt, wenn das Feld `ca_zulassung` der Lage fehlt
CA_OFFEN = "pruef.hinweis_ca_offen: Die Zulassung der lokalen CA ist offen; die Cloud-Route ist gesperrt. Das ist kein Fehler der Installation."
CA_ABGELEHNT = "pruef.hinweis_ca_abgelehnt: Die Zulassung der lokalen CA ist abgelehnt; die Cloud-Route ist gesperrt."
CA_ZUGELASSEN = "pruef.hinweis_ca_zugelassen: Die Zulassung der lokalen CA ist erteilt; die Cloud-Route ist nicht gesperrt."
CA_UNBEKANNT = "pruef.hinweis_ca_unbekannt: Die Zulassung der lokalen CA war nicht lesbar; die Cloud-Route ist gesperrt."
fall_pruef("linux-ca-offen", "die CA-Zulassung ist offen (Vorgabe nach der Installation): eine eigene Zeile, die Cloud-Route ist gesperrt, kein Fehler: rc 0", 0,
           [CA_OFFEN, OK7], lx(lambda o: o.update(ca_zulassung="offen")))
fall_pruef("linux-ca-abgelehnt", "die CA-Zulassung ist abgelehnt: eine eigene Zeile, rc 0", 0, [CA_ABGELEHNT, OK7], lx(lambda o: o.update(ca_zulassung="abgelehnt")))
fall_pruef("linux-ca-zugelassen", "die CA-Zulassung ist erteilt: grün, eine Zeile, rc 0", 0, [CA_ZUGELASSEN, OK7], lx(lambda o: o.update(ca_zulassung="zugelassen")))
fall_pruef("linux-ca-unbekannt", "lauf.json ist da, aber nicht lesbar oder ohne einen der drei Werte: die Cloud-Route gilt als gesperrt, eine Zeile, rc 0", 0,
           [CA_UNBEKANNT, OK7], lx(lambda o: o.update(ca_zulassung="unbekannt")))
fall_pruef("linux-ca-offen-mit-befund", "ein Befund und offene CA-Zulassung: rc 1, die CA-Zeile steht nach den Befunden und zählt nicht in `pruef.rot`", 1,
           ["pruef.vault_fehlt: Der Vault-Ordner fehlt.", CA_OFFEN, "pruef.rot: 1 Befunde."], lx(lambda o: o.update(vault="fehlt", ca_zulassung="offen")))
fall_pruef("linux-ca-reihenfolge-der-hinweise", "Reihenfolge der Hinweise: Ubuntus Profil, System-bwrap, CA-Zulassung, dann `hinweis_sudo`", 1,
           ["pruef.vault_nicht_pruefbar: Der Vault-Ordner war nicht pruefbar.", UBUNTU_GELADEN, HINWEIS_UBUNTU, CA_OFFEN, HINWEIS_SUDO, "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(ohne_root=True, vault="nicht_pruefbar", ubuntu_profil="geladen", system_bwrap="ubuntu", ca_zulassung="offen")))
fall_pruef("linux-ca-vor-entfernen-ohne-zeile", "`prerm`: die CA-Zeile gehört nicht zu `--vor-entfernen`", 0, [],
           lx(lambda o: o.update(schluessel="fehlt", copilot="fehlt", ca_zulassung="offen")), aufruf=["--vor-entfernen"])

# Programmordner und Harness-Suche (Nachtrag 21.11.): ein verbotener Programmordner in lauf.json ist ein Befund; ein fehlendes Harness ist ein Hinweis mit Grund
PO_VERBOTEN = ("pruef.programmordner_verboten: Ein Programmordner in lauf.json beruehrt den Anmeldespeicher oder ein Geheimnis; "
               "der Einschluss lehnt jede Sitzung ab (5 verboten).")
HN_NICHT_GEFUNDEN = ("pruef.hinweis_harness_nicht_gefunden: {n} nicht gefunden in ~/.nvm/versions/node/v*/bin, ~/.local/bin, ~/.npm-global/bin, "
                     "~/.volta/bin, /usr/local/bin, /usr/bin; andere Orte werden nicht gesucht.")
HN_AUSSERHALB = ("pruef.hinweis_harness_ausserhalb: {n} liegt an einem gesuchten Ort, der echte Pfad aber nicht unter /usr, /opt, ~/.nvm, ~/.local, "
                 "~/.npm-global oder ~/.volta; er wird nicht genommen.")
HN_NICHT_EINGETRAGEN = "pruef.hinweis_harness_nicht_eingetragen: {n} liegt an einem gesuchten Ort, steht aber nicht in lauf.json; sudo dpkg-reconfigure werkbank."
CA_DOPPELT = "pruef.hinweis_ca_doppelt: lauf.json nennt ca_zulassung mehr als einmal; das gilt als offen, die Cloud-Route ist gesperrt."
fall_pruef("linux-programmordner-verboten", "ein Programmordner in lauf.json berührt den Anmeldespeicher (so einen Ordner trug die Einrichtung früher ein): ein eigener Befund mit festem Code, rc 1", 1,
           [PO_VERBOTEN, "pruef.rot: 1 Befunde."], lx(lambda o: o.update(programmordner="verboten")))
fall_pruef("linux-programmordner-ok", "die Programmordner von lauf.json bestehen die Regel des Einschlusses: keine Zeile", 0, [OK7], lx(lambda o: o.update(programmordner="ok")))
fall_pruef("linux-programmordner-verboten-vor-schluessel", "Reihenfolge: der Befund des Programmordners steht nach dem Nutzerordner und vor der Schlüsseldatei", 1,
           ["pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt.", PO_VERBOTEN, "pruef.schluessel_fehlt: Die Schluesseldatei fehlt.", "pruef.rot: 3 Befunde."],
           lx(lambda o: o.update(programmordner="verboten", nutzerordner="fehlt", schluessel="fehlt")))
fall_pruef("linux-harness-nicht-gefunden", "pi steht nicht in lauf.json und liegt an keinem gesuchten Ort: ein Hinweis nennt die abgesuchten Orte, kein Befund, rc 0", 0,
           [HN_NICHT_GEFUNDEN.format(n="pi"), OK7], lx(lambda o: o.update(harness={"pi": "nicht_gefunden"})))
fall_pruef("linux-harness-ausserhalb", "pi liegt an einem gesuchten Ort, der echte Pfad aber außerhalb der erlaubten Wurzeln: eigener Hinweis, kein Befund", 0,
           [HN_AUSSERHALB.format(n="pi"), OK7], lx(lambda o: o.update(harness={"pi": "ausserhalb"})))
fall_pruef("linux-harness-nicht-eingetragen", "pi liegt an einem gesuchten Ort und steht nicht in lauf.json (Datei älter als die Installation von pi): Hinweis auf dpkg-reconfigure", 0,
           [HN_NICHT_EINGETRAGEN.format(n="pi"), OK7], lx(lambda o: o.update(harness={"pi": "nicht_eingetragen"})))
fall_pruef("linux-harness-beide-reihenfolge", "beide Harness fehlen: erst copilot, dann pi, nach dem Hinweis der CA", 0,
           [CA_OFFEN, HN_NICHT_GEFUNDEN.format(n="copilot"), HN_AUSSERHALB.format(n="pi"), OK7],
           lx(lambda o: o.update(harness={"copilot": "nicht_gefunden", "pi": "ausserhalb"}, ca_zulassung="offen")))
fall_pruef("linux-ca-doppelter-schluessel", "lauf.json nennt ca_zulassung zweimal: eigene Zeile mit festem Code, gilt als offen (wie im Paket), kein Befund, rc 0", 0,
           [CA_DOPPELT, OK7], lx(lambda o: o.update(ca_zulassung="doppelt")))

# Harness that postinst did not enter (Folgezug "weich auslassen"): one finding each, from the state file of the package; the folder stands last.
AB_SATZ = ("pruef.programmordner_verboten: Harness {n} nicht eingetragen. Abhilfe: Harness unter einem anderen Praefix installieren, "
           "dann dpkg-reconfigure werkbank. Ordner: {o}")
fall_pruef("linux-abgelehnt-ein-harness", "postinst trug pi nicht ein, weil ein Programmordner den Anmeldespeicher berührt: ein Befund mit festem Code, dem Harness, der Abhilfe und dem Ordner, rc 1", 1,
           [AB_SATZ.format(n="pi", o="/srv/beispiel/heim/nutzer1/.local"), "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(programmordner_abgelehnt=[{"harness": "pi", "ordner": "/srv/beispiel/heim/nutzer1/.local"}])))
fall_pruef("linux-abgelehnt-beide-reihenfolge", "beide Harness abgelehnt: erst copilot, dann pi, auch wenn die Lage sie umgekehrt nennt; je eine Zeile", 1,
           [AB_SATZ.format(n="copilot", o="/opt/c"), AB_SATZ.format(n="pi", o="/srv/beispiel/.ssh"), "pruef.rot: 2 Befunde."],
           lx(lambda o: o.update(programmordner_abgelehnt=[{"harness": "pi", "ordner": "/srv/beispiel/.ssh"}, {"harness": "copilot", "ordner": "/opt/c"}])))
fall_pruef("linux-abgelehnt-ohne-doppelten-hinweis", "ein abgelehntes Harness hat nur den Befund, nicht noch den Hinweis „nicht eingetragen“; das andere Harness behält seinen Hinweis (die Hinweise stehen nach den Befunden)", 1,
           [AB_SATZ.format(n="pi", o="/opt/p"), HN_NICHT_GEFUNDEN.format(n="copilot"), "pruef.rot: 1 Befunde."],
           lx(lambda o: o.update(programmordner_abgelehnt=[{"harness": "pi", "ordner": "/opt/p"}], harness={"copilot": "nicht_gefunden", "pi": "nicht_eingetragen"})))
fall_pruef("linux-abgelehnt-vor-schluessel", "Reihenfolge: nach dem Befund des Programmordners in lauf.json und vor der Schlüsseldatei", 1,
           ["pruef.nutzerordner_fehlt: Der Ordner des Nutzers unter /var/lib/werkbank fehlt.", PO_VERBOTEN, AB_SATZ.format(n="pi", o="/opt/p"),
            "pruef.schluessel_fehlt: Die Schluesseldatei fehlt.", "pruef.rot: 4 Befunde."],
           lx(lambda o: o.update(programmordner="verboten", programmordner_abgelehnt=[{"harness": "pi", "ordner": "/opt/p"}], nutzerordner="fehlt", schluessel="fehlt")))
fall_pruef("linux-abgelehnt-leer", "keine Ablehnung in der Zustandsdatei: keine Zeile", 0, [OK7], lx(lambda o: o.update(programmordner_abgelehnt=[])))

# ------------------------------------------------------------------------------------------ Prüfbefehl macOS
LAGE_MAC = {
    "plattform": "macos",
    "konten": 1,
    "launchd": "ok",
    "schluesselbund": "ok",
    "einschluss": "ok",
    "einstellungen": {"zustand": "ok"},
    "vault": "ok",
    "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []},
    "stelle": "ok",
}
OK6 = "pruef.ok: Alle 6 Pruefungen bestanden."


def mc(aendern=None):
    obj = copy.deepcopy(LAGE_MAC)
    if aendern:
        aendern(obj)
    return obj


fall_pruef("macos-gruen", "macOS: alle sechs Prüfungen bestanden", 0, [OK6], mc())
fall_pruef("macos-stelle-einrichten", "A108 unter macOS: nur die Stelle fehlt, zwei Konten: rc 20", 20, [STELLE_ZEILE], mc(lambda o: o.update(stelle="fehlt", konten=2)))
fall_pruef("macos-stelle-nicht-bei-befund", "A108 Gegenprobe 2 unter macOS: ein Befund gibt rc 1 ohne Stellenzeile", 1,
           rot(["pruef.einschluss_fehlt: Der Einschluss ist nicht verfuegbar."]), mc(lambda o: o.update(stelle="fehlt", konten=3, einschluss="fehlt")))
for z, satz in (("fehlt", "pruef.launchd_fehlt: Der Dienst werkbank.halter ist nicht eingetragen."),
                ("gestoppt", "pruef.launchd_gestoppt: Der Dienst werkbank.halter laeuft nicht."),
                ("konto", "pruef.launchd_konto: Der Dienst werkbank.halter laeuft nicht unter seinem eigenen Konto."),
                ("nicht_pruefbar", "pruef.launchd_nicht_pruefbar: Der Dienst werkbank.halter war nicht pruefbar.")):
    fall_pruef("macos-launchd-" + z.replace("_", "-"), f"Halter-Dienst im Zustand {z}", 1, rot([satz]), mc(lambda o, z=z: o.update(launchd=z)))
for z, satz in (("fehlt", "pruef.schluesselbund_fehlt: Der Schluesselbund-Eintrag des Modellwegs fehlt."),
                ("nicht_pruefbar", "pruef.schluesselbund_nicht_pruefbar: Der Schluesselbund-Eintrag war nicht pruefbar.")):
    fall_pruef("macos-schluesselbund-" + z.replace("_", "-"), f"Schlüsselbund-Eintrag im Zustand {z}; die Zeile nennt weder Namen noch Inhalt", 1,
               rot([satz]), mc(lambda o, z=z: o.update(schluesselbund=z)))
for z, satz in (("fehlt", "pruef.einschluss_fehlt: Der Einschluss ist nicht verfuegbar."),
                ("probelauf", "pruef.einschluss_probelauf: Der Probelauf des Einschlusses scheitert."),
                ("nicht_pruefbar", "pruef.einschluss_nicht_pruefbar: Der Einschluss war nicht pruefbar.")):
    fall_pruef("macos-einschluss-" + z.replace("_", "-"), f"Einschluss im Zustand {z}", 1, rot([satz]), mc(lambda o, z=z: o.update(einschluss=z)))
fall_pruef("macos-vault-rechte", "Vault-Ordner mit falschen Rechten", 1, rot(["pruef.vault_rechte: Der Vault-Ordner hat falsche Rechte."]), mc(lambda o: o.update(vault="rechte")))
fall_pruef("macos-alles-rot", "jede Prüfung rot: Reihenfolge Halter, Schlüsselbund, Einschluss, Einstellungen, Vault, Paketstand", 1,
           rot(["pruef.launchd_gestoppt: Der Dienst werkbank.halter laeuft nicht.",
                "pruef.schluesselbund_fehlt: Der Schluesselbund-Eintrag des Modellwegs fehlt.",
                "pruef.einschluss_probelauf: Der Probelauf des Einschlusses scheitert.",
                "pruef.einstellungen_fehlt: Die Einstellungsdatei fehlt oder ist nicht lesbar.",
                "pruef.vault_fehlt: Der Vault-Ordner fehlt.",
                "pruef.paketstand_manifest_fehlt: Das Manifest des Pakets fehlt."]),
           mc(lambda o: o.update(launchd="gestoppt", schluesselbund="fehlt", einschluss="probelauf", einstellungen={"zustand": "fehlt"}, vault="fehlt",
                                 paketstand={"manifest": "fehlt", "bau": "auslieferung", "version": "gleich", "dateien": []})))
fall_pruef("macos-aufruf-argument", "ein Argument unter macOS: Code 64", 64, ["pruef.aufruf: Der Pruefbefehl kennt dieses Argument nicht."], mc(), aufruf=["x"], strom="stderr")

# ------------------------------------------------------------------------------------------ Statischer Bau
def fall_statisch(name, beschreibung, programme, code=None):
    fall("statisch-" + name, {"beschreibung": beschreibung, "form": "statisch", "programme": programme},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


def elf(ziel, interp=False, needed=()):
    return {"ziel": ziel, "interp": interp, "needed": list(needed)}


STARTER_ZIEL = "usr/lib/werkbank/werkbank-einschluss"
fall_statisch("gueltig", "Starter ist statisch: kein Interpreter, keine DT_NEEDED", [elf(STARTER_ZIEL)])
fall_statisch("gueltig-geruest-ohne-weiterreicher", "im Gerüst fehlt der Weiterreicher; geprüft wird, was gestuft ist", [elf(STARTER_ZIEL)])
fall_statisch("gueltig-dynamisches-fremdes-programm", "ein anderes Programm des Pakets darf dynamisch sein (der Modellweg löst Namen auf)",
              [elf(STARTER_ZIEL), elf("usr/lib/werkbank/werkbank-modellweg", True, ["libc.so.6"])])
fall_statisch("starter-mit-interpreter", "der Starter hat einen Programminterpreter: er ist dynamisch gelinkt", [elf(STARTER_ZIEL, True)], "programm_nicht_statisch")
fall_statisch("starter-mit-needed", "der Starter listet eine Bibliothek (DT_NEEDED)", [elf(STARTER_ZIEL, False, ["libc.so.6"])], "programm_nicht_statisch")
WEITER_ZIEL = "usr/lib/werkbank/einschluss-weiter"
fall_statisch("gueltig-mit-weiterreicher", "Starter und Weiterreicher sind statisch", [elf(STARTER_ZIEL), elf(WEITER_ZIEL)])
fall_statisch("weiterreicher-dynamisch", "der Weiterreicher ist dynamisch gelinkt (Programminterpreter und Bibliotheken)", [elf(STARTER_ZIEL), elf(WEITER_ZIEL, True, ["libc.so.6"])], "programm_nicht_statisch")
fall_statisch("weiterreicher-mit-needed", "der Weiterreicher listet eine Bibliothek (DT_NEEDED)", [elf(WEITER_ZIEL, False, ["libgcc_s.so.1"])], "programm_nicht_statisch")
# Entscheidung vom 21.11.2026: Läufer, Trampolin und Ordnermarke laufen außerhalb des Einschlusses und ohne Profil; sie sind dynamisch (OpenSSL der Distribution).
LAUF_ZIEL, KIND_ZIEL, OM_ZIEL = "usr/lib/werkbank/werkbank-lauf", "usr/lib/werkbank/werkbank-lauf-kind", "usr/bin/werkbank-ordnermarke"
fall_statisch("laeufer-dynamisch-ist-erlaubt", "der Läufer ist dynamisch gelinkt (libssl3t64 der Distribution): die Statik-Prüfung gilt für ihn nicht", [elf(STARTER_ZIEL), elf(LAUF_ZIEL, True, ["libc.so.6", "libssl.so.3"])])
fall_statisch("trampolin-dynamisch-ist-erlaubt", "das Trampolin ist dynamisch gelinkt: erlaubt", [elf(STARTER_ZIEL), elf(KIND_ZIEL, True, ["libc.so.6"])])
fall_statisch("ordnermarke-dynamisch-ist-erlaubt", "`werkbank-ordnermarke` ist dynamisch gelinkt: erlaubt", [elf(STARTER_ZIEL), elf(OM_ZIEL, True, ["libc.so.6"])])

# ------------------------------------------------------------------------------------------ lauf.json (postinst)
import sys  # noqa: E402
sys.path.insert(0, str(HIER))
import referenz as _ref  # noqa: E402


def fall_lauf(name, beschreibung, **lage):
    f = {"beschreibung": beschreibung, "form": "laufkonfig", "nutzer": "nutzer1", "uid": 1000, "heim": "/srv/beispiel/heim/nutzer1", "harness": {}, "vorher": None}
    f.update(lage)
    datei = _ref.laufkonfig(f)
    e = {"gueltig": True, "datei": datei}
    if datei is not None:
        e.update(modus="0644", eigentuemer="root:root")
        ab = _ref.lauf_abgelehnt(f)
        if ab:
            # a harness with a refused program folder is not entered: postinst ends with 0, writes one journal line (code 19, harness, no path) and the state file
            e["abgelehnt_datei"] = "".join(f"{h} {o}\n" for h, o in ab)
            e["journal"] = [f"werkbank: Programmordner abgelehnt, Code 19, Harness {h}" for h, _ in ab]
    fall("laufkonfig-" + name, f, e)


PI_NPM = {"programm": "/srv/beispiel/heim/nutzer1/.nvm/versions/node/v24.1.0/lib/node_modules/pi-coding-agent/dist/cli.js",
          "node": "/srv/beispiel/heim/nutzer1/.nvm/versions/node/v24.1.0/bin/node"}
COPILOT_NPM = {"programm": "/usr/local/lib/node_modules/@github/copilot/index.js", "node": "/usr/local/bin/node"}
fall_lauf("ohne-harness", "kein Harness gefunden: die Tabelle `harness` ist leer, die Ordner fehlen, PATH ist fest; die Datei steht trotzdem da (lauf endet dann mit seinem festen Code)")
fall_lauf("pi-nvm", "pi unter einer nvm-Installation des Nutzers: die Paketwurzel von pi und der bin-Ordner von node als Programmordner (nicht der ganze Präfix), der bin-Ordner von node vorn im PATH",
          harness={"pi": PI_NPM})
fall_lauf("copilot-usr-local", "Copilot unter /usr/local: nichts unter /usr braucht einen Programmordner, PATH bleibt fest", harness={"copilot": COPILOT_NPM})
fall_lauf("beide-harness", "beide Harness gefunden: beide Einträge, Programmordner ohne Doppel, Reihenfolge copilot, pi", harness={"copilot": COPILOT_NPM, "pi": PI_NPM})
OPENCODE_NPM = {"programm": "/srv/beispiel/heim/nutzer1/.npm-global/lib/node_modules/opencode-ai/bin/opencode.exe", "node": None}
fall_lauf("opencode-npm", "opencode (eigenes Programm ohne node) unter dem npm-Prefix <heim>/.npm-global: die Paketwurzel opencode-ai ist der Programmordner, PATH bleibt fest",
          harness={"opencode": OPENCODE_NPM})
fall_lauf("drei-harness", "alle drei Harness gefunden: drei Einträge in der Reihenfolge copilot, pi, opencode, Programmordner ohne Doppel",
          harness={"copilot": COPILOT_NPM, "pi": PI_NPM, "opencode": OPENCODE_NPM})
fall_lauf("harness-einzelprogramm", "ein Harness als einzelnes Programm außerhalb von /usr ohne node: sein Ordner ist der Programmordner",
          harness={"pi": {"programm": "/opt/pi/bin/pi", "node": None}})
fall_lauf("ordner-genestet", "Paketwurzel von pi und bin-Ordner von node (mit copilot) liegen nebeneinander, keiner im anderen: beide bleiben (vorher: ein Präfix umfasste beide)",
          harness={"pi": {"programm": "/opt/pi/lib/node_modules/pi/cli.js", "node": "/opt/pi/bin/node"}, "copilot": {"programm": "/opt/pi/bin/copilot", "node": None}})
fall_lauf("anderer-nutzer", "die Werte hängen am Nutzer und seiner uid: Ordner, laufzeit, Heim", nutzer="anna", uid=1234, heim="/srv/beispiel/heim/anna")
fall_lauf("laengster-nutzer", "Nutzername mit 32 Zeichen und die größte uid: laufzeit hängt nur an der uid, der längste Socketpfad bleibt unter 108 Byte",
          nutzer="a" * 32, uid=4294967294, heim="/srv/beispiel/heim/" + "a" * 32)
fall_lauf("vorher-zugelassen", "eine frühere Datei trug `zugelassen` (vom Betreiber eingetragen): dpkg-reconfigure behält es", vorher="zugelassen")
fall_lauf("vorher-abgelehnt", "eine frühere Datei trug `abgelehnt`: bleibt", vorher="abgelehnt")
fall_lauf("vorher-unbekannt", "ein unbekannter früherer Wert wird nie übernommen: `offen`", vorher="ja")
fall_lauf("vorher-umgeschrieben-indent1", "eine Datei, die ein Admin mit einem anderen Werkzeug neu geschrieben hat (json.dumps mit indent=1, Leerzeichen um den Doppelpunkt): `zugelassen` bleibt", vorher="zugelassen", vorher_form="indent1")
fall_lauf("vorher-umgeschrieben-zeilen", "Schlüssel, Doppelpunkt und Wert auf getrennten Zeilen, mit Tabulator und CR LF: `abgelehnt` bleibt", vorher="abgelehnt", vorher_form="zeilen")
fall_lauf("vorher-doppelter-schluessel", "der Schlüssel `ca_zulassung` steht zweimal: der Wert gilt als ungültig, das Ergebnis ist `offen`", vorher="zugelassen", vorher_form="doppelt")
fall_lauf("vorher-als-wert", "der Text `ca_zulassung` steht als zweiter Schlüssel-Treffer in einem anderen Wert: die Datei gilt als ungültig, das Ergebnis ist `offen`", vorher="zugelassen", vorher_form="als-wert")
fall_lauf("vorher-leerzeichen-im-wert", "der Wert `\"zugelassen \"` (mit Leerzeichen im Wert) ist keiner der drei: das Ergebnis ist `offen`", vorher="zugelassen", vorher_form="leerzeichen-im-wert")
fall_lauf("antwort-zugelassen", "eine Antwort von debconf wartet (`zugelassen`, Frage von dpkg-reconfigure oder debconf-set-selections), die Datei fehlt noch: zugelassen",
          antwort="zugelassen")
fall_lauf("antwort-gilt-vor-der-datei", "die Datei trug `zugelassen`, die wartende Antwort ist `abgelehnt`: die Antwort gilt", vorher="zugelassen", antwort="abgelehnt")
fall_lauf("antwort-offen-nimmt-zurueck", "die Datei trug `zugelassen`, die wartende Antwort ist `offen`: offen (ein Admin nimmt die Zulassung zurück)", vorher="zugelassen", antwort="offen")
fall_lauf("antwort-ungueltig", "eine Antwort außerhalb der drei Werte wird nie übernommen: die Datei gilt", vorher="zugelassen", antwort="ja")
fall_lauf("kein-nutzer", "ohne sudo-Nutzer schreibt postinst keine Datei", nutzer=None)
fall_lauf("heim-form-falsch", "ein Heimordner mit Leerzeichen oder Punktsegment ist keine Ordnerform: keine Datei", heim="/srv/beispiel/heim/nutzer 1")
fall_lauf("heim-relativ", "ein relativer Heimordner: keine Datei", heim="heim/nutzer1")
fall_lauf("harness-pfad-form-falsch", "ein Harnessprogramm mit Anführungszeichen im Pfad: keine Datei (nie ein Pfad, der JSON bricht)",
          harness={"pi": {"programm": "/opt/pi/\"x/pi", "node": None}})
H1 = "/srv/beispiel/heim/nutzer1"
PI_PAKET = "pi-coding-agent"
fall_lauf("prefix-local", "pi mit npm --prefix <heim>/.local: der Programmordner ist die Paketwurzel unter .local/lib/node_modules, nie .local (das den Anmeldespeicher .local/share/keyrings enthält); node aus /usr gibt keinen Ordner",
          harness={"pi": {"programm": f"{H1}/.local/lib/node_modules/{PI_PAKET}/dist/cli.js", "node": "/usr/bin/node"}})
fall_lauf("prefix-npm-global", "pi mit dem npm-Prefix <heim>/.npm-global: die Paketwurzel unter .npm-global/lib/node_modules, nicht der Prefix",
          harness={"pi": {"programm": f"{H1}/.npm-global/lib/node_modules/{PI_PAKET}/dist/cli.js", "node": "/usr/bin/node"}})
fall_lauf("paket-mit-scope", "Paket mit Scope (@scope/paket) unter .local: die Paketwurzel hat zwei Ebenen unter lib/node_modules",
          harness={"pi": {"programm": f"{H1}/.local/lib/node_modules/@earendil-works/{PI_PAKET}/dist/bundle/cli.js", "node": "/usr/bin/node"}})
fall_lauf("node-unter-lokal-bin", "node liegt unter <heim>/.local/bin: der Ordner von node ist .local/bin, nicht zwei Ebenen darüber (.local); er steht im PATH vorn",
          harness={"pi": {"programm": "/opt/pi/pi", "node": f"{H1}/.local/bin/node"}})
fall_lauf("ordner-genestet-bin", "Ein Ordner im bin-Ordner von node fällt zusammen: nur der äußere bleibt; die Paketwurzel liegt daneben",
          harness={"pi": {"programm": "/opt/pi/lib/node_modules/pi/cli.js", "node": "/opt/pi/bin/node"}, "copilot": {"programm": "/opt/pi/bin/werkzeug/copilot", "node": None}})
fall_lauf("verboten-anmeldespeicher", "Der Ordner des Programms liegt im Anmeldespeicher (.local/share/keyrings): das Harness wird nicht eingetragen (Journalzeile mit Code 19, Zustandsdatei), postinst endet mit 0",
          harness={"pi": {"programm": f"{H1}/.local/share/keyrings/pi", "node": None}}, pruefer_verboten=[f"{H1}/.local/share/keyrings"])
fall_lauf("verboten-node-im-anmeldespeicher", "Der Ordner von node liegt im Anmeldespeicher: das Harness wird nicht eingetragen, postinst endet mit 0",
          harness={"pi": {"programm": "/opt/pi/pi", "node": f"{H1}/.local/share/keyrings/bin/node"}}, pruefer_verboten=[f"{H1}/.local/share/keyrings/bin"])
fall_lauf("verboten-geheimnis-segment", "Ein Segment des Programmordners ist ein Geheimnisname (.secrets-sync): das Harness wird nicht eingetragen, postinst endet mit 0",
          harness={"pi": {"programm": "/opt/.secrets-sync/pi/pi", "node": None}}, pruefer_verboten=["/opt/.secrets-sync/pi"])
fall_lauf("verboten-nur-ein-harness", "Nur der Ordner von copilot ist verboten: copilot wird nicht eingetragen, pi und sein Programmordner bleiben; eine Zeile in der Zustandsdatei",
          harness={"copilot": {"programm": f"{H1}/.local/share/keyrings/copilot", "node": None}, "pi": PI_NPM}, pruefer_verboten=[f"{H1}/.local/share/keyrings"])
fall_lauf("verboten-beide-harness", "Beide Harness haben einen verbotenen Ordner: keines wird eingetragen, die Datei hat leere `harness` und `programmordner`, zwei Zeilen in der Zustandsdatei (erst copilot, dann pi)",
          harness={"copilot": {"programm": "/opt/.secrets-sync/c/copilot", "node": None}, "pi": {"programm": f"{H1}/.ssh/pi", "node": None}}, pruefer_verboten=["/opt/.secrets-sync/c", f"{H1}/.ssh"])
fall_lauf("verboten-node-gemeinsam", "Der Ordner von node ist verboten und beide Harness nutzen ihn: beide werden abgelehnt, jedes mit dem Ordner von node (nach dem Paketordner, der erlaubt ist)",
          harness={"copilot": {"programm": "/opt/c/lib/node_modules/c/cli.js", "node": f"{H1}/.local/share/keyrings/bin/node"}, "pi": {"programm": "/opt/p/lib/node_modules/p/cli.js", "node": f"{H1}/.local/share/keyrings/bin/node"}},
          pruefer_verboten=[f"{H1}/.local/share/keyrings/bin"])
fall_lauf("verboten-erste-ablehnung-zaehlt", "Beide Ordner eines Harness sind verboten (Paketordner und node): die Zustandsdatei nennt nur den ersten (den Paketordner), eine Zeile je Harness",
          harness={"pi": {"programm": f"{H1}/.ssh/pi/cli.js", "node": f"{H1}/.local/share/keyrings/node"}}, pruefer_verboten=[f"{H1}/.ssh/pi", f"{H1}/.local/share/keyrings"])
fall_lauf("verboten-kein-harness-betroffen", "Die Prüfung nennt einen verbotenen Ordner, den kein gefundenes Harness braucht: nichts wird abgelehnt, keine Zustandsdatei",
          harness={"pi": {"programm": "/opt/pi/pi", "node": None}}, pruefer_verboten=[f"{H1}/.ssh"])
assert _ref.laufzeit_socket_laenge(4294967294) <= 107

# ------------------------------------------------------------------------------------------ Entfernen (apt purge)
NUTZER = "nutzer1"
VOR = ["datei:/etc/hostname", "datei:/srv/beispiel/heim/nutzer1/.bashrc", "ordner:/var/lib/apt", "datei:/var/log/dpkg.log", "konto:nutzer1"]
INSTALLIERT = ["datei:/usr/bin/werkbank-probelauf", "datei:/usr/lib/werkbank/werkbank-kern", "datei:/etc/apparmor.d/usr.lib.werkbank.werkbank-einschluss",
               "ordner:/var/lib/werkbank", "ordner:/var/lib/werkbank/nutzer1", "datei:/var/lib/werkbank/nutzer1/modellweg.schluessel",
               "datei:/var/lib/werkbank/nutzer1/protokoll/ereignisse.jsonl", "einheit:werkbank-kern.service", "profil:werkbank-einschluss-bwrap"]
ERGEBNIS = "/srv/beispiel/heim/nutzer1/probelauf-ergebnis.json"


def fall_purge(name, beschreibung, nachher, code=None, vorher=None, ergebnisdatei=ERGEBNIS):
    fall("entfernen-" + name, {"beschreibung": beschreibung, "form": "entfernen", "nutzer": NUTZER, "heimwurzel": "/srv/beispiel/heim", "ergebnisdatei": ergebnisdatei,
                               "vorher": VOR if vorher is None else vorher, "nachher": nachher},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_purge("gueltig", "nach `apt purge` ist alles weg, was das Paket anlegte; es bleibt nichts außer den Systemprotokollen", list(VOR))
fall_purge("gueltig-mit-ergebnisdatei", "ausdrückliche Ausnahme 1: die Ergebnisdatei am Ort von `--ergebnis`", VOR + ["datei:" + ERGEBNIS])
fall_purge("gueltig-mit-anmeldung", "ausdrückliche Ausnahme 2: die Anmeldung der Copilot CLI in ihrem eigenen Ordner im Heim",
           VOR + ["ordner:/srv/beispiel/heim/nutzer1/.copilot", "datei:/srv/beispiel/heim/nutzer1/.copilot/anmeldung.json"])
fall_purge("gueltig-systemprotokolle", "ausdrückliche Ausnahme 3: Protokolle von `dpkg`, `apt` und dem Journal",
           VOR + ["datei:/var/log/apt/history.log", "datei:/var/log/apt/term.log", "datei:/var/log/journal/abc/system.journal"])
fall_purge("rolle-gruppe-weg", "Nachtrag Rolle aus der Gruppe: `purge` entfernt die angelegte Gruppe wb-bruecke", list(VOR))
fall_purge("rolle-gruppe-bleibt", "Nachtrag Rolle aus der Gruppe: die Gruppe wb-bruecke steht nach `purge` noch da", VOR + ["gruppe:wb-bruecke"],
           "entfernen_rueckstand")
fall_purge("rolle-gruppe-von-vorher-bleibt", "Nachtrag Rolle aus der Gruppe: eine Gruppe wb-bruecke von vor der Installation (keine Marke) bleibt",
           VOR + ["gruppe:wb-bruecke"], vorher=VOR + ["gruppe:wb-bruecke"])
fall_purge("rolle-gruppe-von-vorher-geloescht", "Nachtrag Rolle aus der Gruppe: `purge` löscht eine Gruppe wb-bruecke, die vorher schon da war",
           list(VOR), "entfernen_verloren", vorher=VOR + ["gruppe:wb-bruecke"])
fall_purge("rolle-marke-bleibt", "Nachtrag Rolle aus der Gruppe: die Marke /var/lib/werkbank/rollengruppe-angelegt bleibt nach `purge`",
           VOR + ["datei:/var/lib/werkbank/rollengruppe-angelegt"], "entfernen_rueckstand")
fall_purge("lauf-json-bleibt", "`/etc/werkbank/lauf.json` bleibt nach `purge` liegen", VOR + ["datei:/etc/werkbank/lauf.json"], "entfernen_rueckstand")
fall_purge("kern-json-bleibt", "`/etc/werkbank/kern.json` bleibt nach `purge` liegen", VOR + ["datei:/etc/werkbank/kern.json"], "entfernen_rueckstand")
fall_purge("bruecke-json-bleibt", "`/etc/werkbank/bruecke.json` bleibt nach `purge` liegen", VOR + ["datei:/etc/werkbank/bruecke.json"], "entfernen_rueckstand")
fall_purge("etc-ordner-bleibt", "der leere Ordner `/etc/werkbank` bleibt nach `purge` liegen", VOR + ["ordner:/etc/werkbank"], "entfernen_rueckstand")
fall_purge("sitzungsordner-lauf-bleibt", "der Ordner `/var/lib/werkbank/s/<uid>` für die Zustandsordner der Sitzungen bleibt nach `purge` liegen", VOR + ["ordner:/var/lib/werkbank/s/1000"], "entfernen_rueckstand")
fall_purge("laufzeitordner-bleibt", "der Ordner `/var/lib/werkbank/l/<uid>` für die Socket-Ordner bleibt nach `purge` liegen", VOR + ["ordner:/var/lib/werkbank/l/1000"], "entfernen_rueckstand")
fall_purge("profil-geladen", "nach `purge` ist kein Label `werkbank-*` mehr geladen; ein Rest einer älteren Installation (vorher schon da) zählt auch",
           VOR + ["profil:werkbank-einschluss-bwrap"], "entfernen_rueckstand", vorher=VOR + ["profil:werkbank-einschluss-bwrap"])
fall_purge("profil-laeufer-geladen", "das Profil des Läufers (Form A) ist noch geladen", VOR + ["profil:/usr/lib/werkbank/werkbank-lauf"], "entfernen_rueckstand")
fall_purge("profil-starter-geladen", "das Profil des festen Starters (Form A, V1) ist noch geladen", VOR + ["profil:/usr/lib/werkbank/werkbank-einschluss"], "entfernen_rueckstand")
fall_purge("profil-kind-geladen", "das Kindprofil `werkbank-einschluss-kind` ist noch geladen", VOR + ["profil:werkbank-einschluss-kind"], "entfernen_rueckstand")
fall_purge("starter-uebrig", "der Starter liegt nach `purge` noch da", VOR + ["datei:/usr/lib/werkbank/werkbank-einschluss"], "entfernen_rueckstand")
fall_purge("profil-kopie-geladen", "das Profil der bwrap-Kopie (Form B) ist noch geladen", VOR + ["profil:/usr/lib/werkbank/bwrap"], "entfernen_rueckstand")
fall_purge("bwrap-kopie-uebrig", "die bwrap-Kopie (Form B) liegt nach `purge` noch da", VOR + ["datei:/usr/lib/werkbank/bwrap"], "entfernen_rueckstand")
fall_purge("profil-einschluss-geladen", "das Label `werkbank-einschluss` (das erste der drei Labels der Profildatei) ist noch geladen", VOR + ["profil:werkbank-einschluss"], "entfernen_rueckstand")
fall_purge("gueltig-alle-drei-labels-weg", "nach `purge` sind alle drei Labels der Profildatei (`werkbank-einschluss`, `werkbank-einschluss-bwrap`, `werkbank-einschluss-kind`) entladen",
           list(VOR), vorher=list(VOR))
fall_purge("gueltig-ubuntu-profil-labels-bleiben", "Ubuntus Profil `bwrap-userns-restrict` (Labels `bwrap` und `unpriv_bwrap`) stand vorher da und bleibt; `purge` entlädt es nicht (nicht von uns)",
           VOR + ["profil:bwrap", "profil:unpriv_bwrap"], vorher=VOR + ["profil:bwrap", "profil:unpriv_bwrap"])
fall_purge("verloren-ubuntu-profil", "`purge` hat Ubuntus Profil `bwrap-userns-restrict` entladen: das ist Fremdes, das verloren ging",
           list(VOR), vorher=VOR + ["profil:bwrap", "profil:unpriv_bwrap"], code="entfernen_verloren")
fall_purge("rueckstand-apparmor-cache", "die übersetzte Zwischendatei des Profils unter `/var/cache/apparmor/` steht nach `purge` noch da (gemessen auf 26.04.1: `apparmor_parser -R` lässt sie liegen)",
           VOR + ["datei:/var/cache/apparmor/f6da0254.0/usr.lib.werkbank.werkbank-einschluss"], "entfernen_rueckstand")
fall_purge("rueckstand-zustandsordner-nutzer", "der Zustandsordner `/var/lib/werkbank/<nutzer>/` samt `ordnermarken.json` steht nach `purge` noch da",
           VOR + ["ordner:/var/lib/werkbank/nutzer1", "datei:/var/lib/werkbank/nutzer1/ordnermarken.json"], "entfernen_rueckstand")
fall_purge("rueckstand-sitzungsordner", "der Ordner `sitzungen/` samt einer `<sitzung>.zielliste.json` steht nach `purge` noch da",
           VOR + ["ordner:/var/lib/werkbank/nutzer1/sitzungen", "datei:/var/lib/werkbank/nutzer1/sitzungen/s1.zielliste.json"], "entfernen_rueckstand")
fall_purge("gueltig-ordnermarken-und-zielliste-weg", "nach `purge` sind Ordnermarken und Ziellisten der Sitzungen mit dem Ordner des Nutzers gelöscht",
           list(VOR), vorher=list(VOR))
fall_purge("gueltig-system-profil-bleibt", "das eigene Profil von Ubuntu für das System-bwrap stand vorher da und bleibt (nicht von uns)",
           VOR + ["profil:/usr/bin/bwrap"], vorher=VOR + ["profil:/usr/bin/bwrap"])
fall_purge("rueckstand-schluessel", "die Schlüsseldatei steht nach `purge` noch da", VOR + ["datei:/var/lib/werkbank/nutzer1/modellweg.schluessel"], "entfernen_rueckstand")
fall_purge("rueckstand-ordner", "der Ordner `/var/lib/werkbank` steht noch da", VOR + ["ordner:/var/lib/werkbank"], "entfernen_rueckstand")
fall_purge("rueckstand-programm", "ein Programm steht noch da", VOR + ["datei:/usr/lib/werkbank/werkbank-kern"], "entfernen_rueckstand")
fall_purge("rueckstand-profil", "das AppArmor-Profil ist noch geladen", VOR + ["profil:werkbank-einschluss-bwrap"], "entfernen_rueckstand")
fall_purge("rueckstand-einheit", "die Einheit ist noch bekannt", VOR + ["einheit:werkbank-kern.service"], "entfernen_rueckstand")
fall_purge("rueckstand-prozess", "ein Prozess des Pakets läuft noch", VOR + ["prozess:werkbank-modellweg"], "entfernen_rueckstand")
fall_purge("rueckstand-konto", "ein vom Paket angelegtes Konto steht noch da", VOR + ["konto:werkbank"], "entfernen_rueckstand")
fall_purge("rueckstand-gruppe", "eine vom Paket angelegte Gruppe steht noch da", VOR + ["gruppe:werkbank"], "entfernen_rueckstand")
fall_purge("rueckstand-fremde-datei-im-heim", "eine andere Datei im Heim ist keine Ausnahme", VOR + ["datei:/srv/beispiel/heim/nutzer1/.config/werkbank/x"], "entfernen_rueckstand")
fall_purge("rueckstand-andere-ergebnisdatei", "nur die Datei am Ort von `--ergebnis` ist Ausnahme, nicht eine zweite", VOR + ["datei:" + ERGEBNIS, "datei:/srv/beispiel/heim/nutzer1/zweite.json"], "entfernen_rueckstand")
fall_purge("rueckstand-copilot-eines-anderen", "der Ordner `.copilot` eines anderen Nutzers ist keine Ausnahme", VOR + ["ordner:/srv/beispiel/heim/nutzer2/.copilot"], "entfernen_rueckstand")
fall_purge("rueckstand-var-lib-nachbar", "`/var/lib/apt2` ist keine Systemausnahme (nur `/var/lib/apt/` darunter)", VOR + ["datei:/var/lib/apt2/x"], "entfernen_rueckstand")
fall_purge("verloren-fremde-datei", "`purge` hat eine Datei entfernt, die nicht zum Paket gehört", [e for e in VOR if e != "datei:/etc/hostname"], "entfernen_verloren")
fall_purge("verloren-heim", "`purge` hat die Heimdatei des Nutzers entfernt", [e for e in VOR if e != "datei:/srv/beispiel/heim/nutzer1/.bashrc"], "entfernen_verloren")
# Nachtrag Halter (23.11.): die Ablage des Halters im Heim ist keine Ausnahme; `/etc/werkbank/halter.json` und die Einheit gehen mit `purge`.
ABL = "/srv/beispiel/heim/nutzer1/.local/state/agent-workbench"
fall_purge("halter-json-bleibt", "Nachtrag Halter: `/etc/werkbank/halter.json` bleibt nach `purge` liegen", VOR + ["datei:/etc/werkbank/halter.json"], "entfernen_rueckstand")
fall_purge("halter-einheit-bleibt", "Nachtrag Halter: die Einheit `wb-halter.service` ist nach `purge` noch bekannt", VOR + ["einheit:wb-halter.service"], "entfernen_rueckstand")
fall_purge("halter-prozess-laeuft", "Nachtrag Halter: der Halter läuft nach `purge` noch", VOR + ["prozess:werkbank-halter"], "entfernen_rueckstand")
fall_purge("ablage-anmeldung-bleibt", "Nachtrag Halter: `anmeldung.copilot` steht nach `purge` noch in der Ablage", VOR + [f"datei:{ABL}/halter/geheim/anmeldung.copilot"], "entfernen_rueckstand")
fall_purge("ablage-ordner-bleibt", "Nachtrag Halter: die leere Ebene `agent-workbench/halter/geheim` steht nach `purge` noch da", VOR + [f"ordner:{ABL}", f"ordner:{ABL}/halter", f"ordner:{ABL}/halter/geheim"], "entfernen_rueckstand")
fall_purge("ablage-temp-bleibt", "Nachtrag Halter: eine Temp-Datei von werkbank-anmelden in der Ablage ist auch ein Rückstand", VOR + [f"datei:{ABL}/halter/geheim/.anmeldung.copilot.4242.tmp"], "entfernen_rueckstand")
fall_purge("ablage-gueltig-weg", "Nachtrag Halter: nach `purge` fehlen anmeldung.copilot und die drei Ebenen; `.local/state` stand vorher da und bleibt",
           VOR + ["ordner:/srv/beispiel/heim/nutzer1/.local/state"], vorher=VOR + ["ordner:/srv/beispiel/heim/nutzer1/.local/state"])
fall_purge("ablage-gueltig-fremdes-bleibt", "Nachtrag Halter: `agent-workbench` stand vorher mit einer fremden Datei da; Ordner und Datei bleiben, nur die Ebenen des Halters gehen",
           VOR + [f"ordner:{ABL}", f"datei:{ABL}/ausgang-audit.jsonl"], vorher=VOR + [f"ordner:{ABL}", f"datei:{ABL}/ausgang-audit.jsonl"])
fall_purge("ablage-verloren-fremdes", "Nachtrag Halter: `purge` hat eine fremde Datei unter `agent-workbench` gelöscht", list(VOR),
           vorher=VOR + [f"datei:{ABL}/ausgang-audit.jsonl"], code="entfernen_verloren")
fall_purge("ablage-verloren-local-state", "Nachtrag Halter: `purge` hat den Ordner `.local/state` entfernt, der vorher da war", list(VOR),
           vorher=VOR + ["ordner:/srv/beispiel/heim/nutzer1/.local/state"], code="entfernen_verloren")
fall_purge("form-eintrag", "ein Eintrag ohne bekannte Art", VOR + ["verzeichnis:/x"], "entfernen_form")
fall_purge("form-relativer-pfad", "ein relativer Pfad", VOR + ["datei:relativ"], "entfernen_form")
fall_purge("reihenfolge-rueckstand-vor-verloren", "Rückstand und Verlust zugleich: `entfernen_rueckstand` zuerst",
           [e for e in VOR if e != "datei:/etc/hostname"] + ["ordner:/var/lib/werkbank"], "entfernen_rueckstand")

# ------------------------------------------------------------------------------------------ Entfernen mit `apt remove`: der Schlüssel bleibt, der Hinweis sagt es
REST = "pruef.schluessel_rest: Eine Schluesseldatei liegt noch da; apt purge loescht sie."
SCHL = "datei:/var/lib/werkbank/nutzer1/modellweg.schluessel"


def fall_remove(name, beschreibung, code=None, **abweichung):
    f = {"beschreibung": beschreibung, "form": "remove", "nutzer": NUTZER, "marke": "SCHLUESSEL-PRUEFMARKE-0001",
         "nach_installation": VOR + [SCHL], "nach_remove": VOR + [SCHL],
         "meldung": "Entferne werkbank ...\n" + REST + "\n", "journal": "werkbank: " + REST + "\n"}
    f.update(abweichung)
    fall("remove-" + name, f, {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_remove("gueltig", "nach `apt remove` liegt die Schlüsseldatei noch da (Nutzerdaten) und der Hinweis nennt es mit dem festen Satz, ohne Inhalt")
fall_remove("gueltig-ohne-schluessel", "nie ein Schlüssel eingegeben: nichts liegt da und es gibt keinen Hinweis",
            nach_installation=VOR, nach_remove=VOR, meldung="Entferne werkbank ...\n", journal="")
fall_remove("schluessel-verloren", "`remove` hat die Schlüsseldatei gelöscht (das tut nur `purge`)", "remove_schluessel_verloren", nach_remove=list(VOR))
fall_remove("hinweis-fehlt", "die Datei liegt da, aber die Meldung nennt es nicht", "remove_hinweis_fehlt", meldung="Entferne werkbank ...\n")
fall_remove("hinweis-mit-inhalt-in-meldung", "der Schlüssel steht in der Meldung", "remove_hinweis_inhalt", meldung="Entferne werkbank ...\n" + REST + "\nSCHLUESSEL-PRUEFMARKE-0001\n")
fall_remove("hinweis-mit-inhalt-im-journal", "der Schlüssel steht im Journal", "remove_hinweis_inhalt", journal="werkbank: SCHLUESSEL-PRUEFMARKE-0001\n")
fall_remove("reihenfolge-verloren-vor-hinweis", "Datei gelöscht und kein Hinweis: `remove_schluessel_verloren` zuerst", "remove_schluessel_verloren",
            nach_remove=list(VOR), meldung="")

# Nachtrag Halter (23.11.): mit `heimwurzel` gilt dasselbe für `anmeldung.copilot` in der Ablage des Halters.
ANM = "datei:/srv/beispiel/heim/nutzer1/.local/state/agent-workbench/halter/geheim/anmeldung.copilot"
CREST = "pruef.copilot_rest: Eine Anmeldedatei der Copilot CLI liegt noch da; apt purge loescht sie."
fall_remove("ablage-gueltig", "Nachtrag Halter: nach `apt remove` liegen Schlüsseldatei und anmeldung.copilot noch da, beide festen Sätze stehen in der Meldung",
            heimwurzel="/srv/beispiel/heim", nach_installation=VOR + [SCHL, ANM], nach_remove=VOR + [SCHL, ANM],
            meldung="Entferne werkbank ...\n" + REST + "\n" + CREST + "\n", journal="werkbank: " + REST + "\nwerkbank: " + CREST + "\n")
fall_remove("ablage-verloren", "Nachtrag Halter: `remove` hat anmeldung.copilot gelöscht (das tut nur `purge`)", "remove_schluessel_verloren",
            heimwurzel="/srv/beispiel/heim", nach_installation=VOR + [SCHL, ANM], nach_remove=VOR + [SCHL],
            meldung="Entferne werkbank ...\n" + REST + "\n")
fall_remove("ablage-hinweis-fehlt", "Nachtrag Halter: anmeldung.copilot liegt nach `remove` da, aber die Meldung nennt nur die Schlüsseldatei", "remove_hinweis_fehlt",
            heimwurzel="/srv/beispiel/heim", nach_installation=VOR + [SCHL, ANM], nach_remove=VOR + [SCHL, ANM])
fall_remove("ablage-ohne-heimwurzel-ungeprueft", "ohne `heimwurzel` prüft ein Fall nur die Schlüsseldatei (wie vor dem Nachtrag)",
            nach_installation=VOR + [SCHL, ANM], nach_remove=VOR + [SCHL])

# ------------------------------------------------------------------------------------------ Einrichten: der Schlüssel erscheint nirgends
MARKE = "SCHLUESSEL-PRUEFMARKE-0001"


def fall_einricht(name, beschreibung, code=None, **abweichung):
    f = {"beschreibung": beschreibung, "form": "einrichten", "nutzer": NUTZER, "marke": MARKE,
         "stdout": "Gateway-Adresse: \nSchluessel der IT: \nEingerichtet.\n", "stderr": "", "journal": "werkbank-probelauf: eingerichtet\n",
         "argv": "werkbank-probelauf --ergebnis /srv/beispiel/heim/nutzer1/probelauf-ergebnis.json", "umgebung": "TERM=xterm\nLANG=C.UTF-8\n",
         "protokolle": ["{\"aktion\":\"einrichten\",\"ergebnis\":\"ok\"}\n"], "ergebnisdatei": "{\"art\":\"probelauf-ergebnis\",\"fassung\":1}\n",
         "echo": False, "datei": {"modus": "0600", "eigentuemer": NUTZER, "inhalt": MARKE}}
    f.update(abweichung)
    fall("einrichten-" + name, f, {"gueltig": True} if code is None else {"gueltig": False, "code": "einricht." + code})


fall_einricht("gueltig", "erster Aufruf: der Schlüssel steht nur in der Datei 0600 des Nutzers, in keiner Ausgabe, keinem Protokoll, keiner Befehlszeile")
fall_einricht("schluessel-in-stdout", "der Schlüssel erscheint in der Ausgabe", "schluessel_in_ausgabe", stdout="Schluessel: " + MARKE + "\n")
fall_einricht("schluessel-in-stderr", "der Schlüssel erscheint in einer Fehlermeldung", "schluessel_in_ausgabe", stderr="Fehler bei " + MARKE + "\n")
fall_einricht("schluessel-im-journal", "der Schlüssel erscheint im Journal", "schluessel_in_ausgabe", journal="x " + MARKE + "\n")
fall_einricht("schluessel-in-argv", "der Schlüssel steht in der Befehlszeile", "schluessel_in_ausgabe", argv="werkbank-probelauf --schluessel " + MARKE)
fall_einricht("schluessel-in-umgebung", "der Schlüssel steht in der Umgebung des Prozesses", "schluessel_in_ausgabe", umgebung="WERKBANK_SCHLUESSEL=" + MARKE + "\n")
fall_einricht("schluessel-im-protokoll", "der Schlüssel erscheint in einer Protokollzeile", "schluessel_in_ausgabe", protokolle=["a\n", "{\"k\":\"" + MARKE + "\"}\n"])
fall_einricht("schluessel-in-ergebnisdatei", "der Schlüssel erscheint in der Ergebnisdatei des Probelaufs", "schluessel_in_ausgabe", ergebnisdatei=MARKE)
fall_einricht("echo-an", "die Eingabe des Schlüssels wird auf dem Bildschirm gezeigt", "echo_an", echo=True)
fall_einricht("datei-modus-0644", "die Datei hat den Modus 0644", "datei_modus", datei={"modus": "0644", "eigentuemer": NUTZER, "inhalt": MARKE})
fall_einricht("datei-modus-0400", "die Datei hat den Modus 0400 (nicht genau 0600)", "datei_modus", datei={"modus": "0400", "eigentuemer": NUTZER, "inhalt": MARKE})
fall_einricht("datei-eigentuemer-root", "die Datei gehört root", "datei_eigentuemer", datei={"modus": "0600", "eigentuemer": "root", "inhalt": MARKE})
fall_einricht("datei-inhalt-falsch", "die Datei enthält nicht den eingegebenen Schlüssel", "datei_inhalt", datei={"modus": "0600", "eigentuemer": NUTZER, "inhalt": "anderer-wert"})
fall_einricht("reihenfolge-ausgabe-vor-modus", "Schlüssel in der Ausgabe und falscher Modus: `schluessel_in_ausgabe` zuerst", "schluessel_in_ausgabe",
              stdout=MARKE, datei={"modus": "0644", "eigentuemer": NUTZER, "inhalt": MARKE})


# ------------------------------------------------------------------------------------------ Bauwerkzeuge
SUMME_WIX = "f30ef0c74e2a986126539c5780be93ac24e8136eaf723b1937b26272703ae173"
SUMME_DPKG_DEV = "507b526fd78f9dff89696bbc3085af3117059f37fd281542778ac8b97be44690"
SUMME_BUILD_ESSENTIAL = "e4896a79cf1d2b62c6d5be654e34a84678cc8cad344c6468419d53b614e1af2b"
BW_WINDOWS = f"wix\t5.0.2\tMS-RL\tsha256:{SUMME_WIX}\tnein\n"
BW_LINUX = (f"build-essential\t12.12ubuntu2\tGPL-2.0-or-later\tsha256:{SUMME_BUILD_ESSENTIAL}\tnein\n"
            f"dpkg-dev\t1.23.7ubuntu1\tGPL-2.0-or-later\tsha256:{SUMME_DPKG_DEV}\tnein\n")


def fall_bw(name, beschreibung, inhalt, code=None, plattform="ubuntu-26.04-x64"):
    fall("bauwerkzeuge-" + name, {"beschreibung": beschreibung, "form": "bauwerkzeuge", "text": inhalt, "plattform": plattform},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_bw("gueltig-linux", "Bauwerkzeuge des deb: dpkg-dev und build-essential, je mit Fassung, Lizenz, Prüfsumme und `nein`", BW_LINUX)
fall_bw("gueltig-windows", "Bauwerkzeug der MSI: WiX 5.0.2 unter MS-RL", BW_WINDOWS, plattform="windows-x64")
fall_bw("gueltig-macos-leer-erlaubt", "unter macOS ist keine Pflichtzeile vorgeschrieben; eine Zeile genügt",
        BW_WINDOWS, plattform="macos-arm64")
fall_bw("gueltig-weiteres-werkzeug", "ein weiteres Bauwerkzeug neben den Pflichtzeilen",
        BW_LINUX + f"dpkg-dev-extra\t1.0\tMIT\tsha256:{SUMME_DPKG_DEV}\tnein\n")
fall_bw("form-leer", "leere Liste", "", "bauwerkzeuge_form")
fall_bw("form-ohne-zeilenende", "letzte Zeile ohne LF", BW_LINUX.rstrip("\n"), "bauwerkzeuge_form")
fall_bw("form-cr", "Zeilenende CR LF", BW_LINUX.replace("\n", "\r\n"), "bauwerkzeuge_form")
fall_bw("form-vier-felder", "Zeile mit vier Feldern", BW_LINUX.replace("\tnein", "", 1), "bauwerkzeuge_form")
fall_bw("form-sechs-felder", "Zeile mit sechs Feldern", BW_LINUX.replace("\tnein\n", "\tnein\tx\n", 1), "bauwerkzeuge_form")
fall_bw("form-feld-leer", "die Fassung ist leer", BW_LINUX.replace("12.12ubuntu2", ""), "bauwerkzeuge_form")
fall_bw("form-name-gross", "Name mit Großbuchstaben", BW_LINUX.replace("build-essential", "Build-Essential"), "bauwerkzeuge_form")
fall_bw("form-summe-zu-kurz", "Prüfsumme mit 63 Zeichen", BW_LINUX.replace(SUMME_DPKG_DEV, SUMME_DPKG_DEV[:-1]), "bauwerkzeuge_form")
fall_bw("form-summe-gross", "Prüfsumme in Großbuchstaben", BW_LINUX.replace(SUMME_DPKG_DEV, SUMME_DPKG_DEV.upper()), "bauwerkzeuge_form")
fall_bw("form-summe-ohne-art", "Prüfsumme ohne `sha256:`", BW_LINUX.replace("sha256:" + SUMME_DPKG_DEV, SUMME_DPKG_DEV), "bauwerkzeuge_form")
fall_bw("form-nicht-ascii", "Zeichen außerhalb ASCII in der Lizenz", BW_LINUX.replace("GPL-2.0-or-later", "GPL-2.0-or-sp\u00e4ter", 1), "bauwerkzeuge_form")
fall_bw("doppelt", "dpkg-dev zweimal", BW_LINUX + BW_LINUX.split("\n")[1] + "\n", "bauwerkzeug_doppelt")
fall_bw("ausgeliefert-ja", "ein Bauwerkzeug mit `ja`", BW_LINUX.replace("\tnein", "\tja", 1), "bauwerkzeug_ausgeliefert")
fall_bw("ausgeliefert-gross", "`Nein` statt `nein`", BW_LINUX.replace("\tnein", "\tNein", 1), "bauwerkzeug_ausgeliefert")
fall_bw("ausgeliefert-windows", "WiX mit `ja`", BW_WINDOWS.replace("nein", "ja"), "bauwerkzeug_ausgeliefert", plattform="windows-x64")
fall_bw("fehlt-dpkg-dev", "dpkg-dev fehlt im deb", BW_LINUX.split("\n")[0] + "\n", "bauwerkzeug_fehlt")
fall_bw("fehlt-build-essential", "build-essential fehlt im deb", BW_LINUX.split("\n")[1] + "\n", "bauwerkzeug_fehlt")
fall_bw("fehlt-wix", "WiX fehlt in der Liste der MSI (nur ein deb-Werkzeug steht darin)", BW_LINUX, "bauwerkzeug_fehlt", plattform="windows-x64")
fall_bw("reihenfolge-form-vor-doppelt", "falsche Form geht vor `doppelt`", BW_LINUX + BW_LINUX.split("\n")[1] + "\n" + "x\n", "bauwerkzeuge_form")
fall_bw("reihenfolge-doppelt-vor-ausgeliefert", "`doppelt` geht vor `ausgeliefert`",
        BW_LINUX + BW_LINUX.split("\n")[1].replace("nein", "ja") + "\n", "bauwerkzeug_doppelt")
fall_bw("reihenfolge-ausgeliefert-vor-fehlt", "`ausgeliefert` geht vor `fehlt`", BW_LINUX.split("\n")[0].replace("nein", "ja") + "\n",
        "bauwerkzeug_ausgeliefert")

# ------------------------------------------------------------------------------------------ kern.json und bruecke.json (postinst)
def fall_dienst(name, beschreibung, **lage):
    f = {"beschreibung": beschreibung, "form": "dienstkonfig", "nutzer": "nutzer1", "uid": 1000}
    f.update(lage)
    d = _ref.dienstkonfig(f)
    e = {"gueltig": True, "dateien": d}
    if d is not None:
        e.update(modus="0644", eigentuemer="root:root")
        j = _ref.dienst_admin(f)[2]
        if j:
            e["journal"] = j  # the fixed journal lines of postinst (no path, no content)
    fall("dienstkonfig-" + name, f, e)


fall_dienst("nutzer", "ein Nutzer mit uid 1000: Socket-Ordner unter /run/user/1000/werkbank, die Umgebung der Sitzungen ist genau TERM, die Brücke zeigt auf denselben Socket und legt Token- und Portdatei in denselben Ordner")
fall_dienst("andere-uid", "eine andere uid und ein anderer Nutzername: alle Pfade folgen der uid, die Einstellungsdatei dem Namen", nutzer="anna-b", uid=1234)
fall_dienst("groesste-uid", "die größte uid 4294967294: der Socket-Pfad bleibt unter 100 Byte", nutzer="n", uid=4294967294)
fall_dienst("langer-name", "der längste Nutzername (32 Zeichen): er steht in `betrachter`, `anfang.personen` und im Pfad der Einstellungsdatei", nutzer="a" * 32, uid=1000)
fall_dienst("uid-null", "uid 0 (der Nutzer root wird nie genommen, die Form bleibt gültig)", nutzer="n", uid=0)
fall_dienst("kein-nutzer", "kein Nutzer (kein sudo): keine Datei", nutzer=None)
fall_dienst("keine-uid", "keine Zahl als uid: keine Datei", uid=None)

fall_dienst("sitzungen-folgt-der-uid", "die Sitzungsordner liegen unter /var/lib/werkbank/s/<uid> mit der uid des Nutzers, nicht mit dem Namen: `lauf.sitzungen` gleich `sitzungen` in lauf.json", nutzer="anna-b", uid=77)
fall_dienst("marker-folgen-der-uid", "die Marker-Datei des Probelaufs liegt unter /run/user/<uid>/werkbank-probelauf und der Zaehlordner unter /var/lib/werkbank/z/<uid>, beide mit der uid des Nutzers, nicht mit dem Namen", nutzer="anna-b", uid=77)

fall_dienst("ziellisten-folgen-dem-nutzernamen", "`lauf.ziellisten` ist der Ordner der Ziellisten, `<zustand>/sitzungen` von lauf.json: der Zustandsordner des Nutzers (nach dem Namen, nicht nach der uid) plus /sitzungen; er ist nicht `lauf.sitzungen` (/var/lib/werkbank/s/<uid>)", nutzer="anna-b", uid=77)

fall_dienst("bestand-folgt-dem-nutzernamen", "`lauf.bestand` ist die Datei der Sitzungsliste des Kerns: der Zustandsordner des Nutzers (nach dem Namen, nicht nach der uid) plus /dienst/bestand.json; sie liegt nicht in `lauf.sitzungen`, `lauf.ziellisten` oder `lauf.laufzeit`", nutzer="anna-b", uid=77)

fall_dienst("gespraeche-folgt-dem-nutzernamen", "`lauf.gespraeche` ist der Ordner der Gespräche: der Zustandsordner des Nutzers (nach dem Namen, nicht nach der uid) plus /gespraeche, genau `gespraeche` von lauf.json; er liegt nicht in `lauf.sitzungen`, `lauf.ziellisten` oder `lauf.bestand`", nutzer="anna-b", uid=77)

# Admin fields (Nachtrag 21.11.): `lauf.groesse` of kern.json and `port` of bruecke.json survive an update, read as JSON text like `ca_zulassung`
G_UNGUELTIG = "werkbank: kern.json: groesse ungueltig, die Vorgabe gilt"
P_UNGUELTIG = "werkbank: bruecke.json: port ungueltig, die Vorgabe gilt"
fall_dienst("admin-groesse-bleibt", "der Admin setzte lauf.groesse auf [200, 50]: ein Update schreibt denselben Wert wieder (Leerzeichen im Paar fallen nicht ins Gewicht)",
            vorher_kern='{"lauf":{"programm":"/x","groesse": [ 200 , 50 ],"env":{}}}\n')
fall_dienst("admin-port-bleibt", "der Admin setzte port auf 8765: ein Update schreibt ihn wieder", vorher_bruecke='{"port":8765,"deflate":true}\n')
fall_dienst("admin-umgeschrieben-zeilen", "beide Dateien wurden mit einem anderen Werkzeug neu geschrieben (Zeilen, Tabulator, CR LF, Leerzeichen um den Doppelpunkt): beide Werte bleiben",
            vorher_kern='{\r\n\t"lauf": {\r\n\t\t"groesse"\t:\r\n\t\t[ 132 ,\n 43 ]\r\n\t}\r\n}\r\n', vorher_bruecke='{\n "dienst": {},\n "port"\n :\n 9001\n}\n')
fall_dienst("admin-beide-bleiben", "beide Admin-Felder gesetzt: beide bleiben, jedes andere Feld ist wieder das des Pakets (der Admin änderte auch deflate und das Programm von Hand: das gilt nicht)",
            vorher_kern='{"deflate":false,"lauf":{"programm":"/opt/x","groesse":[100,30]}}', vorher_bruecke='{"deflate":false,"port":4000,"token_datei":"/x"}')
fall_dienst("admin-ohne-feld", "die alte Datei hat die Felder nicht (ein früherer Stand des Pakets): die Vorgabe gilt, keine Journalzeile",
            vorher_kern='{"lauf":{"programm":"/x"}}', vorher_bruecke='{"deflate":true}')
fall_dienst("admin-groesse-zu-klein", "lauf.groesse hat weniger als 20 Spalten: die Vorgabe und eine Journalzeile", vorher_kern='{"lauf":{"groesse":[10,40]}}')
fall_dienst("admin-groesse-doppelt", "der Schlüssel groesse steht zweimal: ungültig, Vorgabe und eine Journalzeile", vorher_kern='{"lauf":{"groesse":[100,30],"x":{"groesse":[90,20]}}}')
fall_dienst("admin-groesse-zahl-form", "lauf.groesse mit einer führenden Null in einer Zahl: ungültig, Vorgabe und eine Journalzeile", vorher_kern='{"lauf":{"groesse":[0120,40]}}')
fall_dienst("admin-port-zu-gross", "port 70000 liegt über 65535: die Vorgabe 0 und eine Journalzeile", vorher_bruecke='{"port":70000}')
fall_dienst("admin-port-als-text", "port als Text (\"8765\"): ungültig, Vorgabe 0 und eine Journalzeile", vorher_bruecke='{"port":"8765"}')
fall_dienst("admin-port-doppelt", "der Schlüssel port steht zweimal: ungültig, Vorgabe 0 und eine Journalzeile", vorher_bruecke='{"port":8000,"x":{"port":9000}}')
fall_dienst("admin-port-obergrenze-bleibt", "port 65535 ist der größte gültige Wert und bleibt", vorher_bruecke='{"port":65535}')
fall_dienst("admin-groesse-grenzen-bleiben", "lauf.groesse [1000, 1000] (obere Grenzen) bleibt", vorher_kern='{"lauf":{"groesse":[1000,1000]}}')
fall_dienst("admin-groesse-untere-grenzen-bleiben", "lauf.groesse [20, 5] (untere Grenzen) bleibt", vorher_kern='{"lauf":{"groesse":[20,5]}}')
fall_dienst("admin-groesse-zu-gross", "lauf.groesse mit 1001 Spalten: Vorgabe und eine Journalzeile", vorher_kern='{"lauf":{"groesse":[1001,40]}}')
fall_dienst("admin-groesse-zeilen-zu-klein", "lauf.groesse mit 4 Zeilen: Vorgabe und eine Journalzeile", vorher_kern='{"lauf":{"groesse":[120,4]}}')
fall_dienst("admin-port-doppelt-zweiter-ohne-treffer", "der Schlüssel port steht zweimal, der zweite mit einem Text als Wert: der Schlüssel zählt, ungültig, Vorgabe 0 und eine Journalzeile",
            vorher_bruecke='{"port":8000,"x":{"port":"9000"}}')
fall_dienst("admin-groesse-doppelt-zweiter-ohne-treffer", "der Schlüssel groesse steht zweimal, der zweite mit einem Text als Wert: ungültig, Vorgabe und eine Journalzeile",
            vorher_kern='{"lauf":{"groesse":[100,30],"x":{"groesse":"90x20"}}}')
fall_dienst("admin-port-null-bleibt", "port 0 (das System wählt) ist ein gültiger Wert und bleibt", vorher_bruecke='{"port":0}')

# ------------------------------------------------------------------------------------------ Einheit des Kern-Dienstes (NoNewPrivileges)
KERN_EINHEIT_GUT = (
    "[Unit]\nDescription=Werkbank core service\n\n[Service]\nType=simple\nExecStart=/usr/lib/werkbank/werkbank-kern /etc/werkbank/kern.json\n"
    "RuntimeDirectory=werkbank\nRuntimeDirectoryMode=0700\n\n[Install]\nWantedBy=default.target\n")


def fall_kerneinheit(name, beschreibung, zusatz, code=None, text=None):
    t = KERN_EINHEIT_GUT if text is None else text
    t = t.replace("[Service]\n", "[Service]\n" + zusatz, 1)
    fall("kerneinheit-" + name, {"beschreibung": beschreibung, "form": "kerneinheit", "text": t},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


NNP = "kern_einheit_nonewprivs"
fall_kerneinheit("gueltig", "die Einheit ohne NoNewPrivileges und ohne Option, die es nach sich zieht", "")
fall_kerneinheit("gueltig-ausdruecklich-nein", "`NoNewPrivileges=no` ist erlaubt (es steht nur als Aussage da)", "NoNewPrivileges=no\n")
fall_kerneinheit("nonewprivs-yes", "`NoNewPrivileges=yes`: der Wechsel des Starters nach bwrap wird abgewiesen", "NoNewPrivileges=yes\n", NNP)
fall_kerneinheit("nonewprivs-true", "`NoNewPrivileges=true` zählt wie `yes`", "NoNewPrivileges=true\n", NNP)
fall_kerneinheit("nonewprivs-1-und-gross", "`NoNewPrivileges=1` und `NoNewPrivileges=YES` zählen", "NoNewPrivileges=YES\n", NNP)
fall_kerneinheit("nonewprivs-nein-danach-ja", "der letzte Wert gilt: erst `no`, dann `yes`", "NoNewPrivileges=no\nNoNewPrivileges=yes\n", NNP)
fall_kerneinheit("nonewprivs-ja-danach-nein", "der letzte Wert gilt: erst `yes`, dann `no` ist erlaubt", "NoNewPrivileges=yes\nNoNewPrivileges=no\n")
fall_kerneinheit("nonewprivs-auskommentiert", "eine auskommentierte Zeile zählt nicht", "# NoNewPrivileges=yes\n; NoNewPrivileges=yes\n")
fall_kerneinheit("folge-adressfamilien", "`RestrictAddressFamilies=` zieht NoNewPrivileges nach sich (Nutzer-Manager)", "RestrictAddressFamilies=AF_UNIX\n", NNP)
fall_kerneinheit("folge-systemaufruf-filter", "`SystemCallFilter=` zieht NoNewPrivileges nach sich", "SystemCallFilter=@system-service\n", NNP)
fall_kerneinheit("folge-leerer-wert-setzt-zurueck", "ein leerer Wert setzt die Option zurück", "RestrictAddressFamilies=AF_UNIX\nRestrictAddressFamilies=\n")
fall_kerneinheit("folge-sperre-namensraeume", "`RestrictNamespaces=yes` zieht NoNewPrivileges nach sich", "RestrictNamespaces=yes\n", NNP)
fall_kerneinheit("mit-anderer-haertung-ohne-folge", "`ProtectHome=read-only` zieht NoNewPrivileges nicht nach sich und ist kein Befund dieses Falls", "ProtectHome=read-only\n")


# ------------------------------------------------------------------------------------------ Schritt II: Systemnutzer wb-halter und drei Rollengruppen
# Entscheid des Betreibers, 23.11.2026: Entscheidung 4 ist für wb-halter und drei Rollengruppen aufgehoben; purge nach Weg a.
# Die Erwartungen stehen hier von Hand, unabhängig von referenz.py.
SYS_KONTEN = ["konto:wb-halter", "gruppe:wb-halter", "gruppe:wb-hub", "gruppe:wb-bruecke", "gruppe:wb-oberflaeche"]

# Stufe 9 mit setgid-Liste (Form `liste`, Feld `schritt` 2, dazu `gruppen`: Pfad -> Gruppe im gestuften Baum)
MG = "usr/lib/werkbank/werkbank-modellweg"


def fall_setgid(name, beschreibung, aenderung, gruppen, code=None, schritt=2):
    m = dict(MODI_GUT)
    m[MG] = "2755"
    m.update(aenderung)
    eintrag = {"beschreibung": beschreibung, "form": "liste", "liste": LISTE_LINUX, "ausschluss": AUSSCHLUSS_LINUX,
               "quellen": QUELLEN_LINUX, "baum": BAUM_LINUX, "modi": m, "gruppen": gruppen}
    if schritt is not None:
        eintrag["schritt"] = schritt
    fall("liste-setgid-" + name, eintrag, {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_setgid("gueltig", "Schritt II: das Rollenprogramm der Brücke ist root:wb-bruecke 2755, alles andere ohne setgid", {}, {MG: "wb-bruecke"})
fall_setgid("ausserhalb-liste", "Schritt II: `werkbank-pruefen` trägt setgid, steht aber nicht in der setgid-Liste", {"usr/bin/werkbank-pruefen": "2755"},
            {MG: "wb-bruecke", "usr/bin/werkbank-pruefen": "wb-hub"}, "modus_setgid")
fall_setgid("ordner-ausserhalb-liste", "Schritt II: ein Ordner mit setgid (2755) steht nicht in der Liste", {"usr/lib/werkbank/": "2755"},
            {MG: "wb-bruecke"}, "modus_setgid")
fall_setgid("falsche-gruppe", "Schritt II: das Rollenprogramm der Brücke trägt die Gruppe wb-hub statt wb-bruecke", {}, {MG: "wb-hub"}, "modus_setgid")
fall_setgid("ohne-gruppe", "Schritt II: das Rollenprogramm trägt setgid, der Baum nennt aber keine Gruppe", {}, {}, "modus_setgid")
fall_setgid("falscher-modus", "Schritt II: 2750 statt genau 2755", {MG: "2750"}, {MG: "wb-bruecke"}, "modus_setgid")
fall_setgid("fehlt-an-listenpfad", "Schritt II: der Pfad der Liste liegt im Baum, aber ohne setgid (0755)", {MG: "0755"}, {MG: "wb-bruecke"}, "modus_setgid")
fall_setgid("setuid-nie", "Schritt II: setuid (4755) ist nie erlaubt, auch nicht an einem Pfad der Liste", {MG: "4755"}, {MG: "wb-bruecke"}, "modus_setgid")
fall_setgid("sticky-nie", "Schritt II: das Sticky-Bit an einem Ordner ist nie erlaubt", {"usr/lib/werkbank/": "1755"}, {MG: "wb-bruecke"}, "modus_setgid")
fall_setgid("schreibbar-vor-setgid", "Schritt II: 2775 ist für die Gruppe schreibbar; der erste Teil von Stufe 9 meldet zuerst", {MG: "2775"}, {MG: "wb-bruecke"},
            "modus_schreibbar")
fall_setgid("schritt1-unveraendert", "Schritt I (ohne `schritt`): die setgid-Liste gilt nicht, Stufe 9 prüft nur Schreibrechte (unverändert)",
            {"usr/bin/werkbank-pruefen": "2755"}, {}, None, schritt=None)

# Installation: preinst legt Konto und Gruppen an (Form `systemnutzer`)
PERSON = {"name": "nutzer1", "uid": 1000, "gruppe": "nutzer1", "heim": "/srv/beispiel/heim/nutzer1", "shell": "/bin/bash"}
HALTER_KONTO = {"name": "wb-halter", "uid": 998, "gruppe": "wb-halter", "heim": "/nonexistent", "shell": "/usr/sbin/nologin"}
GRUPPEN_VOR = [{"name": "nutzer1", "gid": 1000, "mitglieder": []}, {"name": "sudo", "gid": 27, "mitglieder": ["nutzer1"]}]
SYS_GRUPPEN = [{"name": "wb-halter", "gid": 998, "mitglieder": []}, {"name": "wb-hub", "gid": 997, "mitglieder": []},
               {"name": "wb-bruecke", "gid": 996, "mitglieder": []}, {"name": "wb-oberflaeche", "gid": 995, "mitglieder": []}]


def stand(konten, gruppen):
    return {"konten": copy.deepcopy(konten), "gruppen": copy.deepcopy(gruppen)}


def fall_sysnutzer(name, beschreibung, code=None, vorher=None, nachher=None, rc=0):
    fall("systemnutzer-" + name, {"beschreibung": beschreibung, "form": "systemnutzer",
                                  "vorher": stand([PERSON], GRUPPEN_VOR) if vorher is None else vorher,
                                  "nachher": stand([PERSON, HALTER_KONTO], GRUPPEN_VOR + SYS_GRUPPEN) if nachher is None else nachher,
                                  "preinst_rc": rc},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


def nach(aendern):
    s = stand([PERSON, HALTER_KONTO], GRUPPEN_VOR + SYS_GRUPPEN)
    aendern(s)
    return s


def gruppe(s, name):
    return next(g for g in s["gruppen"] if g["name"] == name)


def konto(s, name):
    return next(k for k in s["konten"] if k["name"] == name)


fall_sysnutzer("gueltig-installation", "Installation: preinst legt das Systemkonto wb-halter (eigene Gruppe, /nonexistent, nologin) und die drei Rollengruppen an")
fall_sysnutzer("gueltig-update-bestehend", "Update: Konto und Gruppen bestehen schon in der richtigen Form und bleiben unverändert",
               vorher=stand([PERSON, HALTER_KONTO], GRUPPEN_VOR + SYS_GRUPPEN))
fall_sysnutzer("konto-fehlt", "preinst endet mit 0, aber das Konto wb-halter fehlt", "konto_fehlt", nachher=stand([PERSON], GRUPPEN_VOR + SYS_GRUPPEN))
fall_sysnutzer("gruppe-fehlt", "preinst endet mit 0, aber die Rollengruppe wb-oberflaeche fehlt", "konto_fehlt",
               nachher=nach(lambda s: s["gruppen"].remove(gruppe(s, "wb-oberflaeche"))))
fall_sysnutzer("rc-ohne-grund", "preinst endet mit 1, obwohl kein fremdes Konto im Weg ist", "konto_fehlt", rc=1)
fall_sysnutzer("konto-form-shell", "das Konto wb-halter hat eine Anmelde-Shell", "konto_form", nachher=nach(lambda s: konto(s, "wb-halter").update(shell="/bin/bash")))
fall_sysnutzer("konto-form-uid", "das Konto wb-halter liegt außerhalb des Bereichs der Systemkonten", "konto_form",
               nachher=nach(lambda s: konto(s, "wb-halter").update(uid=1001)))
fall_sysnutzer("konto-form-heim", "das Konto wb-halter hat einen Heimordner statt /nonexistent", "konto_form",
               nachher=nach(lambda s: konto(s, "wb-halter").update(heim="/var/lib/wb-halter")))
fall_sysnutzer("konto-form-hauptgruppe", "das Konto wb-halter hat nogroup statt der eigenen Gruppe", "konto_form",
               nachher=nach(lambda s: konto(s, "wb-halter").update(gruppe="nogroup")))
fall_sysnutzer("gruppe-form-gid", "die Rollengruppe wb-hub liegt außerhalb des Bereichs der Systemgruppen", "konto_form",
               nachher=nach(lambda s: gruppe(s, "wb-hub").update(gid=1002)))
fall_sysnutzer("mitglied-person", "die Person ist Mitglied der Rollengruppe wb-hub (sie bekäme die Rolle ohne Rollenprogramm)", "rollengruppe_mitglied",
               nachher=nach(lambda s: gruppe(s, "wb-hub").update(mitglieder=["nutzer1"])))
fall_sysnutzer("mitglied-halter", "das Konto wb-halter ist Mitglied der Rollengruppe wb-bruecke", "rollengruppe_mitglied",
               nachher=nach(lambda s: gruppe(s, "wb-bruecke").update(mitglieder=["wb-halter"])))
FREMD = {"name": "wb-halter", "uid": 1001, "gruppe": "wb-halter", "heim": "/srv/beispiel/heim/wb-halter", "shell": "/bin/bash"}
fall_sysnutzer("fremd-konto-abbruch", "ein Mensch heißt schon wb-halter: preinst endet mit 24 und ändert nichts (fail closed)",
               vorher=stand([PERSON, FREMD], GRUPPEN_VOR), nachher=stand([PERSON, FREMD], GRUPPEN_VOR), rc=24)
fall_sysnutzer("fremd-konto-uebernommen", "ein Mensch heißt schon wb-halter, und preinst übernimmt das Konto", "konto_fremd",
               vorher=stand([PERSON, FREMD], GRUPPEN_VOR), nachher=stand([PERSON, FREMD], GRUPPEN_VOR + SYS_GRUPPEN))
FREMD_GRUPPE = {"name": "wb-hub", "gid": 997, "mitglieder": ["nutzer1"]}
fall_sysnutzer("fremd-gruppe-abbruch", "eine Gruppe wb-hub mit Mitgliedern besteht schon: preinst endet mit 24 und ändert nichts",
               vorher=stand([PERSON], GRUPPEN_VOR + [FREMD_GRUPPE]), nachher=stand([PERSON], GRUPPEN_VOR + [FREMD_GRUPPE]), rc=24)
fall_sysnutzer("fremd-gruppe-uebernommen", "eine Gruppe wb-hub mit Mitgliedern besteht schon, und preinst legt den Rest daneben an", "konto_fremd",
               vorher=stand([PERSON], GRUPPEN_VOR + [FREMD_GRUPPE]),
               nachher=stand([PERSON, HALTER_KONTO], GRUPPEN_VOR + [FREMD_GRUPPE] + [g for g in SYS_GRUPPEN if g["name"] != "wb-hub"]))
fall_sysnutzer("verloren-fremde-gruppe", "preinst hat die Person aus der Gruppe sudo genommen", "konto_verloren",
               nachher=nach(lambda s: gruppe(s, "sudo").update(mitglieder=[])))
fall_sysnutzer("form-feld-fehlt", "ein Konto ohne das Feld shell", "systemnutzer_form",
               nachher=nach(lambda s: konto(s, "wb-halter").pop("shell")))

# Update von Schritt I auf Schritt II: Umzug der Ablage (Form `umzug`)
UMZUG_ZEILE = "werkbank: Umzug der Ablage gescheitert, Code 21"


def geheim(name, eigentuemer="nutzer1", modus="0600", inhalt=None):
    return {"name": name, "sha256": sha(inhalt or name), "modus": modus, "eigentuemer": eigentuemer}


Q2 = [geheim("anmeldung.copilot"), geheim("mcp.wiki")]
Z2 = [geheim("anmeldung.copilot", "wb-halter"), geheim("mcp.wiki", "wb-halter")]


def fall_umzug(name, beschreibung, code=None, quelle_vorher=None, quelle_nachher=None, ziel_vorher=None, ziel_nachher=None, journal="", rc=0):
    fall("umzug-" + name, {"beschreibung": beschreibung, "form": "umzug", "nutzer": "nutzer1",
                           "quelle_vorher": Q2 if quelle_vorher is None else quelle_vorher,
                           "quelle_nachher": [] if quelle_nachher is None else quelle_nachher,
                           "ziel_vorher": [] if ziel_vorher is None else ziel_vorher,
                           "ziel_nachher": Z2 if ziel_nachher is None else ziel_nachher,
                           "journal": journal, "postinst_rc": rc},
         {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_umzug("gueltig-zwei-geheimnisse", "Update: beide Geheimnisse liegen danach unter /var/lib/wb-halter (wb-halter, 0600, gleicher Inhalt), die Quelle ist leer")
fall_umzug("gueltig-ein-geheimnis", "Update mit nur anmeldung.copilot", quelle_vorher=Q2[:1], ziel_nachher=Z2[:1])
fall_umzug("gueltig-ohne-quelle", "frische Installation oder keine Ablage der Person: nichts zu tun, keine Journalzeile", quelle_vorher=[], ziel_nachher=[])
fall_umzug("gueltig-wiederholung", "ein früherer Lauf hatte das Ziel schon fertig (gleicher Inhalt); dieser Lauf räumt die Quelle", ziel_vorher=Z2)
fall_umzug("gueltig-fremdes-im-ziel-bleibt", "ein anderes Geheimnis lag schon im Ziel und bleibt", ziel_vorher=[geheim("mcp.post", "wb-halter")],
           ziel_nachher=[geheim("mcp.post", "wb-halter")] + Z2)
fall_umzug("quelle-bleibt", "das Ziel ist fertig, aber eine Datei der Quelle liegt noch im Heim", "umzug_quelle_bleibt", quelle_nachher=Q2[1:])
fall_umzug("gescheitert-quelle-bleibt", "der Umzug scheitert: Quelle und Ziel sind unverändert, das Journal nennt Code 21",
           quelle_nachher=Q2, ziel_nachher=[], journal=UMZUG_ZEILE + "\n")
fall_umzug("gescheitert-ohne-code", "der Umzug scheitert still: Quelle und Ziel sind unverändert, aber keine Journalzeile", "umzug_code_fehlt",
           quelle_nachher=Q2, ziel_nachher=[])
fall_umzug("halb-ziel", "halber Zustand: nur eines von zwei Geheimnissen liegt im Ziel, die Quelle ist unverändert", "umzug_halb",
           quelle_nachher=Q2, ziel_nachher=Z2[:1], journal=UMZUG_ZEILE + "\n")
fall_umzug("halb-quelle", "halber Zustand: das Ziel ist leer, aber eine Datei der Quelle ist schon gelöscht", "umzug_halb",
           quelle_nachher=Q2[:1], ziel_nachher=[], journal=UMZUG_ZEILE + "\n")
fall_umzug("inhalt-modus", "ein Geheimnis liegt im Ziel mit 0644", "umzug_inhalt", ziel_nachher=[Z2[0], geheim("mcp.wiki", "wb-halter", "0644")])
fall_umzug("inhalt-eigentuemer", "ein Geheimnis liegt im Ziel, gehört aber noch der Person", "umzug_inhalt", ziel_nachher=[Z2[0], geheim("mcp.wiki")])
fall_umzug("inhalt-anders", "ein Geheimnis liegt im Ziel mit anderem Inhalt", "umzug_inhalt",
           ziel_nachher=[Z2[0], geheim("mcp.wiki", "wb-halter", inhalt="anderer Inhalt")])
ALT = geheim("anmeldung.copilot", "wb-halter", inhalt="alte Anmeldung")
fall_umzug("konflikt-bleibt", "im Ziel liegt anmeldung.copilot schon mit anderem Inhalt: nichts wird geändert, Code 21",
           ziel_vorher=[ALT], quelle_nachher=Q2, ziel_nachher=[ALT], journal=UMZUG_ZEILE + "\n")
fall_umzug("konflikt-ueberschrieben", "im Ziel lag anmeldung.copilot mit anderem Inhalt, und der Umzug hat es überschrieben", "umzug_konflikt",
           ziel_vorher=[ALT])
fall_umzug("abbruch", "der gescheiterte Umzug lässt postinst mit einem Fehler enden (Entscheidung 7 verbietet das)", "umzug_abbruch",
           quelle_nachher=Q2, ziel_nachher=[], journal=UMZUG_ZEILE + "\n", rc=21)
fall_umzug("code-falsch", "der Umzug gelingt, aber das Journal meldet Code 21", "umzug_code_falsch", journal=UMZUG_ZEILE + "\n")
fall_umzug("form-name", "ein Name mit Schrägstrich ist keine Kennung", "umzug_form", quelle_vorher=[geheim("../anmeldung.copilot")])

# remove in Schritt II: Ablage, Konto und Gruppen bleiben (Form `remove`, Feld `schritt` 2)
ANM2 = "datei:/var/lib/wb-halter/geheim/anmeldung.copilot"
WIKI2 = "datei:/var/lib/wb-halter/geheim/mcp.wiki"
ABL2 = ["ordner:/var/lib/wb-halter", "ordner:/var/lib/wb-halter/geheim"]
INST2 = VOR + [SCHL, ANM2, WIKI2] + ABL2 + SYS_KONTEN
MELD2 = "Entferne werkbank ...\n" + REST + "\n" + CREST + "\n"


def fall_remove2(name, beschreibung, code=None, **abweichung):
    f = {"schritt": 2, "nach_installation": list(INST2), "nach_remove": list(INST2), "meldung": MELD2,
         "journal": "werkbank: " + REST + "\nwerkbank: " + CREST + "\n"}
    f.update(abweichung)
    fall_remove("konto-" + name, beschreibung, code, **f)


fall_remove2("gueltig", "Schritt II: nach `apt remove` liegen Ablage, Konto und Gruppen noch da, und die Meldung nennt beide Reste")
fall_remove2("verloren", "Schritt II: `remove` hat das Konto wb-halter gelöscht", "remove_konto_verloren", nach_remove=[e for e in INST2 if e != "konto:wb-halter"])
fall_remove2("gruppe-verloren", "Schritt II: `remove` hat die Rollengruppe wb-oberflaeche gelöscht", "remove_konto_verloren",
             nach_remove=[e for e in INST2 if e != "gruppe:wb-oberflaeche"])
fall_remove2("ablage-verloren", "Schritt II: `remove` hat anmeldung.copilot unter /var/lib/wb-halter gelöscht", "remove_schluessel_verloren",
             nach_remove=[e for e in INST2 if e != ANM2])
fall_remove2("anderes-geheimnis-verloren", "Schritt II: `remove` hat ein anderes Geheimnis der Ablage gelöscht", "remove_schluessel_verloren",
             nach_remove=[e for e in INST2 if e != WIKI2])
fall_remove2("hinweis-fehlt", "Schritt II: die Anmeldung liegt da, aber die Meldung nennt `pruef.copilot_rest` nicht", "remove_hinweis_fehlt",
             meldung="Entferne werkbank ...\n" + REST + "\n")
fall_remove2("reihenfolge-ablage-vor-konto", "Schritt II: Ablage und Konto gelöscht: `remove_schluessel_verloren` zuerst", "remove_schluessel_verloren",
             nach_remove=[e for e in INST2 if e not in (ANM2, "konto:wb-halter")])

# purge in Schritt II, Weg a (Form `entfernen`, Feld `schritt` 2, `rest` und `journal`)
KONTO_REST = "werkbank: Konto wb-halter und Rollengruppen bleiben, Code 23"
REST_DATEI = "/srv/beispiel/daten/bericht.txt"


def fall_purge2(name, beschreibung, nachher, code=None, rest=None, journal="", **mounts):
    eintrag = {"beschreibung": beschreibung, "form": "entfernen", "schritt": 2, "nutzer": NUTZER, "heimwurzel": "/srv/beispiel/heim",
               "ergebnisdatei": ERGEBNIS, "vorher": list(VOR), "nachher": nachher,
               "rest": {"dateien": [], "prozesse": []} if rest is None else rest, "journal": journal}
    eintrag.update(mounts)  # nur die Fälle `entfernen-konto-mount-*` tragen einhaengepunkte, findmnt_fehler und alte_regel
    fall("entfernen-konto-" + name, eintrag, {"gueltig": True} if code is None else {"gueltig": False, "code": "paket." + code})


fall_purge2("gueltig-weg", "Schritt II: ohne Rest löscht `purge` Ablage, Konto und Gruppen; es bleibt nichts", list(VOR))
fall_purge2("bleibt-ohne-rest", "Schritt II: ohne Rest steht das Konto wb-halter nach `purge` noch da", VOR + ["konto:wb-halter"], "entfernen_rueckstand")
fall_purge2("gruppe-bleibt-ohne-rest", "Schritt II: ohne Rest steht die Rollengruppe wb-hub nach `purge` noch da", VOR + ["gruppe:wb-hub"], "entfernen_rueckstand")
fall_purge2("ablage-bleibt", "Schritt II: `/var/lib/wb-halter` steht nach `purge` noch da", VOR + ["ordner:/var/lib/wb-halter"], "entfernen_rueckstand")
fall_purge2("ablage-geheimnis-bleibt", "Schritt II: ein Geheimnis unter `/var/lib/wb-halter/geheim` steht nach `purge` noch da",
            VOR + ["ordner:/var/lib/wb-halter", "ordner:/var/lib/wb-halter/geheim", ANM2], "entfernen_rueckstand")
fall_purge2("einheit-bleibt", "Schritt II: die System-Einheit wb-halter.service ist nach `purge` noch bekannt", VOR + ["einheit:wb-halter.service"], "entfernen_rueckstand")
fall_purge2("rest-datei-bleibt", "Schritt II, Weg a mit Rest: eine Datei mit der uid von wb-halter liegt außerhalb der Ablage; Konto und Gruppen bleiben, Code 23 im Journal",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI], rest={"dateien": [REST_DATEI], "prozesse": []}, journal=KONTO_REST + "\n")
fall_purge2("rest-prozess-bleibt", "Schritt II, Weg a mit Rest: ein fremder Prozess läuft unter wb-halter; Konto und Gruppen bleiben, Code 23 im Journal",
            VOR + SYS_KONTEN + ["prozess:sleep"], rest={"dateien": [], "prozesse": ["sleep"]}, journal=KONTO_REST + "\n")
fall_purge2("trotz-rest-geloescht", "Schritt II, Weg a: es gibt eine Datei mit der uid, aber `purge` hat das Konto gelöscht (die uid würde neu vergeben)",
            VOR + ["datei:" + REST_DATEI], "entfernen_konto_trotz_rest", rest={"dateien": [REST_DATEI], "prozesse": []}, journal=KONTO_REST + "\n")
fall_purge2("gruppe-trotz-rest-geloescht", "Schritt II, Weg a: es gibt einen Rest, aber `purge` hat die Rollengruppe wb-bruecke gelöscht",
            VOR + [e for e in SYS_KONTEN if e != "gruppe:wb-bruecke"] + ["datei:" + REST_DATEI], "entfernen_konto_trotz_rest",
            rest={"dateien": [REST_DATEI], "prozesse": []}, journal=KONTO_REST + "\n")
fall_purge2("rest-ohne-meldung", "Schritt II, Weg a: Konto und Gruppen bleiben wegen eines Rests, aber das Journal nennt Code 23 nicht",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI], "entfernen_rest_ohne_meldung", rest={"dateien": [REST_DATEI], "prozesse": []})
fall_purge2("rest-halter-prozess", "Schritt II: der Halter selbst läuft noch; das ist ein Rückstand, kein Rest", VOR + SYS_KONTEN + ["prozess:werkbank-halter"],
            "entfernen_rueckstand", rest={"dateien": [], "prozesse": ["werkbank-halter"]}, journal=KONTO_REST + "\n")
fall_purge2("rest-in-ablage", "Schritt II: eine Datei mit der uid liegt noch in der Ablage; das ist ein Rückstand, kein Rest",
            VOR + SYS_KONTEN + ["datei:/var/lib/wb-halter/geheim/anmeldung.copilot"], "entfernen_rueckstand",
            rest={"dateien": ["/var/lib/wb-halter/geheim/anmeldung.copilot"], "prozesse": []}, journal=KONTO_REST + "\n")
fall_purge2("rest-zweite-datei", "Schritt II, Weg a: nur die gemeldeten Reste bleiben; eine zweite, nicht gemeldete Datei ist ein Rückstand",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI, "datei:/srv/beispiel/daten/zweite.txt"], "entfernen_rueckstand",
            rest={"dateien": [REST_DATEI], "prozesse": []}, journal=KONTO_REST + "\n")
fall_purge2("rest-form", "Schritt II: ein Rest mit relativem Pfad", VOR + SYS_KONTEN, "entfernen_form", rest={"dateien": ["relativ"], "prozesse": []})


# purge in Schritt II: die Suche nach Resten geht über jeden lokalen Einhängepunkt (Teil A2, `purge` Schritt 3; Felder einhaengepunkte, findmnt_fehler, alte_regel)
def punkt(pfad, typ, dateien=(), lesbar=True):
    return {"pfad": pfad, "typ": typ, "lesbar": lesbar, "dateien": list(dateien)}


M_WURZEL = punkt("/", "ext4")
M_PSEUDO = [punkt("/proc", "proc"), punkt("/sys", "sysfs"), punkt("/dev", "devtmpfs"), punkt("/run", "tmpfs"), punkt("/sys/fs/cgroup", "cgroup2"),
            punkt("/srv/beispiel/snap/kern", "squashfs"), punkt("/srv/beispiel/netz", "nfs4"), punkt("/srv/beispiel/freigabe", "cifs"),
            punkt("/srv/beispiel/lauf/sitzung/doc", "fuse.portal", lesbar=False), punkt("/boot/efi", "vfat")]
M_ZWEITE = punkt("/srv/beispiel/daten", "ext4", [REST_DATEI])
M_OHNE_REST = [M_WURZEL] + M_PSEUDO + [punkt("/srv/beispiel/daten", "ext4")]
fall_purge2("mount-rest-zweite-partition", "Schritt II, Weg a: die Datei mit der uid liegt auf einer zweiten Partition; die Suche über jeden Einhängepunkt findet sie, Konto und Gruppen bleiben, Code 23",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI], journal=KONTO_REST + "\n", einhaengepunkte=[M_WURZEL] + M_PSEUDO[:5] + [M_ZWEITE], alte_regel="rot")
fall_purge2("mount-rest-zweite-partition-nur-wurzel-geloescht", "Gegenprobe: dieselbe Lage, aber `purge` suchte nur mit `find / -xdev`, fand nichts und löschte Konto und Gruppen; die ersetzte Regel urteilt diesen Fall grün",
            VOR + ["datei:" + REST_DATEI], "entfernen_konto_trotz_rest", einhaengepunkte=[M_WURZEL] + M_PSEUDO[:5] + [M_ZWEITE], alte_regel="gruen")
fall_purge2("mount-rest-zweite-partition-ohne-meldung", "Schritt II: Rest auf der zweiten Partition, Konto und Gruppen bleiben, aber das Journal nennt Code 23 nicht",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI], "entfernen_rest_ohne_meldung", einhaengepunkte=[M_WURZEL, M_ZWEITE])
fall_purge2("mount-rest-auf-zwei-partitionen", "Schritt II: je eine Datei mit der uid auf zwei Partitionen; beide sind gemeldet, Konto und Gruppen bleiben",
            VOR + SYS_KONTEN + ["datei:" + REST_DATEI, "datei:/srv/beispiel/ablage/nutzer2/notiz.txt"], journal=KONTO_REST + "\n",
            einhaengepunkte=[M_WURZEL, M_ZWEITE, punkt("/srv/beispiel/ablage", "xfs", ["/srv/beispiel/ablage/nutzer2/notiz.txt"])], alte_regel="rot")
fall_purge2("mount-rest-in-ablage-eigene-partition", "Schritt II: `/var/lib/wb-halter` liegt auf eigener Partition und enthält noch eine Datei; das ist ein Rückstand, kein Rest",
            VOR + SYS_KONTEN + ["datei:/var/lib/wb-halter/geheim/anmeldung.copilot"], "entfernen_rueckstand", journal=KONTO_REST + "\n",
            einhaengepunkte=[M_WURZEL, punkt("/var/lib/wb-halter", "ext4", ["/var/lib/wb-halter/geheim/anmeldung.copilot"])])
fall_purge2("mount-nicht-lesbar-bleibt", "Schritt II: ein Einhängepunkt ist nicht lesbar; das gilt als Rest (fail closed), Konto und Gruppen bleiben, Code 23",
            VOR + SYS_KONTEN, journal=KONTO_REST + "\n", einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/platte", "ext4", lesbar=False)], alte_regel="rot")
fall_purge2("mount-nicht-lesbar-geloescht", "Schritt II: ein Einhängepunkt ist nicht lesbar, aber `purge` hat das Konto gelöscht",
            list(VOR), "entfernen_konto_trotz_rest", einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/platte", "ext4", lesbar=False)], alte_regel="gruen")
fall_purge2("mount-unbekannter-typ-bleibt", "Schritt II: ein Einhängepunkt mit einem Typ, der in keiner der beiden Listen steht; das gilt als Rest (fail closed)",
            VOR + SYS_KONTEN, journal=KONTO_REST + "\n", einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/neu", "bcachefs")], alte_regel="rot")
fall_purge2("mount-unbekannter-typ-geloescht", "Schritt II: ein Einhängepunkt mit unbekanntem Typ, aber `purge` hat das Konto gelöscht",
            list(VOR), "entfernen_konto_trotz_rest", einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/neu", "bcachefs")], alte_regel="gruen")
fall_purge2("mount-findmnt-fehler-bleibt", "Schritt II: `findmnt` scheitert, die Liste fehlt; das gilt als Rest (fail closed), Konto und Gruppen bleiben, Code 23",
            VOR + SYS_KONTEN, journal=KONTO_REST + "\n", einhaengepunkte=[], findmnt_fehler=True, alte_regel="rot")
fall_purge2("mount-findmnt-fehler-geloescht", "Schritt II: `findmnt` scheitert, aber `purge` hat das Konto gelöscht",
            list(VOR), "entfernen_konto_trotz_rest", einhaengepunkte=[], findmnt_fehler=True, alte_regel="gruen")
fall_purge2("mount-nur-pseudo-ohne-rest-weg", "Schritt II: neben der Wurzel nur Pseudo-, Netz- und Abbild-Dateisysteme und die Systempartition, kein Rest: Konto und Gruppen sind weg, kein Code 23",
            list(VOR), einhaengepunkte=M_OHNE_REST, alte_regel="gruen")
fall_purge2("mount-nur-pseudo-ohne-rest-konto-bleibt", "Schritt II: dieselbe Lage ohne Rest, aber Konto und Gruppen stehen noch da; das ist ein Rückstand",
            VOR + SYS_KONTEN, "entfernen_rueckstand", einhaengepunkte=M_OHNE_REST)
fall_purge2("mount-pseudo-nicht-lesbar-weg", "Schritt II: ein ausgenommener Typ (Sitzungsansicht von FUSE) ist nicht lesbar; das zählt nicht als Rest, Konto und Gruppen sind weg",
            list(VOR), einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/lauf/sitzung/doc", "fuse.portal", lesbar=False), punkt("/srv/beispiel/lauf/sitzung/gvfs", "fuse.gvfsd-fuse", lesbar=False)])
fall_purge2("mount-form-datei-auf-pseudo", "Schritt II: der Fall nennt eine Datei auf einem ausgenommenen Typ; `purge` kann sie nicht finden, der Fall darf sie nicht behaupten",
            VOR + SYS_KONTEN, "entfernen_form", einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/ram", "tmpfs", ["/srv/beispiel/ram/x.txt"])])
fall_purge2("mount-form-datei-ausserhalb", "Schritt II: die Datei liegt nicht unter ihrem Einhängepunkt", VOR + SYS_KONTEN, "entfernen_form",
            einhaengepunkte=[M_WURZEL, punkt("/srv/beispiel/daten", "ext4", ["/srv/beispiel/andere/x.txt"])])
fall_purge2("mount-form-wurzel-fehlt", "Schritt II: die Liste der Einhängepunkte hat keine Wurzel, obwohl `findmnt` nicht gescheitert ist", list(VOR), "entfernen_form",
            einhaengepunkte=[punkt("/srv/beispiel/daten", "ext4")])
fall_purge2("mount-form-doppelt", "Schritt II: derselbe Einhängepunkt steht zweimal in der Liste", list(VOR), "entfernen_form",
            einhaengepunkte=[M_WURZEL, M_WURZEL])
fall_purge2("mount-form-fehler-mit-liste", "Schritt II: `findmnt` ist gescheitert, aber der Fall bringt eine Liste mit", list(VOR), "entfernen_form",
            einhaengepunkte=[M_WURZEL], findmnt_fehler=True)
fall_purge2("mount-form-rest-dateien-doppelt", "Schritt II: Dateien stehen zugleich in `rest` und in der Liste der Einhängepunkte", VOR + SYS_KONTEN + ["datei:" + REST_DATEI],
            "entfernen_form", rest={"dateien": [REST_DATEI], "prozesse": []}, journal=KONTO_REST + "\n", einhaengepunkte=[M_WURZEL, M_ZWEITE])


# Ausstehende Fälle: keine mehr. Die Fälle des Nachtrags Halter, die erst der Code der Crates bestand, stehen seit dessen Zug in
# `faelle/` und `erwartet/`; die Ordner `faelle-ausstehend/` und `erwartet-ausstehend/` sind entfernt. Ein Name hier hieße wieder ein
# solches Ordnerpaar mit README.md.
AUSSTEHEND = set()


def schreiben():
    paare = (("faelle", "erwartet"), ("faelle-ausstehend", "erwartet-ausstehend"))
    for ordner in paare[0]:
        pfad = HIER / ordner
        if pfad.exists():
            shutil.rmtree(pfad)
        pfad.mkdir()
    namen = [n for n, _, _ in FAELLE]
    assert len(namen) == len(set(namen)), "doppelter Fallname"
    assert AUSSTEHEND <= set(namen), f"ausstehend, aber kein Fall: {sorted(AUSSTEHEND - set(namen))}"
    for name, eintrag, erwartet in FAELLE:
        f_ordner, e_ordner = paare[1] if name in AUSSTEHEND else paare[0]
        (HIER / f_ordner / f"{name}.json").write_text(json.dumps(eintrag, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / e_ordner / f"{name}.json").write_text(json.dumps(erwartet, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(FAELLE) - len(AUSSTEHEND)} Fälle geschrieben, {len(AUSSTEHEND)} ausstehend")


if __name__ == "__main__":
    schreiben()

