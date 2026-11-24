#!/usr/bin/env python3
"""Läufer für Vertrag 4 (Such-Socket je Sitzung). Nur Standardbibliothek.

Aufruf: RANGKERN_BIN=<programm> python3 pruefe.py [--satz sichtbar] [fall ...]
Exit 0: alle Fälle grün. Exit 1: mindestens ein Fall rot. Exit 2: Aufruffehler (etwa RANGKERN_BIN fehlt).
Umgebung des Prüflings: nur PATH, LANG=C.UTF-8, TZ=UTC, HOME=<root>/home. Sockets liegen unter SOCKET_BASIS
(Vorgabe /tmp), weil der Pfad eines Unix-Sockets kurz sein muss.

Rückfall der Einbettung (README, Abschnitt „Rückfall der Einbettung“), alles optional, alte Fälle unverändert:
  Fall `dbs`         {"<pfad>": [[rel, vektortext], ...]}: SQLite-Datenbank mit der Tabelle `embeddings` des Vorbilds.
  Fall `einbettung`  Plan der Attrappe der Einbettung: ein HTTP-Server nur auf 127.0.0.1 mit freiem Port, nur für
                     `POST /api/embed`. In `args` wird `{einbettung}` zu `http://127.0.0.1:<port>`. Plan:
                     `aus` (Port reserviert und geschlossen), `aufrufe` (Verhalten je Verbindung der Reihe nach, danach gilt
                     das letzte weiter; Vorgabe ein Verhalten {}). Verhalten: `vektor` (Vorgabe [1.0, 0.0]),
                     `verzoegerung_ms` (Pause vor der Antwort), `status` (Vorgabe 200), `koerper` `kaputt` (abgeschnittenes
                     JSON), `zu` (Anfrage lesen, ohne Antwort schließen), `danach_aus` (gleich nach dem Annehmen dieser
                     Verbindung den Port schließen: der nächste Verbindungsaufbau wird abgewiesen).
  Schritte           {"einbettung_aus": true} schließt den Port, {"einbettung_an": true} öffnet denselben Port wieder,
                     {"einbettung_plan": {...}} ersetzt die Verhalten; die Nummerierung beginnt von vorn.
  Erwartung          `stderr_leer`: stderr des Prüflings ist bis zum Ende leer. `einbettung_aufrufe`: Zahl der Verbindungen
                     zur Attrappe im ganzen Fall.
Die Protokollzeilen prüft der Läufer immer auch als Kette: kanonische Form (RFC 8785), `vorher` der ersten Zeile einer
Monatsdatei ohne Vormonat 64 Nullen, jeder weiteren der sha256 der vorigen Zeile. Mit KETTE_BIN=<programm `kette` aus
Crate formate> fährt er zusätzlich `kette pruefe-quelle <protokoll> brain` (Exit 0 verlangt).
Eine Monatsdatei, die der Fall unter `files` vorgibt und die der Prozess nicht verändert, prüft der Läufer nur auf Gleichheit
mit der Vorgabe: Sie ist Eingabe des Falls, keine Zeile des Prozesses, und kann absichtlich kaputt sein (etwa ohne LF am Ende).
Kette, Zeilenzahl und `kette pruefe-quelle` gelten nur für Dateien, die der Prozess angelegt oder verändert hat.

Feste Prüfungen jedes Falls mit Bereitschaft (seit 23.11.2026, B11 und OPEN 4 der Vertragsprüfung):
  - Die Bereitschaftszeile ist byte-genau `{"bereit":true}` und LF; danach kommt bis zum Ende nichts mehr auf stdout.
  - `stderr_zeile` steht vor der Bereitschaftszeile: Sie muss schon in der Pipe liegen, wenn die Bereitschaftszeile gelesen
    ist (der Prozess schreibt beide nacheinander; was er vorher schreibt, ist dann schon da).
  - Jede Protokollzeile des Prozesses trägt `zeit` nach dem Muster von Vertrag 5 (UTC, Millisekunden, `Z`), liegt in der
    Monatsdatei ihres Monats und fällt in die Laufzeit des Prozesses (mit 2 s Spiel).
  - Unter Linux gehört `/proc/<pid>/stat` root und nicht der Kennung des Läufers; als root ist die Prüfung blind, darum rot.

Erweiterungen für die Fälle vom 23.11.2026, alles optional:
  Fall `env_extra`     {"NAME": "wert"}: zusätzliche Variablen für den Prüfling (Vertrag 4: er liest keine).
  Fall `vorher_rechte` {"<rel>": "0500"}: Rechte vor dem Start; nach dem Lauf stellt der Läufer 0755 her.
  Fall `ende_signal`   "SIGINT" statt SIGTERM am Ende.
  Erwartung `nachher_fehlt` [<rel>, ...]: Pfade, die nach dem Ende (oder dem gescheiterten Start) nicht da sein dürfen.
  Schritt {"zeilen": [<schritt>, ...]}: alle Anfragen in einem Stück senden, dann je eine Antwort lesen (Reihenfolge).
  Schritt {"line_hex": "<hex>"}: genau diese Bytes und LF (etwa ungültiges UTF-8).
  Schritt {"line_gross": {"anfang": a, "zeichen": z, "anzahl": n, "ende": e}}: die Zeile a + z*n + e und LF.
  Schritt {"line": t, "ohne_lf": true}: die Zeile ohne LF, dann Ende der Schreibseite (shutdown); die Antwort kommt danach.
  Schritt {"grenze": 16}: 16 Verbindungen mit je einem ping offen halten; die 17. wird ohne ein Byte geschlossen; die
                       erste antwortet weiter; nach dem Schließen einer ist wieder Platz (Klarstellung 13). Ohne Antwort.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import re
import selectors
import shutil
import signal
import socket
import sqlite3
import stat
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
FRIST = 10.0
ZEILENFELDER = {
    "ausgabe.gesperrt": {"aktionsname", "anzahl", "ergebnis", "fassung", "grund", "klasse", "kunde", "route", "sitzung",
                         "vorher", "zeit"},
    "probelauf.zaehlung": {"aktionsname", "anzahl", "ergebnis", "fassung", "programm", "sitzung", "vorher", "zeit"},
    "suche.rueckfall": {"aktionsname", "ergebnis", "fassung", "grund", "route", "sitzung", "vorher", "zeit"},
}
NULL_HASH = "0" * 64
BEREIT = b'{"bereit":true}\n'
# Vertrag 5 (rust/spec/VERTRAEGE.md, Feld `zeit`)
ZEIT_MUSTER = re.compile(r"^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$")
ZEIT_SPIEL = 2.0
DB_SCHEMA = "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, vector TEXT NOT NULL, updated REAL NOT NULL)"


class Rot(Exception):
    pass


def gleich(a, b) -> bool:
    """JSON-Gleichheit ohne Python-Abkürzungen: True ist nie 1, Listen und Objekte rekursiv."""
    if isinstance(a, bool) or isinstance(b, bool):
        return type(a) is type(b) and a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(gleich(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(gleich(x, y) for x, y in zip(a, b))
    return type(a) is type(b) and a == b


def zeile_lesen(datei, frist: float) -> bytes | None:
    sel = selectors.DefaultSelector()
    sel.register(datei, selectors.EVENT_READ)
    puffer = b""
    ende = time.monotonic() + frist
    fd = datei.fileno()
    while not puffer.endswith(b"\n"):
        rest = ende - time.monotonic()
        if rest <= 0 or not sel.select(rest):
            return None
        stueck = os.read(fd, 1)
        if not stueck:
            return puffer or None
        puffer += stueck
    return puffer


def bereitschaft_pruefen(zeile: bytes | None) -> None:
    """Vertrag 4: genau `{"bereit":true}` und LF, Byte für Byte (nicht nur JSON-gleich)."""
    if zeile != BEREIT:
        raise Rot(f"Start: Bereitschaftszeile {BEREIT!r} erwartet, bekommen {zeile!r}")


def schon_da(datei) -> bytes:
    """Was jetzt in der Pipe liegt, ohne zu warten (die Pipe bleibt danach blockierend)."""
    fd = datei.fileno()
    os.set_blocking(fd, False)
    roh = b""
    try:
        while True:
            try:
                stueck = os.read(fd, 65536)
            except BlockingIOError:
                break
            if not stueck:
                break
            roh += stueck
    finally:
        os.set_blocking(fd, True)
    return roh


def rest_lesen(datei) -> bytes:
    """Den Rest einer Pipe nach dem Ende des Prozesses, über den Deskriptor (wie `schon_da`)."""
    roh = b""
    while True:
        stueck = os.read(datei.fileno(), 65536)
        if not stueck:
            return roh
        roh += stueck


def stdout_nach_bereitschaft_pruefen(rest: bytes) -> None:
    if rest:
        raise Rot(f"stdout: nach der Bereitschaftszeile kam noch {rest[:120]!r}")


def stderr_pruefen(erw: dict, vor: bytes, nach: bytes) -> None:
    """`vor`: stderr bis zur Bereitschaftszeile, `nach`: der Rest bis zum Ende."""
    if erw.get("stderr_leer") and (vor or nach):
        raise Rot(f"stderr nicht leer: {(vor + nach)[:200]!r}")
    if "stderr_zeile" in erw:
        soll = erw["stderr_zeile"]
        alle = (vor + nach).decode("utf-8", "replace").splitlines()
        if alle.count(soll) != 1:
            raise Rot(f"stderr: die Zeile {soll!r} steht {alle.count(soll)}-mal da, erwartet 1-mal")
        if soll not in vor.decode("utf-8", "replace").splitlines():
            raise Rot(f"stderr: die Zeile {soll!r} kam erst nach der Bereitschaftszeile")


def zeit_ms(text) -> int | None:
    if not isinstance(text, str) or not ZEIT_MUSTER.match(text):
        return None
    try:
        t = datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%S.%fZ").replace(tzinfo=datetime.timezone.utc)
    except ValueError:
        return None
    return int(t.timestamp() * 1000)


def zeit_pruefen(i: int, datei: str, z: dict, fenster: tuple[float, float] | None) -> None:
    """Feld `zeit` einer Zeile des Prozesses: Muster von Vertrag 5, Monat der Datei, Laufzeit des Prozesses."""
    ms = zeit_ms(z.get("zeit"))
    if ms is None:
        raise Rot(f"Protokollzeile {i}: zeit {z.get('zeit')!r} folgt nicht dem Muster von Vertrag 5")
    if datei != f"brain-{z['zeit'][:7]}.jsonl":
        raise Rot(f"Protokollzeile {i}: zeit {z['zeit']} steht in {datei}")
    if fenster is not None:
        von, bis = fenster
        if not (von - ZEIT_SPIEL) * 1000 <= ms <= (bis + ZEIT_SPIEL) * 1000:
            raise Rot(f"Protokollzeile {i}: zeit {z['zeit']} liegt außerhalb der Laufzeit des Prozesses")


def ersetzen(wert: str, root: Path) -> str:
    return wert.replace("{root}", str(root)).replace("{sock}", str(root / "s.sock"))


def antwort_pruefen(nr: int, erw: dict, roh: bytes) -> None:
    try:
        got = json.loads(roh.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise Rot(f"Antwort {nr}: kein JSON ({exc})")
    if not roh.endswith(b"\n") or roh.count(b"\n") != 1:
        raise Rot(f"Antwort {nr}: nicht genau eine Zeile mit LF")
    if "rels" not in erw:
        if not gleich(got, erw):
            raise Rot(f"Antwort {nr}: erwartet {erw}, bekommen {got}")
        return
    schluessel = {"fallback", "hits", "gesperrt"} | ({"id"} if "id" in erw else set())
    if not isinstance(got, dict) or set(got) != schluessel:
        raise Rot(f"Antwort {nr}: Felder {sorted(got) if isinstance(got, dict) else got}, erwartet {sorted(schluessel)}")
    for feld in ("fallback", "gesperrt") + (("id",) if "id" in erw else ()):
        if not gleich(got[feld], erw[feld]):
            raise Rot(f"Antwort {nr}: {feld} erwartet {erw[feld]!r}, bekommen {got[feld]!r}")
    rels = [h.get("rel") for h in got["hits"]]
    if rels != erw["rels"]:
        raise Rot(f"Antwort {nr}: Treffer erwartet {erw['rels']}, bekommen {rels}")
    for rel, felder in erw.get("pruefe_felder", {}).items():
        hit = next(h for h in got["hits"] if h["rel"] == rel)
        for feld, wert in felder.items():
            if wert is None and feld in hit:
                raise Rot(f"Antwort {nr}: {rel} trägt {feld}, das fehlen muss")
            if wert is not None and not gleich(hit.get(feld), wert):
                raise Rot(f"Antwort {nr}: {rel}.{feld} erwartet {wert!r}, bekommen {hit.get(feld)!r}")


def kette_pruefen(ordner: Path, unveraendert: frozenset[str] = frozenset()) -> None:
    """Die Kette jeder Monatsdatei: kanonisch, `vorher` verkettet (Vertrag 5). Die erste Zeile der ersten Monatsdatei
    trägt 64 Nullen; die erste Zeile eines späteren Monats hängt am Anker und wird hier nicht nachgerechnet.
    Dateien in `unveraendert` (Vorgaben des Falls, vom Prozess nicht berührt) bleiben außen vor."""
    dateien = sorted(p for p in ordner.glob("brain-*.jsonl")
                     if not p.name.startswith("brain.anker") and p.name not in unveraendert)
    for nr, datei in enumerate(dateien):
        roh = datei.read_bytes()
        if roh and not roh.endswith(b"\n"):
            raise Rot(f"Kette {datei.name}: letzte Zeile ohne LF")
        vorher = NULL_HASH if nr == 0 else None
        for i, z in enumerate(roh.split(b"\n")[:-1] if roh else [], 1):
            obj = json.loads(z.decode("utf-8"))
            kanon = json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
            if z != kanon:
                raise Rot(f"Kette {datei.name}, Zeile {i}: nicht kanonisch")
            if vorher is not None and obj.get("vorher") != vorher:
                raise Rot(f"Kette {datei.name}, Zeile {i}: vorher {obj.get('vorher')!r}, erwartet {vorher}")
            vorher = hashlib.sha256(z).hexdigest()
    kette = os.environ.get("KETTE_BIN", "")
    if kette and dateien:
        lauf = subprocess.run([kette, "pruefe-quelle", str(ordner), "brain"], stdin=subprocess.DEVNULL,
                              capture_output=True, timeout=FRIST)
        if lauf.returncode != 0:
            raise Rot(f"kette pruefe-quelle: Exit {lauf.returncode}, {lauf.stdout.decode('utf-8', 'replace').strip()}")


def protokoll_vorgabe(fall: dict) -> dict[str, bytes]:
    """Die Monatsdateien, die der Fall unter `protokoll/` vorgibt, als {Name: Bytes} (so, wie fall_laufen sie schreibt)."""
    return {rel.split("/", 1)[1]: text.encode("utf-8") for rel, text in fall["files"].items()
            if rel.startswith("protokoll/") and "/" not in rel.split("/", 1)[1]}


def protokoll_pruefen(root: Path, erwartet: list[dict], vorgabe: dict[str, bytes] | None = None,
                      fenster: tuple[float, float] | None = None) -> None:
    """`erwartet` beschreibt die Zeilen des Prozesses. Eine unveränderte Vorgabe zählt nicht mit und durchläuft die
    Kettenprüfung nicht; eine fehlende Vorgabe ist rot. Ohne `vorgabe` gilt jede Monatsdatei als Datei des Prozesses.
    `fenster` (Sekunden seit 1970): Start und Ende des Prozesses; jede `zeit` muss darin liegen."""
    zeilen = []
    ordner = root / "protokoll"
    vorgabe = vorgabe or {}
    unveraendert = set()
    for name, roh in vorgabe.items():
        datei = ordner / name
        if not datei.is_file():
            raise Rot(f"Protokoll: die Vorgabe {name} fehlt nach dem Lauf")
        if datei.read_bytes() == roh:
            unveraendert.add(name)
    if ordner.is_dir():
        for datei in sorted(ordner.glob("brain-*.jsonl")):
            if datei.name in unveraendert:
                continue
            roh = datei.read_bytes()
            # Beginnt eine veraenderte Vorgabe noch mit ihrem vorgegebenen Inhalt, sind diese ersten Zeilen Eingabe des
            # Falls: sie zaehlen mit (die Erwartung nennt sie), aber ihre `zeit` stammt nicht aus der Laufzeit des Prozesses.
            vor = vorgabe.get(datei.name)
            n_vor = len(vor.decode("utf-8").splitlines()) if vor is not None and roh.startswith(vor) else 0
            for k, z in enumerate(roh.decode("utf-8").splitlines()):
                zeilen.append((datei.name, json.loads(z), k < n_vor))
        kette_pruefen(ordner, frozenset(unveraendert))
    if len(zeilen) != len(erwartet):
        raise Rot(f"Protokoll: {len(zeilen)} Zeilen, erwartet {len(erwartet)}")
    for i, ((name, z, aus_vorgabe), e) in enumerate(zip(zeilen, erwartet), 1):
        if set(z) != ZEILENFELDER.get(z.get("aktionsname"), set()):
            raise Rot(f"Protokollzeile {i}: Felder {sorted(z)}")
        if not aus_vorgabe:
            zeit_pruefen(i, name, z, fenster)
        for feld, wert in e.items():
            if not gleich(z[feld], wert):
                raise Rot(f"Protokollzeile {i}: {feld} erwartet {wert!r}, bekommen {z[feld]!r}")


def dumpable_pruefen(pid: int) -> None:
    """PR_SET_DUMPABLE 0 setzt den Eigentümer der Einträge unter /proc/<pid> auf root (gemessen 20.11.). Als root ist die
    Prüfung blind (der Eigentümer wäre auch ohne den Schritt root); darum muss er sich vom eigenen unterscheiden, wie in
    tests/abschirmung.rs, und als root ist der Fall rot."""
    if os.geteuid() == 0:
        raise Rot("Start: der Läufer läuft als root; die Prüfung von PR_SET_DUMPABLE wäre blind")
    besitzer = os.stat(f"/proc/{pid}/stat").st_uid
    wurzel = root_kennung()
    if wurzel == os.geteuid():
        raise Rot("Start: /usr gehört dem Läufer; die Prüfung von PR_SET_DUMPABLE wäre blind")
    if besitzer == os.geteuid() or besitzer != wurzel:
        raise Rot("Start: Prozess ist nicht PR_SET_DUMPABLE 0")


def root_kennung() -> int:
    """Die uid, unter der root des Wirts in diesem Namensraum erscheint, abgelesen am Eigentümer von /usr: 0 im
    Namensraum des Wirts, die `overflowuid` (meist 65534) in einem Nutzer-Namensraum, der root nicht abbildet (gemessen
    24.11.2026 im Einschluss auf dem Linux-Baurechner: Eigentümer des nicht dumpbaren Kindes 65534, außerhalb 0)."""
    return os.stat("/usr").st_uid


def kernabbild_grenze_pruefen(pid: int) -> None:
    """Regel „Abschirmung über kernabbild“ (Vertrag 4): `Max core file size` zeigt weich 1 und hart 1.
    Positivkontrolle: Kann der Läufer `limits` nicht lesen oder fehlt die Zeile, ist der Fall rot, nie grün."""
    try:
        text = Path(f"/proc/{pid}/limits").read_text(encoding="utf-8")
    except OSError as e:
        raise Rot(f"Start: /proc/{pid}/limits nicht lesbar ({e}); die Grenze 1/1 ist ungeprüft")
    zeilen = [z for z in text.splitlines() if z.startswith("Max core file size")]
    if len(zeilen) != 1:
        raise Rot(f"Start: keine eindeutige Zeile 'Max core file size' in /proc/{pid}/limits")
    felder = zeilen[0].split()
    # "Max core file size <weich> <hart> bytes"
    if len(felder) < 6 or felder[4] != "1" or felder[5] != "1":
        raise Rot(f"Start: Kernabbild-Grenze {' '.join(felder[4:6])!r}, erwartet weich 1 und hart 1")


class Einbettung:
    """Attrappe der Einbettung (Kopf der Datei): HTTP/1.1 nur auf 127.0.0.1, eine Antwort je Verbindung."""

    def __init__(self, plan: dict) -> None:
        self.verhalten = list(plan.get("aufrufe") or [{}])
        self.nummer = 0
        self.aufrufe = 0
        self.sperre = threading.Lock()
        self.halt = threading.Event()
        self.lauscher: tuple[socket.socket, threading.Event] | None = None
        probe = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        probe.bind(("127.0.0.1", 0))
        self.port = probe.getsockname()[1]
        probe.close()
        if not plan.get("aus"):
            self.an()

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}"

    def an(self) -> None:
        with self.sperre:
            if self.lauscher is not None:
                return
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind(("127.0.0.1", self.port))
            s.listen(16)
            s.settimeout(0.02)
            zu = threading.Event()
            self.lauscher = (s, zu)
        threading.Thread(target=self._annehmen, args=(s, zu), daemon=True).start()

    def aus(self) -> None:
        """Schließt den Port und wartet, bis der Faden der Annahme ihn wirklich geschlossen hat."""
        with self.sperre:
            eintrag, self.lauscher = self.lauscher, None
        if eintrag is not None and not eintrag[1].wait(FRIST):
            raise Rot("Einbettung: der Port der Attrappe schließt nicht")

    def setze_plan(self, plan: dict) -> None:
        with self.sperre:
            self.verhalten = list(plan.get("aufrufe") or [{}])
            self.nummer = 0

    def stop(self) -> None:
        self.halt.set()
        try:
            self.aus()
        except Rot:
            pass

    def _annehmen(self, s: socket.socket, zu: threading.Event) -> None:
        """Nur dieser Faden nimmt an und schließt den Port; `aus` und `danach_aus` geben ihm nur das Zeichen."""
        try:
            while not self.halt.is_set():
                with self.sperre:
                    if self.lauscher is None or self.lauscher[0] is not s:
                        return
                try:
                    c, _ = s.accept()
                except socket.timeout:
                    continue
                except OSError:
                    return
                with self.sperre:
                    v = self.verhalten[min(self.nummer, len(self.verhalten) - 1)]
                    self.nummer += 1
                    self.aufrufe += 1
                    if v.get("danach_aus"):
                        self.lauscher = None
                c.settimeout(5.0)
                threading.Thread(target=self._bedienen, args=(c, v), daemon=True).start()
                if v.get("danach_aus"):
                    return
        finally:
            s.close()
            zu.set()

    @staticmethod
    def _lesen(c: socket.socket) -> bool:
        c.settimeout(5.0)
        roh = b""
        while True:
            if b"\r\n\r\n" in roh:
                kopf, rest = roh.split(b"\r\n\r\n", 1)
                laenge = 0
                for z in kopf.decode("latin-1").lower().split("\r\n"):
                    if z.startswith("content-length:"):
                        laenge = int(z.split(":", 1)[1].strip())
                if len(rest) >= laenge:
                    return kopf.startswith(b"POST /api/embed ")
            try:
                stueck = c.recv(65536)
            except OSError:
                return False
            if not stueck:
                return False
            roh += stueck

    def _bedienen(self, c: socket.socket, v: dict) -> None:
        try:
            if not self._lesen(c) or v.get("zu"):
                return
            if v.get("verzoegerung_ms"):
                if self.halt.wait(v["verzoegerung_ms"] / 1000.0):
                    return
            if v.get("koerper") == "kaputt":
                koerper = b'{"embeddings": [[1.0, 0.'
            else:
                koerper = json.dumps({"model": "m", "embeddings": [v.get("vektor", [1.0, 0.0])]}).encode("utf-8")
            status = v.get("status", 200)
            c.sendall(f"HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {len(koerper)}\r\n"
                      f"Connection: close\r\n\r\n".encode("ascii") + koerper)
        except OSError:
            pass
        finally:
            c.close()


def db_bauen(pfad: Path, zeilen: list) -> None:
    pfad.parent.mkdir(parents=True, exist_ok=True)
    con = sqlite3.connect(str(pfad))
    try:
        con.execute(DB_SCHEMA)
        for i, (rel, vektor) in enumerate(zeilen):
            con.execute("INSERT INTO embeddings VALUES (?,?,?,?)", (rel, f"h{i}", vektor, i + 1.0))
        con.commit()
    finally:
        con.close()


def ping_auf(v: socket.socket, nr: int) -> None:
    v.sendall(json.dumps({"op": "ping", "id": nr}, separators=(",", ":")).encode("utf-8") + b"\n")
    roh = v.makefile("rb").readline()
    if not gleich(json.loads(roh.decode("utf-8") or "null"), {"ok": True, "id": nr}):
        raise Rot(f"Grenze: Verbindung {nr} antwortet {roh!r}")


def ohne_byte_zu(v: socket.socket) -> bool:
    """True, wenn der Server die Verbindung ohne ein Byte schließt (binnen FRIST)."""
    try:
        return v.recv(1) == b""
    except ConnectionResetError:
        return True


def grenze_pruefen(sock_pfad: Path, n: int) -> None:
    """Klarstellung 13: höchstens n gleichzeitige Verbindungen."""
    offen = []
    try:
        for i in range(1, n + 1):
            v = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            v.settimeout(FRIST)
            v.connect(str(sock_pfad))
            offen.append(v)
            ping_auf(v, i)
        zuviel = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        zuviel.settimeout(FRIST)
        try:
            zuviel.connect(str(sock_pfad))
            try:
                zuviel.sendall(b'{"op":"ping","id":999}\n')
            except (BrokenPipeError, ConnectionResetError):
                pass
            if not ohne_byte_zu(zuviel):
                raise Rot(f"Grenze: die Verbindung {n + 1} bekam ein Byte")
        except socket.timeout:
            raise Rot(f"Grenze: die Verbindung {n + 1} blieb offen")
        finally:
            zuviel.close()
        ping_auf(offen[0], 1001)
        offen.pop().close()
        # Positivkontrolle: Ist wieder Platz, antwortet eine neue Verbindung. Der Server merkt das Ende der alten erst
        # beim Lesen; darum mehrere Versuche, zusammen höchstens FRIST.
        ende = time.monotonic() + FRIST
        while True:
            v = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            v.settimeout(FRIST)
            v.connect(str(sock_pfad))
            try:
                ping_auf(v, 1002)
                offen.append(v)
                break
            except (Rot, ValueError, OSError):
                v.close()
                if time.monotonic() > ende:
                    raise Rot("Grenze: nach dem Schließen einer Verbindung ist kein Platz frei")
                time.sleep(0.05)
    finally:
        for v in offen:
            v.close()


def nachher_fehlt_pruefen(root: Path, erw: dict) -> None:
    for rel in erw.get("nachher_fehlt", []):
        if os.path.lexists(root / rel):
            raise Rot(f"Ende: {rel} ist noch da")


def fall_laufen(prog: str, fall: dict, erw: dict) -> None:
    basis = os.environ.get("SOCKET_BASIS", "/tmp")
    root = Path(tempfile.mkdtemp(prefix="ss-", dir=basis))
    rechte: list[Path] = []
    proc = None
    einbettung = None
    try:
        for d in fall["dirs"]:
            (root / d).mkdir(parents=True, exist_ok=True)
        for rel, text in fall["files"].items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        for rel, zeilen in fall.get("dbs", {}).items():
            db_bauen(root / rel, zeilen)
        for rel, modus in fall.get("vorher_rechte", {}).items():
            if os.geteuid() == 0:
                raise Rot("Fall mit vorher_rechte läuft nicht als root")
            os.chmod(root / rel, int(modus, 8))
            rechte.append(root / rel)
        if "einbettung" in fall:
            einbettung = Einbettung(fall["einbettung"])
        vorher = (root / "s.sock").read_bytes() if (root / "s.sock").exists() else None
        argv = [prog] + [ersetzen(a, root) for a in fall["args"]]
        if einbettung is not None:
            argv = [a.replace("{einbettung}", einbettung.url) for a in argv]
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8", "TZ": "UTC",
               "HOME": str(root / "home")}
        env.update(fall.get("env_extra", {}))
        t_start = time.time()
        proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                env=env, cwd=root)
        start = erw["start"]
        if start != "bereit":
            try:
                code = proc.wait(FRIST)
            except subprocess.TimeoutExpired:
                raise Rot(f"Start: Exit {start['exit']} erwartet, Prozess läuft nach {FRIST} s noch")
            if code != start["exit"]:
                raise Rot(f"Start: Exit {code}, erwartet {start['exit']}")
            if proc.stdout.read():
                raise Rot("Start: stdout nicht leer")
            sock = root / "s.sock"
            if vorher is not None and (not sock.is_file() or sock.read_bytes() != vorher):
                raise Rot("Start: vorhandene Datei am Socketpfad verändert")
            if vorher is None and sock.exists():
                raise Rot("Start: Socket angelegt, obwohl der Start scheitert")
            nachher_fehlt_pruefen(root, erw)
            return
        zeile = zeile_lesen(proc.stdout, FRIST)
        bereitschaft_pruefen(zeile)
        stderr_vor = schon_da(proc.stderr)
        sock_pfad = root / "s.sock"
        modus = os.lstat(sock_pfad).st_mode
        if not stat.S_ISSOCK(modus):
            raise Rot("Start: am Socketpfad liegt kein Socket")
        if f"{stat.S_IMODE(modus):04o}" != erw["socket_rechte"]:
            raise Rot(f"Start: Socket mit Rechten {stat.S_IMODE(modus):04o}, erwartet {erw['socket_rechte']}")
        if sys.platform.startswith("linux"):
            dumpable_pruefen(proc.pid)
            kernabbild_grenze_pruefen(proc.pid)
        verbindung = None
        lesen = None
        antworten: list[bytes] = []
        for st in fall["steps"]:
            if "verbindung" in st:
                if verbindung:
                    verbindung.close()
                verbindung = None
            elif "einbettung_aus" in st or "einbettung_an" in st or "einbettung_plan" in st:
                if einbettung is None:
                    raise ValueError("Schritt für die Einbettung ohne Feld einbettung im Fall")
                if "einbettung_plan" in st:
                    einbettung.setze_plan(st["einbettung_plan"])
                elif "einbettung_aus" in st:
                    einbettung.aus()
                else:
                    einbettung.an()
            elif "write" in st:
                for rel, text in st["write"].items():
                    (root / rel).write_text(text, encoding="utf-8")
            elif "chmod" in st:
                if os.geteuid() == 0:
                    raise Rot("Fall mit chmod läuft nicht als root")
                for rel, modus in st["chmod"].items():
                    os.chmod(root / rel, int(modus, 8))
                    rechte.append(root / rel)
            elif "grenze" in st:
                grenze_pruefen(sock_pfad, st["grenze"])
            elif "zeilen" in st:
                if verbindung is None:
                    verbindung = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    verbindung.settimeout(FRIST)
                    verbindung.connect(str(sock_pfad))
                    lesen = verbindung.makefile("rb")
                stueck = b"".join(json.dumps(s["request"], ensure_ascii=False, separators=(",", ":")).encode("utf-8")
                                  + b"\n" if "request" in s else s["line"].encode("utf-8") + b"\n" for s in st["zeilen"])
                verbindung.sendall(stueck)
                for _ in st["zeilen"]:
                    try:
                        roh = lesen.readline()
                    except socket.timeout:
                        raise Rot(f"Anfrage {len(antworten) + 1}: keine Antwort binnen {FRIST} s")
                    if not roh:
                        raise Rot(f"Anfrage {len(antworten) + 1}: Verbindung ohne Antwort geschlossen")
                    antworten.append(roh)
            else:
                if verbindung is None:
                    verbindung = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    verbindung.settimeout(FRIST)
                    verbindung.connect(str(sock_pfad))
                    lesen = verbindung.makefile("rb")
                if "request" in st:
                    daten = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":")).encode("utf-8")
                elif "line_hex" in st:
                    daten = bytes.fromhex(st["line_hex"])
                elif "line_gross" in st:
                    g = st["line_gross"]
                    daten = (g["anfang"] + g["zeichen"] * g["anzahl"] + g["ende"]).encode("utf-8")
                else:
                    daten = st["line"].encode("utf-8")
                if st.get("ohne_lf"):
                    verbindung.sendall(daten)
                    verbindung.shutdown(socket.SHUT_WR)
                else:
                    verbindung.sendall(daten + b"\n")
                try:
                    roh = lesen.readline()
                except socket.timeout:
                    raise Rot(f"Anfrage {len(antworten) + 1}: keine Antwort binnen {FRIST} s")
                if not roh:
                    raise Rot(f"Anfrage {len(antworten) + 1}: Verbindung ohne Antwort geschlossen")
                antworten.append(roh)
        if verbindung:
            verbindung.close()
        for p in rechte:
            os.chmod(p, 0o755)
        if len(antworten) != len(erw["responses"]):
            raise Rot(f"{len(antworten)} Antworten, erwartet {len(erw['responses'])}")
        for nr, (e, roh) in enumerate(zip(erw["responses"], antworten), 1):
            klein = roh.decode("utf-8", "replace").lower()
            for marke in fall["verboten"]:
                if marke.lower() in klein:
                    raise Rot(f"Antwort {nr}: enthält die gesperrte Marke {marke!r}")
            antwort_pruefen(nr, e, roh)
        proc.send_signal(getattr(signal, fall.get("ende_signal", "SIGTERM")))
        try:
            code = proc.wait(FRIST)
        except subprocess.TimeoutExpired:
            raise Rot(f"Ende: nach SIGTERM läuft der Prozess nach {FRIST} s noch")
        t_ende = time.time()
        if code != erw["ende"]["exit"]:
            raise Rot(f"Ende: Exit {code}, erwartet {erw['ende']['exit']}")
        stdout_nach_bereitschaft_pruefen(rest_lesen(proc.stdout))
        if os.path.lexists(sock_pfad):
            raise Rot("Ende: Socket liegt nach dem Ende noch da")
        protokoll_pruefen(root, erw["protokoll"], protokoll_vorgabe(fall), (t_start, t_ende))
        nachher_fehlt_pruefen(root, erw)
        if "einbettung_aufrufe" in erw and einbettung.aufrufe != erw["einbettung_aufrufe"]:
            raise Rot(f"Einbettung: {einbettung.aufrufe} Verbindungen, erwartet {erw['einbettung_aufrufe']}")
        stderr_pruefen(erw, stderr_vor, rest_lesen(proc.stderr))
        if "marker_zaehlung" in erw:
            mz = erw["marker_zaehlung"]
            datei = root / mz["datei"]
            if not datei.is_file():
                raise Rot("Zählung: Datei fehlt")
            if f"{stat.S_IMODE(datei.stat().st_mode):04o}" != "0600":
                raise Rot(f"Zählung: Rechte {stat.S_IMODE(datei.stat().st_mode):04o}, erwartet 0600")
            roh = datei.read_bytes()
            got = json.loads(roh.decode("utf-8"))
            kanonisch = json.dumps(got, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
            if roh.rstrip(b"\n") != kanonisch:
                raise Rot("Zählung: Datei nicht in kanonischer Form (RFC 8785)")
            if not gleich(got, mz["inhalt"]):
                raise Rot(f"Zählung: {got}, erwartet {mz['inhalt']}")
    finally:
        if proc and proc.poll() is None:
            proc.kill()
            proc.wait()
        if einbettung is not None:
            einbettung.stop()
        for p in rechte:
            if p.exists():
                os.chmod(p, 0o755)
        shutil.rmtree(root, ignore_errors=True)


def main() -> int:
    ap = argparse.ArgumentParser(allow_abbrev=False)
    ap.add_argument("--satz", default="sichtbar")
    ap.add_argument("faelle", nargs="*")
    a = ap.parse_args()
    prog = os.environ.get("RANGKERN_BIN", "")
    if not prog:
        print("pruefe.py: RANGKERN_BIN fehlt", file=sys.stderr)
        return 2
    satz = HIER / a.satz
    namen = a.faelle or sorted(p.stem for p in (satz / "faelle").glob("*.json"))
    rot = 0
    for name in namen:
        fall = json.loads((satz / "faelle" / f"{name}.json").read_text(encoding="utf-8"))
        erw = json.loads((satz / "erwartet" / f"{name}.json").read_text(encoding="utf-8"))
        try:
            fall_laufen(prog, fall, erw)
            print(f"grün  {name}")
        except (Rot, OSError, ValueError) as exc:
            rot += 1
            print(f"ROT   {name}: {exc}")
    print(f"{len(namen) - rot} von {len(namen)} grün")
    return 1 if rot else 0


if __name__ == "__main__":
    sys.exit(main())
