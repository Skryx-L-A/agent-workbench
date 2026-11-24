#!/usr/bin/env python3
"""Läufer für das MCP-Werkzeug `such-mcp` (Vertrag 4, Abschnitt „MCP-Werkzeug im Einschluss“). Nur Standardbibliothek.

Aufruf: MCP_BIN=<programm> python3 pruefe_mcp.py [fall ...]
Ein Stand-in hört am Socket, schreibt jede empfangene Zeile mit und antwortet mit den Zeilen aus `socket_antworten`.
Exit 0: alle Fälle grün; 1: mindestens einer rot; 2: Aufruffehler.
"""
from __future__ import annotations

import json
import os
import selectors
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
FRIST = 10.0


class Rot(Exception):
    pass


def gleich(a, b) -> bool:
    if isinstance(a, bool) or isinstance(b, bool):
        return type(a) is type(b) and a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(gleich(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(gleich(x, y) for x, y in zip(a, b))
    return type(a) is type(b) and a == b


def enthaelt(gross, teil) -> bool:
    if isinstance(teil, dict):
        return isinstance(gross, dict) and all(k in gross and enthaelt(gross[k], v) for k, v in teil.items())
    return gleich(gross, teil)


def zeile_lesen(datei, frist: float) -> bytes | None:
    sel = selectors.DefaultSelector()
    sel.register(datei, selectors.EVENT_READ)
    puffer, ende, fd = b"", time.monotonic() + frist, datei.fileno()
    while not puffer.endswith(b"\n"):
        rest = ende - time.monotonic()
        if rest <= 0 or not sel.select(rest):
            return None
        stueck = os.read(fd, 1)
        if not stueck:
            return puffer or None
        puffer += stueck
    return puffer


class StandIn(threading.Thread):
    def __init__(self, pfad: Path, antworten: list[str]):
        super().__init__(daemon=True)
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(str(pfad))
        self.srv.listen(4)
        self.antworten = list(antworten)
        self.zeilen: list[object] = []

    def run(self):
        while True:
            try:
                conn, _ = self.srv.accept()
            except OSError:
                return
            with conn:
                datei = conn.makefile("rb")
                roh = datei.readline()
                try:
                    self.zeilen.append(json.loads(roh.decode("utf-8")))
                except ValueError:
                    self.zeilen.append({"_roh": roh.decode("utf-8", "replace")})
                antwort = self.antworten.pop(0) if self.antworten else '{"error":"unerwartet"}'
                conn.sendall(antwort.encode("utf-8") + b"\n")


def pruefen(nr: int, erw: dict, got: dict, werkzeug: dict) -> None:
    if got.get("jsonrpc") != "2.0" or not gleich(got.get("id"), erw["id"]):
        raise Rot(f"Schritt {nr}: jsonrpc/id falsch: {got}")
    if "fehlercode" in erw:
        if not isinstance(got.get("error"), dict) or got["error"].get("code") != erw["fehlercode"] or "result" in got:
            raise Rot(f"Schritt {nr}: Fehler {erw['fehlercode']} erwartet, bekommen {got}")
        return
    if "result" not in got or "error" in got:
        raise Rot(f"Schritt {nr}: Ergebnis erwartet, bekommen {got}")
    res = got["result"]
    if "result" in erw and not gleich(res, erw["result"]):
        raise Rot(f"Schritt {nr}: Ergebnis {res}, erwartet {erw['result']}")
    if "result_teil" in erw and not enthaelt(res, erw["result_teil"]):
        raise Rot(f"Schritt {nr}: Ergebnis {res} enthält nicht {erw['result_teil']}")
    if erw.get("werkzeugliste"):
        tools = res.get("tools")
        if not isinstance(tools, list) or len(tools) != 1:
            raise Rot(f"Schritt {nr}: genau ein Werkzeug erwartet, bekommen {tools}")
        t = tools[0]
        if t.get("name") != werkzeug["name"] or not gleich(t.get("inputSchema"), werkzeug["inputSchema"]):
            raise Rot(f"Schritt {nr}: Werkzeug {t}")
    if "werkzeug_text" in erw:
        if not gleich(res.get("content"), [{"type": "text", "text": erw["werkzeug_text"]}]):
            raise Rot(f"Schritt {nr}: content {res.get('content')}, erwartet Text {erw['werkzeug_text']!r}")
        if not gleich(res.get("isError"), erw["isError"]):
            raise Rot(f"Schritt {nr}: isError {res.get('isError')!r}, erwartet {erw['isError']!r}")


def fall_laufen(prog: str, fall: dict, werkzeug: dict) -> None:
    root = Path(tempfile.mkdtemp(prefix="mcp-", dir=os.environ.get("SOCKET_BASIS", "/tmp")))
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8", "TZ": "UTC", "HOME": str(root / "home")}
    (root / "home").mkdir()
    proc = None
    try:
        if "start_exit" in fall:
            lauf = subprocess.run([prog] + fall["argv"], stdin=subprocess.DEVNULL, capture_output=True, env=env,
                                  cwd=root, timeout=FRIST)
            if lauf.returncode != fall["start_exit"] or lauf.stdout:
                raise Rot(f"Start: Exit {lauf.returncode} und stdout {lauf.stdout[:80]!r}, erwartet Exit {fall['start_exit']}")
            return
        sock = root / "s.sock"
        stand_in = None
        if not fall.get("kein_socket"):
            stand_in = StandIn(sock, fall.get("socket_antworten", []))
            stand_in.start()
        proc = subprocess.Popen([prog, "--socket", str(sock)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, env=env, cwd=root)
        for nr, st in enumerate(fall["schritte"], 1):
            daten = st["roh"] if "roh" in st else json.dumps(st["sende"], ensure_ascii=False, separators=(",", ":"))
            proc.stdin.write(daten.encode("utf-8") + b"\n")
            proc.stdin.flush()
            if "erwarte" in st:
                roh = zeile_lesen(proc.stdout, FRIST)
                if roh is None:
                    raise Rot(f"Schritt {nr}: keine Antwort binnen {FRIST} s")
                try:
                    got = json.loads(roh.decode("utf-8"))
                except ValueError:
                    raise Rot(f"Schritt {nr}: Antwort ist kein JSON: {roh[:120]!r}")
                pruefen(nr, st["erwarte"], got, werkzeug)
        proc.stdin.close()
        try:
            code = proc.wait(FRIST)
        except subprocess.TimeoutExpired:
            raise Rot(f"Ende: nach dem Schließen von stdin läuft das Programm nach {FRIST} s noch")
        rest = proc.stdout.read()
        if code != 0 or rest:
            raise Rot(f"Ende: Exit {code}, übrige Ausgabe {rest[:120]!r}")
        if "socket_zeilen" in fall:
            got = stand_in.zeilen if stand_in else []
            if not gleich(got, fall["socket_zeilen"]):
                raise Rot(f"Socket: empfangen {got}, erwartet {fall['socket_zeilen']}")
    finally:
        if proc and proc.poll() is None:
            proc.kill()
            proc.wait()
        shutil.rmtree(root, ignore_errors=True)


def main() -> int:
    prog = os.environ.get("MCP_BIN", "")
    if not prog:
        print("pruefe_mcp.py: MCP_BIN fehlt", file=sys.stderr)
        return 2
    daten = json.loads((HIER / "faelle.json").read_text(encoding="utf-8"))
    namen = sys.argv[1:] or [f["name"] for f in daten["faelle"]]
    rot = 0
    for fall in (f for f in daten["faelle"] if f["name"] in namen):
        try:
            fall_laufen(prog, fall, daten["werkzeug"])
            print(f"grün  {fall['name']}")
        except (Rot, OSError, subprocess.TimeoutExpired) as exc:
            rot += 1
            print(f"ROT   {fall['name']}: {exc}")
    print(f"{len(namen) - rot} von {len(namen)} grün")
    return 1 if rot else 0


if __name__ == "__main__":
    sys.exit(main())
