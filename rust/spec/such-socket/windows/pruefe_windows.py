#!/usr/bin/env python3
"""Läufer für den Nachtrag „Such-Pipe unter Windows“ zu Vertrag 4. Nur Standardbibliothek.

  RANGKERN_BIN=<programm> python pruefe_windows.py such [--streng] [fall ...]
  MCP_BIN=<programm>      python pruefe_windows.py mcp  [--streng] [fall ...]
  python pruefe_windows.py relais <pipe>      Client für PEER_<KENNUNG> (Konto ohne AppContainer)
  python pruefe_windows.py standin <pipe>     Stand-in mit fremdem Besitzer für wm-fremder-besitzer

Unter Windows spricht der Läufer echte Named Pipes an (_winapi, ctypes). Auf Linux und macOS läuft er gegen eine
Attrappe: Der Prüfling bekommt WB_PIPE_ATTRAPPE=<ordner>; der Pipe-Name \\\\.\\pipe\\<n> ist dort der Unix-Socket
<ordner>/<n>, und der Peer jeder Verbindung steht in <ordner>/peer.json. Das prüft den Läufer, nicht Windows.

Exit 0: kein Fall rot. 1: mindestens ein Fall rot (mit --streng auch: einer ausgelassen). 2: Aufruffehler.
"""
from __future__ import annotations

import hashlib
import json
import os
import queue
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER.parent))
sys.path.insert(0, str(HIER.parent / "mcp"))
from pruefe import Rot, antwort_pruefen, gleich  # noqa: E402  (Vertrag 4)
import pruefe_mcp  # noqa: E402  (Vertrag 4, MCP: pruefen)

WINDOWS = sys.platform == "win32"
FRIST = 10.0
FRIST_STUMM = 8.0          # Vertrag: 5 s bis zur ersten Zeile; der Läufer lässt 3 s Spiel
PAKET_SID_TEST = "S-1-15-2-1111111111-2222222222-3333333333-444444444-555555555-666666666-777777777"
EIGEN_SID_ATTRAPPE = "S-1-5-21-1000000001-1000000002-1000000003-1001"
FELDER = {"ausgabe.gesperrt": {"aktionsname", "anzahl", "ergebnis", "fassung", "grund", "klasse", "kunde", "route",
                               "sitzung", "vorher", "zeit"},
          "kanal.abgewiesen": {"aktionsname", "ergebnis", "fassung", "grund", "kanal", "route", "sitzung", "vorher",
                               "zeit"},
          "probelauf.zaehlung": {"aktionsname", "anzahl", "ergebnis", "fassung", "programm", "sitzung", "vorher",
                                 "zeit"}}
ZEIT = re.compile(r"^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$")
ABGEWIESEN = object()
HALLO = b"pk1\n"          # peerkanal: der Client sendet es nach der Prüfung des Servers, der Server bei Annahme zurück

# Windows-Konstanten
GENERIC_READ, FILE_WRITE_DATA, READ_CONTROL = 0x80000000, 0x2, 0x20000
OPEN_EXISTING = 3
SQOS_IDENTIFIKATION = 0x00100000 | 0x00010000      # SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION
PIPE_ACCESS_DUPLEX, FIRST_PIPE_INSTANCE = 0x3, 0x00080000


def starten(argv, **kw) -> subprocess.Popen:
    """Unter Unix in eigener Prozessgruppe, damit kein Kind einer Attrappe übrig bleibt."""
    return subprocess.Popen(argv, start_new_session=not WINDOWS, **kw)


def beenden(proc: subprocess.Popen | None) -> None:
    if proc is None:
        return
    if not WINDOWS:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    if proc.poll() is None:
        proc.kill()
    proc.wait()


class Aus(Exception):
    """Fall ausgelassen: nicht an diesem Ort lauffähig. Zählt nie als grün."""


# ---------------------------------------------------------------- Kanäle

class Kanal:
    """Eine Verbindung mit Lesefaden: zeile() gibt eine Zeile, b"" für das Ende ohne Bytes oder None nach der Frist."""

    def __init__(self, lesen, schreiben, schliessen):
        self._schreiben, self._schliessen = schreiben, schliessen
        self._q: queue.Queue = queue.Queue()
        self._puffer = b""
        self._ende = False

        def faden():
            try:
                while True:
                    stueck = lesen()
                    if not stueck:
                        break
                    self._q.put(stueck)
            except OSError:
                pass
            self._q.put(None)
        threading.Thread(target=faden, daemon=True).start()

    def senden(self, daten: bytes) -> None:
        self._schreiben(daten)

    def zeile(self, frist: float):
        ende = time.monotonic() + frist
        while b"\n" not in self._puffer:
            if self._ende:
                rest, self._puffer = self._puffer, b""
                return rest
            try:
                stueck = self._q.get(timeout=max(0.0, ende - time.monotonic()))
            except queue.Empty:
                return None
            if stueck is None:
                self._ende = True
            else:
                self._puffer += stueck
        z, _, self._puffer = self._puffer.partition(b"\n")
        return z + b"\n"

    def zu(self) -> None:
        try:
            self._schliessen()
        except OSError:
            pass


def unix_kanal(pfad: Path) -> Kanal:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(str(pfad))
    return Kanal(lambda: s.recv(65536), s.sendall, s.close)


def win_kanal(pipe: str, zugriff: int = GENERIC_READ | FILE_WRITE_DATA) -> Kanal:
    import _winapi
    h = _winapi.CreateFile(pipe, zugriff, 0, 0, OPEN_EXISTING, SQOS_IDENTIFIKATION, 0)

    def lesen():
        try:
            daten, _ = _winapi.ReadFile(h, 65536)
            return daten
        except BrokenPipeError:
            return b""
    return Kanal(lesen, lambda d: _winapi.WriteFile(h, d), lambda: _winapi.CloseHandle(h))


def prozess_kanal(befehl: list[str], pipe: str) -> tuple[Kanal | None, int | None, subprocess.Popen]:
    """Client-Befehl nach README Abschnitt 7: erste Zeile {"oeffnen_fehler": …}, dann Weiterleitung."""
    p = subprocess.Popen(befehl + [pipe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def schreiben(d):
        p.stdin.write(d)
        p.stdin.flush()

    def zu():
        p.stdin.close()
        try:
            p.wait(FRIST)
        except subprocess.TimeoutExpired:
            p.kill()
    k = Kanal(lambda: p.stdout.read1(65536), schreiben, zu)
    kopf = k.zeile(FRIST)
    if not kopf:
        k.zu()
        raise Rot(f"Client {befehl[0]}: keine Kopfzeile")
    fehler = json.loads(kopf)["oeffnen_fehler"]
    if fehler is not None:
        k.zu()
        return None, fehler, p
    return k, None, p


def hallo(k: Kanal, wert: bytes = HALLO) -> bool:
    """Sendet das Hallo und liest die Antwort: True bei Annahme, False bei Ende ohne ein Byte."""
    k.senden(wert)
    r = k.zeile(FRIST)
    if r is None:
        raise Rot(f"Hallo: binnen {FRIST} s weder Hallo noch Ende")
    if r == b"":
        return False
    if r != HALLO:
        raise Rot(f"Hallo: {HALLO!r} oder Ende erwartet, bekommen {r!r}")
    return True


# ---------------------------------------------------------------- Kontext eines Falls

def peer_eintrag(peer: str, sitzung: str) -> dict:
    """Was peerkanal und Plattformdienst in der Attrappe für diese Kennung melden."""
    gemessen = {"sid": EIGEN_SID_ATTRAPPE, "appcontainer_sid": PAKET_SID_TEST, "pid": 4242, "startzeit": 133000000000000000}
    dienst = {"ok": True, "pid": 4242, "start": 133000000000000000, "sid": PAKET_SID_TEST, "sitzung": sitzung}
    if peer == "person":
        gemessen["appcontainer_sid"] = None
    elif peer == "andere-sitzung":
        dienst["sitzung"] = "s-test-2"
    elif peer == "ohne-lauf":
        dienst = {"ok": False, "code": "dienst.peer_unbekannt"}
    elif peer == "dienst-stumm":
        dienst = "stumm"
    elif peer == "zeit-falsch":
        dienst["start"] = 133000000000000001
    elif peer != "agent":
        raise Aus(f"Kennung {peer} bildet die Attrappe nicht nach")
    return gemessen | {"dienst": dienst}


class Ort:
    def __init__(self, root: Path):
        self.root = root
        self.pipe = "\\\\.\\pipe\\wb-such-" + secrets.token_hex(16)
        self.attrappe = None if WINDOWS else Path(tempfile.mkdtemp(prefix="wp-", dir=os.environ.get("SOCKET_BASIS", "/tmp")))
        self.prozesse: list[subprocess.Popen] = []
        self.belegt = None
        if self.attrappe:
            (self.attrappe / "token.json").write_text(json.dumps({"sid": EIGEN_SID_ATTRAPPE}), encoding="utf-8")

    def pfad(self, pipe: str | None = None) -> Path:
        return self.attrappe / (pipe or self.pipe).rsplit("\\", 1)[1]

    def env(self) -> dict:
        env = {"PATH": os.environ.get("PATH", ""), "LANG": "C.UTF-8", "TZ": "UTC", "HOME": str(self.root / "home")}
        if WINDOWS:
            env["SYSTEMROOT"] = os.environ.get("SYSTEMROOT", "C:\\Windows")   # A124
        else:
            env["WB_PIPE_ATTRAPPE"] = str(self.attrappe)
        return env

    def ersetzen(self, wert: str) -> str:
        return (wert.replace("{root}", str(self.root)).replace("{pipe}", self.pipe)
                .replace("{paket_sid}", os.environ.get("PAKET_SID", PAKET_SID_TEST) if WINDOWS else PAKET_SID_TEST))

    def oeffnen(self, peer: str, sitzung: str) -> tuple[Kanal | None, int | None]:
        if not WINDOWS:
            (self.attrappe / "peer.json").write_text(json.dumps(peer_eintrag(peer, sitzung)), encoding="utf-8")
            return unix_kanal(self.pfad()), None
        if peer == "person":
            try:
                return win_kanal(self.pipe), None
            except OSError as exc:
                return None, exc.winerror
        befehl = client_befehl(peer)
        k, fehler, p = prozess_kanal(befehl, self.pipe)
        self.prozesse.append(p)
        return k, fehler

    def vorbelegen(self) -> None:
        if WINDOWS:
            import _winapi
            self.belegt = _winapi.CreateNamedPipe(self.pipe, PIPE_ACCESS_DUPLEX | FIRST_PIPE_INSTANCE, 0, 1, 512, 512, 0, 0)
        else:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.bind(str(self.pfad()))
            s.listen(1)
            self.belegt = s

    def vorbelegt_heil(self) -> bool:
        if WINDOWS:
            import _winapi
            try:
                h = _winapi.CreateFile(self.pipe, GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0)
            except OSError:
                return False
            _winapi.CloseHandle(h)
            return True
        return self.pfad().exists()

    def pipe_weg(self) -> bool:
        ende = time.monotonic() + 2
        while time.monotonic() < ende:
            if WINDOWS:
                import _winapi
                try:
                    _winapi.CloseHandle(_winapi.CreateFile(self.pipe, READ_CONTROL, 0, 0, OPEN_EXISTING, 0, 0))
                except FileNotFoundError:
                    return True
                except OSError as exc:
                    if exc.winerror == 2:
                        return True
            elif not os.path.lexists(self.pfad()):
                return True
            time.sleep(0.1)
        return False

    def aufraeumen(self) -> None:
        for p in self.prozesse:
            if p.poll() is None:
                p.kill()
                p.wait()
        if self.belegt is not None:
            if WINDOWS:
                import _winapi
                _winapi.CloseHandle(self.belegt)
            else:
                self.belegt.close()
        if self.attrappe:
            shutil.rmtree(self.attrappe, ignore_errors=True)


def client_befehl(peer: str) -> list[str]:
    name = "PEER_" + peer.upper().replace("-", "_")
    wert = os.environ.get(name)
    if not wert:
        raise Aus(f"{name} fehlt (Client für die Kennung {peer})")
    befehl = json.loads(wert)
    if not isinstance(befehl, list) or not all(isinstance(x, str) for x in befehl):
        raise Rot(f"{name} ist keine JSON-Liste von Texten")
    return befehl


def ort_pruefen(orte: list[str]) -> None:
    if not WINDOWS:
        if "attrappe" not in orte:
            raise Aus("nur unter Windows prüfbar (" + ", ".join(orte) + ")")
    elif orte == ["attrappe"]:
        raise Aus("nur in der Attrappe")


# ---------------------------------------------------------------- Windows: Sicherheit der Pipe (ctypes)

def eigene_sid() -> str:
    import ctypes
    from ctypes import wintypes
    adv = ctypes.WinDLL("advapi32", use_last_error=True)
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    k32.GetCurrentProcess.restype = wintypes.HANDLE
    tok = wintypes.HANDLE()
    if not adv.OpenProcessToken(k32.GetCurrentProcess(), 0x8, ctypes.byref(tok)):
        raise ctypes.WinError(ctypes.get_last_error())
    groesse = wintypes.DWORD()
    adv.GetTokenInformation(tok, 1, None, 0, ctypes.byref(groesse))
    puffer = ctypes.create_string_buffer(groesse.value)
    if not adv.GetTokenInformation(tok, 1, puffer, groesse, ctypes.byref(groesse)):
        raise ctypes.WinError(ctypes.get_last_error())
    psid = ctypes.cast(puffer, ctypes.POINTER(ctypes.c_void_p))[0]
    text = wintypes.LPWSTR()
    if not adv.ConvertSidToStringSidW(ctypes.c_void_p(psid), ctypes.byref(text)):
        raise ctypes.WinError(ctypes.get_last_error())
    wert = text.value
    k32.LocalFree(text)
    k32.CloseHandle(tok)
    return wert


def sddl_der_pipe(handle: int) -> str:
    import ctypes
    from ctypes import wintypes
    adv = ctypes.WinDLL("advapi32", use_last_error=True)
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)
    info = 0x1 | 0x2 | 0x4 | 0x10        # OWNER, GROUP, DACL, LABEL
    psd = ctypes.c_void_p()
    leer = ctypes.c_void_p()
    rc = adv.GetSecurityInfo(wintypes.HANDLE(handle), 6, info, ctypes.byref(leer), ctypes.byref(leer),
                             ctypes.byref(leer), None, ctypes.byref(psd))
    if rc:
        raise ctypes.WinError(rc)
    text = wintypes.LPWSTR()
    if not adv.ConvertSecurityDescriptorToStringSecurityDescriptorW(psd, 1, info, ctypes.byref(text), None):
        raise ctypes.WinError(ctypes.get_last_error())
    wert = text.value
    k32.LocalFree(text)
    k32.LocalFree(psd)
    return wert


RECHTE = {"GA": 0x10000000, "GR": 0x80000000, "GW": 0x40000000, "GX": 0x20000000, "FA": 0x1F01FF, "FR": 0x120089,
          "FW": 0x120116, "FX": 0x1200A0, "RC": 0x20000, "SD": 0x10000, "WD": 0x40000, "WO": 0x80000, "CC": 0x1,
          "DC": 0x2, "LC": 0x4, "SW": 0x8, "RP": 0x10, "WP": 0x20, "DT": 0x40, "LO": 0x80, "CR": 0x100,
          "NW": 0x1, "NR": 0x2, "NX": 0x4}
KUERZEL_SID = {"LW": "S-1-16-4096", "ME": "S-1-16-8192", "SY": "S-1-5-18", "BA": "S-1-5-32-544", "WD": "S-1-1-0",
               "AC": "S-1-15-2-1", "OW": "S-1-3-4"}


def maske(text: str) -> int:
    if text.lower().startswith("0x"):
        return int(text, 16)
    return sum(RECHTE[text[i:i + 2]] for i in range(0, len(text), 2))


def sddl_zerlegen(sddl: str) -> dict:
    m = re.fullmatch(r"O:(?P<o>.+?)(G:(?P<g>.+?))?D:(?P<df>[A-Z]*)(?P<d>(\([^)]*\))*)(S:(?P<sf>[A-Z]*)(?P<s>(\([^)]*\))*))?", sddl)
    if not m:
        raise Rot(f"SDDL nicht lesbar: {sddl}")
    sid = lambda s: KUERZEL_SID.get(s, s)  # noqa: E731
    aces = [a.split(";") for a in re.findall(r"\(([^)]*)\)", m["d"])]
    label = [a.split(";") for a in re.findall(r"\(([^)]*)\)", m["s"] or "")]
    return {"besitzer": sid(m["o"]), "dacl_flags": m["df"],
            "aces": sorted([a[0], a[1], maske(a[2]), sid(a[5])] for a in aces),
            "label": [[a[0], maske(a[2]), sid(a[5])] for a in label]}


def sicherheit_pruefen(ort: Ort, erw: dict) -> None:
    import _winapi
    h = _winapi.CreateFile(ort.pipe, READ_CONTROL, 0, 0, OPEN_EXISTING, SQOS_IDENTIFIKATION, 0)
    try:
        got = sddl_zerlegen(sddl_der_pipe(h))
    finally:
        _winapi.CloseHandle(h)
    ich = eigene_sid()
    paket = ort.ersetzen("{paket_sid}")
    sub = lambda s: s.replace("{eigene_sid}", ich).replace("{paket_sid}", paket)  # noqa: E731
    soll = {"besitzer": sub(erw["besitzer"]), "dacl_flags": erw["dacl_flags"],
            "aces": sorted([a[0], "", int(a[1], 16), sub(a[2])] for a in erw["aces"]),
            "label": [[erw["label"][0], RECHTE[erw["label"][1]], erw["label"][2]]]}
    if got != soll:
        raise Rot(f"Sicherheit der Pipe: {got}, erwartet {soll}")


# ---------------------------------------------------------------- Protokoll

def protokoll_pruefen(root: Path, erwartet: list[dict]) -> None:
    roh: list[str] = []
    ordner = root / "protokoll"
    if ordner.is_dir():
        for datei in sorted(ordner.glob("brain-*.jsonl")):
            inhalt = datei.read_bytes()
            if inhalt and not inhalt.endswith(b"\n"):
                raise Rot(f"Protokoll {datei.name}: letzte Zeile ohne LF")
            roh += inhalt.decode("utf-8").splitlines()
    if len(roh) != len(erwartet):
        raise Rot(f"Protokoll: {len(roh)} Zeilen, erwartet {len(erwartet)}")
    vorher = "0" * 64
    for i, (text, e) in enumerate(zip(roh, erwartet), 1):
        z = json.loads(text)
        felder = FELDER.get(z.get("aktionsname"))
        if felder is None or set(z) != felder:
            raise Rot(f"Protokollzeile {i}: Felder {sorted(z)}")
        if not ZEIT.match(z["zeit"]):
            raise Rot(f"Protokollzeile {i}: zeit {z['zeit']!r}")
        if z["vorher"] != vorher:
            raise Rot(f"Protokollzeile {i}: vorher ist nicht der sha256 der vorigen Zeile")
        vorher = hashlib.sha256(text.encode("utf-8")).hexdigest()
        for feld, wert in e.items():
            if not gleich(z[feld], wert):
                raise Rot(f"Protokollzeile {i}: {feld} erwartet {wert!r}, bekommen {z[feld]!r}")


# ---------------------------------------------------------------- Such-Prozess

def zaehlung_pruefen(root: Path, mz: dict) -> None:
    """Zähldatei des Probelaufs wie in Vertrag 4. Rechte 0600 nur unter POSIX; unter Windows regelt das eine ACL (offen)."""
    datei = root / mz["datei"]
    if not datei.is_file():
        raise Rot("Zählung: Datei fehlt")
    if not WINDOWS and f"{datei.stat().st_mode & 0o777:04o}" != "0600":
        raise Rot(f"Zählung: Rechte {datei.stat().st_mode & 0o777:04o}, erwartet 0600")
    roh = datei.read_bytes()
    got = json.loads(roh.decode("utf-8"))
    kanonisch = json.dumps(got, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    if roh.rstrip(b"\n") != kanonisch:
        raise Rot("Zählung: Datei nicht in kanonischer Form (RFC 8785)")
    if not gleich(got, mz["inhalt"]):
        raise Rot(f"Zählung: {got}, erwartet {mz['inhalt']}")


def such_fall(prog: str, fall: dict, erw: dict) -> None:
    ort_pruefen(fall["orte"])
    root = Path(tempfile.mkdtemp(prefix="sw-"))
    ort = Ort(root)
    proc = None
    rechte: list[Path] = []
    kanaele: list[Kanal] = []
    try:
        for d in fall["dirs"]:
            (root / d).mkdir(parents=True, exist_ok=True)
        for rel, text in fall["files"].items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        if fall.get("vorbelegen"):
            ort.vorbelegen()
        args = [ort.ersetzen(a) for a in fall["args"]]
        sitzung = args[args.index("--sitzung") + 1] if "--sitzung" in args else ""
        proc = starten([prog] + args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE if "stderr_zeile" in erw else subprocess.DEVNULL, env=ort.env(),
                       cwd=root)
        aus = Kanal(lambda: proc.stdout.read1(65536), lambda d: None, lambda: None)
        if erw["start"] != "bereit":
            try:
                code = proc.wait(FRIST)
            except subprocess.TimeoutExpired:
                raise Rot(f"Start: Exit {erw['start']['exit']} erwartet, Prozess läuft nach {FRIST} s noch")
            if code != erw["start"]["exit"]:
                raise Rot(f"Start: Exit {code}, erwartet {erw['start']['exit']}")
            if aus.zeile(1.0):
                raise Rot("Start: stdout nicht leer")
            if fall.get("vorbelegen") and not ort.vorbelegt_heil():
                raise Rot("Start: die vorher belegte Pipe ist nicht mehr da")
            return
        zeile = aus.zeile(FRIST)
        if zeile != b'{"bereit":true}\n':
            raise Rot(f"Start: Bereitschaftszeile erwartet, bekommen {zeile!r}")
        antworten: list = []
        k = None
        k_peer = None
        for st in fall["steps"]:
            peer = st.get("peer", "agent")
            if "verbindung" in st:
                if k:
                    k.zu()
                k = None
            elif "write" in st:
                for rel, text in st["write"].items():
                    (root / rel).write_text(text, encoding="utf-8")
            elif "chmod" in st:
                for rel, modus in st["chmod"].items():
                    os.chmod(root / rel, int(modus, 8))
                    rechte.append(root / rel)
            elif "sicherheit" in st:
                sicherheit_pruefen(ort, erw["sicherheit"])
            elif "probe" in st:
                befehl = client_befehl("probe-agent")
                ziel = str(proc.pid) if st["probe"] == "prozess" else ort.pipe
                lauf = subprocess.run(befehl + [st["probe"], ziel], capture_output=True, timeout=FRIST)
                antworten.append({"probe_fehler": json.loads(lauf.stdout)["fehler"]})
            elif "zugleich" in st:
                offen = []
                for _ in range(st["zugleich"]):
                    kk, fehler = ort.oeffnen(peer, sitzung)
                    if kk is None:
                        raise Rot(f"zugleich: Öffnen scheitert mit {fehler}")
                    offen.append(kk)
                    kanaele.append(kk)
                for kk in offen:
                    if not hallo(kk):
                        raise Rot("zugleich: Verbindung abgewiesen")
                daten = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":")).encode() + b"\n"
                for kk in offen:
                    kk.senden(daten)
                for kk in offen:
                    antworten.append(kk.zeile(FRIST))
                for kk in offen:
                    kk.zu()
            elif "stumm" in st:
                kk, fehler = ort.oeffnen(peer, sitzung)
                if kk is None:
                    raise Rot(f"stumm: Öffnen scheitert mit {fehler}")
                kanaele.append(kk)
                r = kk.zeile(FRIST_STUMM)
                antworten.append(ABGEWIESEN if r == b"" else r)
                kk.zu()
            else:
                if k is None or peer != k_peer:
                    if k:
                        k.zu()
                    k, fehler = ort.oeffnen(peer, sitzung)
                    k_peer = peer
                    if k is None:
                        antworten.append({"oeffnen_fehler": fehler})
                        continue
                    kanaele.append(k)
                    wert = (st["hallo"].encode("utf-8") + b"\n") if "hallo" in st else HALLO
                    if not hallo(k, wert):
                        antworten.append(ABGEWIESEN)
                        k.zu()
                        k = None
                        continue
                    if "hallo" in st:
                        raise Rot(f"Hallo {st['hallo']!r} wurde angenommen")
                daten = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":")) if "request" in st else st["line"]
                k.senden(daten.encode("utf-8") + b"\n")
                r = k.zeile(FRIST)
                if r is None:
                    raise Rot(f"Anfrage {len(antworten) + 1}: binnen {FRIST} s weder Antwort noch Ende")
                if r == b"":
                    # nach angenommenem Hallo ist ein Ende ohne Antwort keine Abweisung, sondern ein Fehler
                    antworten.append("Ende nach angenommenem Hallo")
                    k.zu()
                    k = None
                else:
                    antworten.append(r)
        if k:
            k.zu()
        for p in rechte:
            os.chmod(p, 0o755)
        if len(antworten) != len(erw["responses"]):
            raise Rot(f"{len(antworten)} Antworten, erwartet {len(erw['responses'])}")
        for nr, (e, got) in enumerate(zip(erw["responses"], antworten), 1):
            if isinstance(got, bytes):
                klein = got.decode("utf-8", "replace").lower()
                for marke in fall["verboten"]:
                    if marke.lower() in klein:
                        raise Rot(f"Antwort {nr}: enthält die verbotene Marke {marke!r}")
            if e == {"abgewiesen": True}:
                if got is not ABGEWIESEN:
                    raise Rot(f"Antwort {nr}: Abweisung ohne Byte erwartet, bekommen {got!r}")
            elif "oeffnen_fehler" in e or "probe_fehler" in e:
                if not gleich(got, e):
                    raise Rot(f"Antwort {nr}: {e} erwartet, bekommen {got!r}")
            elif not isinstance(got, bytes):
                raise Rot(f"Antwort {nr}: Antwort erwartet, bekommen {'Abweisung' if got is ABGEWIESEN else got}")
            else:
                antwort_pruefen(nr, e, got)
        if WINDOWS:
            proc.terminate()
            proc.wait(FRIST)
        else:
            proc.send_signal(signal.SIGTERM)
            try:
                code = proc.wait(FRIST)
            except subprocess.TimeoutExpired:
                raise Rot(f"Ende: nach SIGTERM läuft der Prozess nach {FRIST} s noch")
            if code != erw["ende"]["exit_attrappe"]:
                raise Rot(f"Ende: Exit {code}, erwartet {erw['ende']['exit_attrappe']}")
        if erw["ende"]["pipe_weg"] and not ort.pipe_weg():
            raise Rot("Ende: der Pipe-Name ist nach dem Ende noch belegt")
        protokoll_pruefen(root, erw["protokoll"])
        if "stderr_zeile" in erw:
            zeilen = proc.stderr.read().decode("utf-8", "replace").splitlines()
            if zeilen.count(erw["stderr_zeile"]) != 1:
                raise Rot(f"stderr: die Zeile {erw['stderr_zeile']!r} steht {zeilen.count(erw['stderr_zeile'])}-mal da, erwartet 1-mal")
        if "marker_zaehlung" in erw:
            zaehlung_pruefen(root, erw["marker_zaehlung"])
    finally:
        for kk in kanaele:
            kk.zu()
        beenden(proc)
        for p in rechte:
            if p.exists():
                os.chmod(p, 0o755)
        ort.aufraeumen()
        shutil.rmtree(root, ignore_errors=True)


# ---------------------------------------------------------------- such-mcp

class UnixStandIn(threading.Thread):
    """Stand-in am Unix-Socket der Attrappe: liest das Hallo, antwortet (oder schließt), dann eine Zeile."""

    def __init__(self, pfad: Path, antworten: list[str], hallo_antwort: bool):
        super().__init__(daemon=True)
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(str(pfad))
        self.srv.listen(4)
        self.antworten, self.hallo_antwort = list(antworten), hallo_antwort
        self.zeilen: list = []
        self.stufe = None

    def run(self):
        while True:
            try:
                conn, _ = self.srv.accept()
            except OSError:
                return
            with conn, conn.makefile("rb") as datei:     # beide schließen, sonst bleibt der Socket offen
                h = datei.readline()
                if h != HALLO:
                    self.zeilen.append({"_hallo": h.decode("utf-8", "replace")})
                    continue
                if not self.hallo_antwort:
                    continue
                conn.sendall(HALLO)
                roh = datei.readline()
                try:
                    self.zeilen.append(json.loads(roh.decode("utf-8")))
                except ValueError:
                    self.zeilen.append({"_roh": roh.decode("utf-8", "replace")})
                antwort = self.antworten.pop(0) if self.antworten else '{"error":"unerwartet"}'
                conn.sendall(antwort.encode("utf-8") + b"\n")


class WinStandIn(threading.Thread):
    """Stand-in als Named Pipe der Person; misst auf Wunsch die Stufe der Impersonation."""

    def __init__(self, pipe: str, antworten: list[str], stufe_messen: bool, hallo_antwort: bool):
        super().__init__(daemon=True)
        self.pipe, self.antworten, self.stufe_messen = pipe, list(antworten), stufe_messen
        self.hallo_antwort = hallo_antwort
        self.zeilen: list = []
        self.stufe = None
        self.halt = False
        import _winapi
        self.h = _winapi.CreateNamedPipe(pipe, PIPE_ACCESS_DUPLEX | FIRST_PIPE_INSTANCE, 0, 1, 65536, 65536, 0, 0)

    def run(self):
        import ctypes
        import _winapi
        k32 = ctypes.WinDLL("kernel32", use_last_error=True)
        adv = ctypes.WinDLL("advapi32", use_last_error=True)
        while not self.halt:
            try:
                _winapi.ConnectNamedPipe(self.h)
            except OSError as exc:
                if exc.winerror != 535:      # ERROR_PIPE_CONNECTED
                    return
            if self.halt:
                return
            def zeile_lesen():
                puffer = b""
                try:
                    while not puffer.endswith(b"\n"):
                        stueck, _ = _winapi.ReadFile(self.h, 1)
                        if not stueck:
                            break
                        puffer += stueck
                except OSError:
                    pass
                return puffer
            h = zeile_lesen()
            if h != HALLO or not self.hallo_antwort:
                if h != HALLO:
                    self.zeilen.append({"_hallo": h.decode("utf-8", "replace")})
                k32.DisconnectNamedPipe(ctypes.c_void_p(self.h))
                continue
            _winapi.WriteFile(self.h, HALLO)
            puffer = zeile_lesen()
            if self.stufe_messen and adv.ImpersonateNamedPipeClient(self.h):
                tok = ctypes.c_void_p()
                k32.GetCurrentThread.restype = ctypes.c_void_p
                if adv.OpenThreadToken(ctypes.c_void_p(k32.GetCurrentThread()), 0x8, True, ctypes.byref(tok)):
                    wert = ctypes.c_int()
                    n = ctypes.c_ulong()
                    if adv.GetTokenInformation(tok, 9, ctypes.byref(wert), 4, ctypes.byref(n)):
                        self.stufe = {0: "anonym", 1: "identifikation", 2: "impersonation", 3: "delegation"}[wert.value]
                    k32.CloseHandle(tok)
                adv.RevertToSelf()
            try:
                self.zeilen.append(json.loads(puffer.decode("utf-8")))
            except ValueError:
                self.zeilen.append({"_roh": puffer.decode("utf-8", "replace")})
            antwort = self.antworten.pop(0) if self.antworten else '{"error":"unerwartet"}'
            try:
                _winapi.WriteFile(self.h, antwort.encode("utf-8") + b"\n")
                k32.FlushFileBuffers(ctypes.c_void_p(self.h))
            except OSError:
                pass
            k32.DisconnectNamedPipe(ctypes.c_void_p(self.h))

    def stopp(self):
        import _winapi
        self.halt = True
        try:
            _winapi.CloseHandle(_winapi.CreateFile(self.pipe, GENERIC_READ, 0, 0, OPEN_EXISTING, 0, 0))
        except OSError:
            pass
        self.join(2)
        _winapi.CloseHandle(self.h)


def mcp_fall(prog: str, fall: dict, werkzeug: dict) -> None:
    ort_pruefen(fall["orte"])
    root = Path(tempfile.mkdtemp(prefix="mw-"))
    (root / "home").mkdir()
    ort = Ort(root)
    proc = None
    stand_in = None
    fremd = None
    try:
        argv = [ort.ersetzen(a) for a in fall.get("argv", [])]
        if "start_exit" in fall:
            lauf = subprocess.run([prog] + argv, stdin=subprocess.DEVNULL, capture_output=True, env=ort.env(), cwd=root,
                                  timeout=FRIST)
            if lauf.returncode != fall["start_exit"] or lauf.stdout:
                raise Rot(f"Start: Exit {lauf.returncode} und stdout {lauf.stdout[:80]!r}, erwartet Exit {fall['start_exit']}")
            return
        if fall.get("standin") == "fremdes-konto":
            wert = os.environ.get("STANDIN_FREMDES_KONTO")
            if not wert:
                raise Aus("STANDIN_FREMDES_KONTO fehlt")
            fremd = subprocess.Popen(json.loads(wert) + [ort.pipe], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            if fremd.stdout.readline() != b'{"bereit":true}\n':
                raise Rot("Stand-in des fremden Kontos meldet sich nicht")
        elif not fall.get("kein_socket"):
            if WINDOWS:
                stand_in = WinStandIn(ort.pipe, fall.get("socket_antworten", []), fall.get("stufe_messen", False),
                                      fall.get("hallo_antwort", True))
            else:
                stand_in = UnixStandIn(ort.pfad(), fall.get("socket_antworten", []), fall.get("hallo_antwort", True))
            stand_in.start()
        proc = starten([prog] + argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                       env=ort.env(), cwd=root)
        aus = Kanal(lambda: proc.stdout.read1(65536), lambda d: None, lambda: None)
        for nr, st in enumerate(fall["schritte"], 1):
            daten = st["roh"] if "roh" in st else json.dumps(st["sende"], ensure_ascii=False, separators=(",", ":"))
            proc.stdin.write(daten.encode("utf-8") + b"\n")
            proc.stdin.flush()
            if "erwarte" in st:
                roh = aus.zeile(FRIST + 2)
                if not roh:
                    raise Rot(f"Schritt {nr}: keine Antwort binnen {FRIST + 2} s")
                try:
                    got = json.loads(roh.decode("utf-8"))
                except ValueError:
                    raise Rot(f"Schritt {nr}: Antwort ist kein JSON: {roh[:120]!r}")
                pruefe_mcp.pruefen(nr, st["erwarte"], got, werkzeug)
        proc.stdin.close()
        try:
            code = proc.wait(FRIST)
        except subprocess.TimeoutExpired:
            raise Rot(f"Ende: nach dem Schließen von stdin läuft das Programm nach {FRIST} s noch")
        rest = aus.zeile(0.5)
        if code != 0 or rest:
            raise Rot(f"Ende: Exit {code}, übrige Ausgabe {rest!r}")
        if fremd is not None:
            fremd.stdin.close()
            zeilen = json.loads(fremd.stdout.readline())["zeilen"]
        else:
            zeilen = stand_in.zeilen if stand_in else []
        if "socket_zeilen" in fall and not gleich(zeilen, fall["socket_zeilen"]):
            raise Rot(f"Pipe: empfangen {zeilen}, erwartet {fall['socket_zeilen']}")
        if "stufe" in fall and stand_in.stufe != fall["stufe"]:
            raise Rot(f"Stufe der Impersonation {stand_in.stufe}, erwartet {fall['stufe']}")
    finally:
        beenden(proc)
        if stand_in is not None:
            if WINDOWS:
                stand_in.stopp()
            else:
                stand_in.srv.close()
        if fremd is not None and fremd.poll() is None:
            fremd.kill()
            fremd.wait()
        ort.aufraeumen()
        shutil.rmtree(root, ignore_errors=True)


# ---------------------------------------------------------------- Hilfsrollen in der VM

def relais(pipe: str) -> int:
    """Client eines Kontos ohne AppContainer: Kopfzeile, dann stdin an die Pipe und die Pipe an stdout."""
    try:
        k = win_kanal(pipe)
    except OSError as exc:
        sys.stdout.buffer.write(json.dumps({"oeffnen_fehler": exc.winerror}).encode() + b"\n")
        return 0
    sys.stdout.buffer.write(b'{"oeffnen_fehler":null}\n')
    sys.stdout.buffer.flush()

    def hin():
        for zeile in sys.stdin.buffer:
            k.senden(zeile)
    threading.Thread(target=hin, daemon=True).start()
    while True:
        z = k.zeile(3600)
        if not z:
            return 0
        sys.stdout.buffer.write(z)
        sys.stdout.buffer.flush()


def standin(pipe: str) -> int:
    """Stand-in eines fremden Kontos: DACL für alle, Besitzer ist dieses Konto. Bedient eine Verbindung."""
    import ctypes
    from ctypes import wintypes
    import _winapi
    adv = ctypes.WinDLL("advapi32", use_last_error=True)
    k32 = ctypes.WinDLL("kernel32", use_last_error=True)

    class SA(ctypes.Structure):
        _fields_ = [("nLength", wintypes.DWORD), ("lpSecurityDescriptor", ctypes.c_void_p), ("bInheritHandle", wintypes.BOOL)]
    psd = ctypes.c_void_p()
    if not adv.ConvertStringSecurityDescriptorToSecurityDescriptorW("D:P(A;;0x12019f;;;WD)S:(ML;;NW;;;LW)", 1,
                                                                    ctypes.byref(psd), None):
        raise ctypes.WinError(ctypes.get_last_error())
    sa = SA(ctypes.sizeof(SA), psd, False)
    k32.CreateNamedPipeW.restype = ctypes.c_void_p
    h = k32.CreateNamedPipeW(pipe, PIPE_ACCESS_DUPLEX | FIRST_PIPE_INSTANCE, 0, 1, 65536, 65536, 0, ctypes.byref(sa))
    if h in (None, ctypes.c_void_p(-1).value):
        raise ctypes.WinError(ctypes.get_last_error())
    zeilen: list = []

    def bedienen():
        try:
            _winapi.ConnectNamedPipe(h)
        except OSError:
            pass
        try:
            daten, _ = _winapi.ReadFile(h, 65536)
            if daten:
                zeilen.append({"_hallo": daten.decode("utf-8", "replace")})
                _winapi.WriteFile(h, HALLO)
                daten, _ = _winapi.ReadFile(h, 65536)
                if daten:
                    zeilen.append(json.loads(daten.decode("utf-8")))
                _winapi.WriteFile(h, b'{"fallback":true,"hits":[],"gesperrt":0}\n')
        except (OSError, ValueError):
            pass
    threading.Thread(target=bedienen, daemon=True).start()
    sys.stdout.buffer.write(b'{"bereit":true}\n')
    sys.stdout.buffer.flush()
    sys.stdin.buffer.read()
    sys.stdout.buffer.write(json.dumps({"zeilen": zeilen}).encode() + b"\n")
    return 0


# ---------------------------------------------------------------- Rahmen

def main() -> int:
    argv = sys.argv[1:]
    if len(argv) == 2 and argv[0] in ("relais", "standin"):
        if not WINDOWS:
            print(f"pruefe_windows.py: {argv[0]} gibt es nur unter Windows", file=sys.stderr)
            return 2
        return relais(argv[1]) if argv[0] == "relais" else standin(argv[1])
    if not argv or argv[0] not in ("such", "mcp"):
        print(__doc__, file=sys.stderr)
        return 2
    art, rest = argv[0], argv[1:]
    streng = "--streng" in rest
    namen = [n for n in rest if n != "--streng"]
    if art == "such":
        prog = os.environ.get("RANGKERN_BIN", "")
        alle = sorted(p.stem for p in (HIER / "faelle").glob("*.json"))
    else:
        prog = os.environ.get("MCP_BIN", "")
        daten = json.loads((HIER / "mcp_faelle.json").read_text(encoding="utf-8"))
        nach_name = {f["name"]: f for f in daten["faelle"]}
        alle = list(nach_name)
    if not prog:
        print(f"pruefe_windows.py: {'RANGKERN_BIN' if art == 'such' else 'MCP_BIN'} fehlt", file=sys.stderr)
        return 2
    unbekannt = [n for n in namen if n not in alle]
    if unbekannt:
        print(f"pruefe_windows.py: unbekannte Fälle {unbekannt}", file=sys.stderr)
        return 2
    gruen = rot = aus = 0
    for name in namen or alle:
        try:
            if art == "such":
                fall = json.loads((HIER / "faelle" / f"{name}.json").read_text(encoding="utf-8"))
                erw = json.loads((HIER / "erwartet" / f"{name}.json").read_text(encoding="utf-8"))
                such_fall(prog, fall, erw)
            else:
                mcp_fall(prog, nach_name[name], daten["werkzeug"])
            gruen += 1
            print(f"grün  {name}")
        except Aus as exc:
            aus += 1
            print(f"AUS   {name}: {exc}")
        except (Rot, pruefe_mcp.Rot, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as exc:
            rot += 1
            print(f"ROT   {name}: {type(exc).__name__}: {exc}")
    print(f"{gruen} grün, {rot} rot, {aus} ausgelassen von {gruen + rot + aus} ({'Windows' if WINDOWS else 'Attrappe'})")
    return 1 if rot or (streng and aus) else 0


if __name__ == "__main__":
    sys.exit(main())
