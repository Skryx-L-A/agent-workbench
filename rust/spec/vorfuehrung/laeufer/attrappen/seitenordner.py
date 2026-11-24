"""Seitenordner der Attrappe (Browser-Modus): drei Dateien aus einem Ordner, wie sie die echte Bruecke liefert.

Form (Stufe 2a): `index.html`, `ansicht.js`, `ansicht_bg.wasm`, flach, ohne Unterordner. Die Dateien werden beim Start einmal
mit O_NOFOLLOW gelesen und aus dem Speicher geliefert; zur Laufzeit liest die Attrappe nichts mehr aus dem Ordner.
Namen, Typen und CSP richten sich nach rust/crates/bruecke/src/seite.rs (CSP) und verbindung.rs (`bedienen`, `seiten_antwort`)."""
from __future__ import annotations

import hashlib
import os
import stat

GRENZE = 32 * 1024 * 1024  # je Datei
# Pfad -> (Dateiname im Ordner, Content-Type). Der Typ der Bruecke: text/html und text/javascript mit charset=utf-8 (verbindung.rs),
# application/wasm (Stufe 2a; seite.rs dieses Standes liefert noch kein wasm).
WEGE = {
    "/": ("index.html", "text/html; charset=utf-8"),
    "/ansicht.js": ("ansicht.js", "text/javascript; charset=utf-8"),
    "/ansicht_bg.wasm": ("ansicht_bg.wasm", "application/wasm"),
}
DATEIEN = ("index.html", "ansicht.js", "ansicht_bg.wasm")
# seite.rs CSP; nur script-src bekommt 'wasm-unsafe-eval' dazu (Auftrag).
CSP = ("default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; "
       "img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")


class SeitenFehler(Exception):
    pass


def _lies(ordner, name):
    pfad = os.path.join(ordner, name)
    try:
        fd = os.open(pfad, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except OSError:
        raise SeitenFehler("Seitenordner unvollstaendig: %s fehlt oder ist keine regulaere Datei" % name)
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode):
            raise SeitenFehler("Seitenordner unvollstaendig: %s fehlt oder ist keine regulaere Datei" % name)
        if st.st_size > GRENZE:
            raise SeitenFehler("Seitenordner: %s ist groesser als %d Bytes" % (name, GRENZE))
        with os.fdopen(fd, "rb", closefd=False) as f:
            roh = f.read(GRENZE + 1)
    finally:
        os.close(fd)
    if len(roh) > GRENZE:
        raise SeitenFehler("Seitenordner: %s ist groesser als %d Bytes" % (name, GRENZE))
    return roh


def lade(ordner):
    """Gibt {Pfad: (Bytes, Content-Type)}. Wirft SeitenFehler mit festem Text, wenn der Ordner oder eine Datei fehlt."""
    try:
        ordner = os.path.realpath(ordner)
        if not os.path.isdir(ordner):
            raise SeitenFehler("Seitenordner fehlt: kein Ordner")
    except OSError:
        raise SeitenFehler("Seitenordner fehlt: kein Ordner")
    return {weg: (_lies(ordner, name), typ) for weg, (name, typ) in WEGE.items()}


def pruefsummen(seite):
    """Namen und sha256 je Datei, keine Inhalte."""
    return {name: hashlib.sha256(seite[weg][0]).hexdigest() for weg, (name, _) in WEGE.items()}


def beleg_html(pfad):
    """Ohne `seite_ordner`: die eigene Seite der Attrappe; nur `index.html` traegt dann eine Pruefsumme."""
    with open(pfad, "rb") as f:
        h = hashlib.sha256(f.read()).hexdigest()
    return {"name": "seite.html", "sha256": {"index.html": h, "ansicht.js": None, "ansicht_bg.wasm": None}}


def zeile(beleg):
    """`Seite: <name> index.html sha256 <wert>, ansicht.js sha256 <wert>, ansicht_bg.wasm sha256 <wert>`; `-` fuer eine fehlende Datei."""
    h = beleg["sha256"]
    return "Seite: %s %s" % (beleg["name"], ", ".join("%s sha256 %s" % (n, h.get(n) or "-") for n in DATEIEN))
