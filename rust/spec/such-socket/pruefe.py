#!/usr/bin/env python3
"""Läufer für Vertrag 4 (Such-Socket je Sitzung). Nur Standardbibliothek.

Aufruf: RANGKERN_BIN=<programm> python3 pruefe.py [--satz sichtbar] [fall ...]
Exit 0: alle Fälle grün. Exit 1: mindestens ein Fall rot. Exit 2: Aufruffehler (etwa RANGKERN_BIN fehlt).
Umgebung des Prüflings: nur PATH, LANG=C.UTF-8, TZ=UTC, HOME=<root>/home. Sockets liegen unter SOCKET_BASIS
(Vorgabe /tmp), weil der Pfad eines Unix-Sockets kurz sein muss.
"""
from __future__ import annotations

import argparse
import json
import os
import selectors
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
FRIST = 10.0
ZEILENFELDER = {
    "ausgabe.gesperrt": {"aktionsname", "anzahl", "ergebnis", "fassung", "grund", "klasse", "kunde", "route", "sitzung",
                         "vorher", "zeit"},
    "probelauf.zaehlung": {"aktionsname", "anzahl", "ergebnis", "fassung", "programm", "sitzung", "vorher", "zeit"},
}


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


def protokoll_pruefen(root: Path, erwartet: list[dict]) -> None:
    zeilen = []
    ordner = root / "protokoll"
    if ordner.is_dir():
        for datei in sorted(ordner.glob("brain-*.jsonl")):
            for z in datei.read_text(encoding="utf-8").splitlines():
                zeilen.append(json.loads(z))
    if len(zeilen) != len(erwartet):
        raise Rot(f"Protokoll: {len(zeilen)} Zeilen, erwartet {len(erwartet)}")
    for i, (z, e) in enumerate(zip(zeilen, erwartet), 1):
        if set(z) != ZEILENFELDER.get(z.get("aktionsname"), set()):
            raise Rot(f"Protokollzeile {i}: Felder {sorted(z)}")
        for feld, wert in e.items():
            if not gleich(z[feld], wert):
                raise Rot(f"Protokollzeile {i}: {feld} erwartet {wert!r}, bekommen {z[feld]!r}")


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


def fall_laufen(prog: str, fall: dict, erw: dict) -> None:
    basis = os.environ.get("SOCKET_BASIS", "/tmp")
    root = Path(tempfile.mkdtemp(prefix="ss-", dir=basis))
    rechte: list[Path] = []
    proc = None
    try:
        for d in fall["dirs"]:
            (root / d).mkdir(parents=True, exist_ok=True)
        for rel, text in fall["files"].items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        vorher = (root / "s.sock").read_bytes() if (root / "s.sock").exists() else None
        argv = [prog] + [ersetzen(a, root) for a in fall["args"]]
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8", "TZ": "UTC",
               "HOME": str(root / "home")}
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
            return
        zeile = zeile_lesen(proc.stdout, FRIST)
        if zeile is None or not gleich(json.loads(zeile or b"null"), {"bereit": True}):
            raise Rot(f"Start: Bereitschaftszeile {{\"bereit\": true}} erwartet, bekommen {zeile!r}")
        sock_pfad = root / "s.sock"
        modus = os.lstat(sock_pfad).st_mode
        if not stat.S_ISSOCK(modus):
            raise Rot("Start: am Socketpfad liegt kein Socket")
        if f"{stat.S_IMODE(modus):04o}" != erw["socket_rechte"]:
            raise Rot(f"Start: Socket mit Rechten {stat.S_IMODE(modus):04o}, erwartet {erw['socket_rechte']}")
        if sys.platform.startswith("linux"):
            if os.stat(f"/proc/{proc.pid}/stat").st_uid != 0:
                # PR_SET_DUMPABLE 0 setzt den Eigentümer der Einträge unter /proc/<pid> auf root (gemessen 20.11.)
                raise Rot("Start: Prozess ist nicht PR_SET_DUMPABLE 0")
            kernabbild_grenze_pruefen(proc.pid)
        verbindung = None
        lesen = None
        antworten: list[bytes] = []
        for st in fall["steps"]:
            if "verbindung" in st:
                if verbindung:
                    verbindung.close()
                verbindung = None
            elif "write" in st:
                for rel, text in st["write"].items():
                    (root / rel).write_text(text, encoding="utf-8")
            elif "chmod" in st:
                if os.geteuid() == 0:
                    raise Rot("Fall mit chmod läuft nicht als root")
                for rel, modus in st["chmod"].items():
                    os.chmod(root / rel, int(modus, 8))
                    rechte.append(root / rel)
            else:
                if verbindung is None:
                    verbindung = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    verbindung.settimeout(FRIST)
                    verbindung.connect(str(sock_pfad))
                    lesen = verbindung.makefile("rb")
                if "request" in st:
                    daten = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":"))
                else:
                    daten = st["line"]
                verbindung.sendall(daten.encode("utf-8") + b"\n")
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
        proc.send_signal(signal.SIGTERM)
        try:
            code = proc.wait(FRIST)
        except subprocess.TimeoutExpired:
            raise Rot(f"Ende: nach SIGTERM läuft der Prozess nach {FRIST} s noch")
        if code != erw["ende"]["exit"]:
            raise Rot(f"Ende: Exit {code}, erwartet {erw['ende']['exit']}")
        if os.path.lexists(sock_pfad):
            raise Rot("Ende: Socket liegt nach dem Ende noch da")
        protokoll_pruefen(root, erw["protokoll"])
        if "stderr_zeile" in erw:
            zeilen = proc.stderr.read().decode("utf-8", "replace").splitlines()
            if zeilen.count(erw["stderr_zeile"]) != 1:
                raise Rot(f"stderr: die Zeile {erw['stderr_zeile']!r} steht {zeilen.count(erw['stderr_zeile'])}-mal da, erwartet 1-mal")
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
