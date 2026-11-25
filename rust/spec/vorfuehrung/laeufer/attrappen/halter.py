"""Kleine Attrappe des Halters fuer die Suche des Laeufers: nur die Op `modell.suche` (Halter-Vertrag, `rust/spec/halter/README.md`,
Abschnitte 3, 4 `modell.suche`, 5 und 7; Vertrag 8, Abschnitt 9a, S2 Punkte 3a und 3b).

Aufruf:
  halter.py --socket <pfad> --konfig <datei> [--ablage <ordner>] [--fehler <name,...>]

Sie bindet einen Unix-Socket 0600 in einem Ordner, der schon besteht und 0700 hat, meldet `{"bereit":true}` auf stdout und bedient
je Verbindung Zeilen nacheinander, bis die Gegenseite schliesst. SIGTERM entfernt den Socket und endet mit Exit 0.

Konfiguration (JSON; `ziele` ist die Teilmenge von `halter.json`, die der Laeufer liest, Halter-Vertrag Abschnitt 2.3a):
  {"ziele": {<ziel>: {"schluessel": <name des Geheimnisses>}}, "antworten": {<ziel>: <antwort>}}
  - `ziele` mit `--ablage`: der Halter sucht selbst. Der Wert des Geheimnisses steht in `<ablage>/<name>` (Datei 0600, eine Zeile,
    `O_NOFOLLOW`); fehlt sie, ist es `halter.kein_schluessel`. Die Pfade liest er wie die Suche des Weiterreichers (S3, S4).
  - `antworten`: eine feste Antwort je Ziel, entweder die vier Zaehler oder `{"code": <code>}` (Form der Faelle in
    `rust/spec/modellweg/faelle/suche.json`, Feld `halter`). Sie geht vor `ziele`.
  Ein Ziel, das in keinem der beiden steht, ist `halter.ziel_unbekannt`.

Hergeleitet, nicht aus dem Vertrag: die Form dieser Konfiguration, der Weg zum Socket (der echte Halter liegt an einem festen Pfad,
Abschnitt 2.3) und dass die Attrappe die Rolle des Aufrufers nicht misst (sie antwortet jedem Peer der eigenen uid). Aus dem Vertrag:
Zeilenform RFC 8785 in beide Richtungen (`halter.nicht_kanonisch`), geschlossene Form der Anfrage, die Grenzen von `ziel` und `pfade`,
die Antwort `{"art":"antwort","fassung":1,"id":...,"ok":true,"suche":{...}}` und die Fehlerzeile mit `code` und festem `grund`.

`--fehler` schaltet Fehler der Mutationsprobe in der Suche des Halters ein: `suche-unlesbar` (die erste Wurzel gilt als nicht lesbar)
und `suche-folgt-verknuepfung` (die Suche folgt Verknuepfungen). Nur Standardbibliothek.
"""
from __future__ import annotations

import json
import os
import re
import signal
import socketserver
import stat
import sys
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad
import modellweg  # noqa: E402

GRUND = {
    "halter.zu_gross": "Zeile zu gross",
    "halter.json_ungueltig": "kein gueltiges JSON",
    "halter.nicht_kanonisch": "nicht kanonisch",
    "halter.fassung_unbekannt": "Fassung unbekannt",
    "halter.op_unbekannt": "Op unbekannt",
    "halter.rolle": "Op fuer Rolle nicht erlaubt",
    "halter.schluessel_unbekannt": "unbekannter Schluessel",
    "halter.schluessel_fehlt": "Pflichtfeld fehlt",
    "halter.typ_falsch": "falscher Typ",
    "halter.wert_form": "Wert hat falsche Form",
    "halter.ziel_unbekannt": "Ziel unbekannt",
    "halter.kein_schluessel": "kein Schluessel hinterlegt",
    "halter.intern": "interner Fehler",
}
OPS = ("schluessel.vorhanden", "modell.aufruf", "modell.strom", "strom.teil", "strom.ende", "modell.suche", "mcp.aufruf",
       "schluessel.setzen", "schluessel.loeschen")
KENNUNG = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
NAME = re.compile(r"^(modell|anmeldung|mcp|mail)\.[a-z0-9][a-z0-9._-]*$")
PFAD = re.compile(r"^/[^\x00-\x1f]{0,4095}$")
GRENZE = 33554432


class Fehler(Exception):
    def __init__(self, code):
        self.code = code


def _id_ok(x):
    if isinstance(x, bool):
        return False
    if isinstance(x, int):
        return 0 <= x <= 9007199254740991
    return isinstance(x, str) and bool(KENNUNG.match(x))


def geheimnis(ablage, name):
    """Wert des Geheimnisses `<ablage>/<name>` ohne abschliessendes LF; sonst `halter.kein_schluessel`."""
    if not isinstance(name, str) or len(name) > 64 or not NAME.match(name):
        raise Fehler("halter.kein_schluessel")
    try:
        fd = os.open(os.path.join(ablage, name), os.O_RDONLY | os.O_NOFOLLOW)
    except OSError:
        raise Fehler("halter.kein_schluessel")
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600:
            raise Fehler("halter.kein_schluessel")
        roh = os.read(fd, 16385)
    finally:
        os.close(fd)
    wert = roh[:-1] if roh.endswith(b"\n") else roh
    if not wert or b"\n" in wert:
        raise Fehler("halter.kein_schluessel")
    return wert


def antworte(zeile, konf, fehler):
    """Eine Anfragezeile ohne LF -> eine Antwortzeile ohne LF (Bytes)."""
    rid = None
    try:
        if len(zeile) > GRENZE:
            raise Fehler("halter.zu_gross")
        try:
            if zeile.startswith(b"\xef\xbb\xbf"):
                raise ValueError
            a = json.loads(zeile.decode("utf-8"), object_pairs_hook=modellweg._doppelt)
        except (ValueError, UnicodeDecodeError):
            raise Fehler("halter.json_ungueltig")
        if not isinstance(a, dict):
            raise Fehler("halter.json_ungueltig")
        if _id_ok(a.get("id")):
            rid = a["id"]
        if modellweg.kanonisch(a) != zeile:
            raise Fehler("halter.nicht_kanonisch")
        if a.get("fassung") != 1 or isinstance(a.get("fassung"), bool):
            raise Fehler("halter.fassung_unbekannt")
        if a.get("op") not in OPS:
            raise Fehler("halter.op_unbekannt")
        if a["op"] != "modell.suche":
            raise Fehler("halter.rolle")  # diese Attrappe bedient nur die Rolle `bruecke` und davon nur die Suche
        if set(a) - {"fassung", "id", "op", "ziel", "pfade"}:
            raise Fehler("halter.schluessel_unbekannt")
        if not {"id", "ziel", "pfade"} <= set(a):
            raise Fehler("halter.schluessel_fehlt")
        if not isinstance(a["ziel"], str) or not isinstance(a["pfade"], list) or any(not isinstance(p, str) for p in a["pfade"]) \
                or not (isinstance(a["id"], (int, str)) and not isinstance(a["id"], bool)):
            raise Fehler("halter.typ_falsch")
        if not _id_ok(a["id"]) or not KENNUNG.match(a["ziel"]) or not 1 <= len(a["pfade"]) <= 16 \
                or any(not PFAD.match(p) or any(s in (".", "..") for s in p.split("/")) for p in a["pfade"]):
            raise Fehler("halter.wert_form")
        ziel = a["ziel"]
        fest = (konf.get("antworten") or {}).get(ziel)
        if fest is not None:
            if "code" in fest:
                raise Fehler(fest["code"])
            zahlen = {n: fest[n] for n in modellweg.ZAEHLER}
        elif ziel in (konf.get("ziele") or {}) and konf.get("ablage"):
            wert = geheimnis(konf["ablage"], konf["ziele"][ziel].get("schluessel"))
            zahlen = modellweg.durchsuchen(a["pfade"], [wert], "suche-folgt-verknuepfung" in fehler, "suche-unlesbar" in fehler)
        else:
            raise Fehler("halter.ziel_unbekannt")
        return modellweg.kanonisch({"art": "antwort", "fassung": 1, "id": rid, "ok": True, "suche": zahlen})
    except Fehler as e:
        code = e.code if e.code in GRUND else "halter.intern"  # ein Code aus den Faellen, den die Tabelle hier nicht fuehrt
        grund = GRUND[code]
        return modellweg.kanonisch({"art": "antwort", "code": code, "fassung": 1, "grund": grund, "id": rid, "ok": False})


class Dienst(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


def main(argv):
    o = {}
    i = 0
    while i < len(argv):
        if argv[i] not in ("--socket", "--konfig", "--ablage", "--fehler") or i + 1 >= len(argv) or argv[i] in o:
            sys.stderr.write("halter.aufruf: Aufruf ungueltig\n")
            return 64
        o[argv[i]] = argv[i + 1]
        i += 2
    if not {"--socket", "--konfig"} <= set(o):
        sys.stderr.write("halter.aufruf: Aufruf ungueltig\n")
        return 64
    fehler = o.get("--fehler", "").split(",") if o.get("--fehler") else []
    with open(o["--konfig"], encoding="utf-8") as f:
        konf = json.load(f)
    konf["ablage"] = o.get("--ablage")
    pfad = o["--socket"]
    st = os.lstat(os.path.dirname(pfad))
    if not stat.S_ISDIR(st.st_mode) or stat.S_IMODE(st.st_mode) != 0o700 or os.path.lexists(pfad):
        sys.stderr.write("halter.socket_belegt: Socket belegt\n")
        return 74

    class H(socketserver.StreamRequestHandler):
        def handle(self):
            while True:
                zeile = self.rfile.readline(GRENZE + 2)
                if not zeile:
                    return
                if not zeile.endswith(b"\n"):
                    self.wfile.write(antworte(b"x" * (GRENZE + 1), konf, fehler) + b"\n")
                    return
                self.wfile.write(antworte(zeile[:-1], konf, fehler) + b"\n")
                self.wfile.flush()

    alt = os.umask(0o177)
    try:
        srv = Dienst(pfad, H)
    finally:
        os.umask(alt)
    os.chmod(pfad, 0o600)

    def beenden(*_):
        try:
            os.unlink(pfad)
        except OSError:
            pass
        os._exit(0)
    signal.signal(signal.SIGTERM, beenden)
    signal.signal(signal.SIGINT, beenden)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    sys.stdout.write('{"bereit":true}\n')
    sys.stdout.flush()
    threading.Event().wait()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
