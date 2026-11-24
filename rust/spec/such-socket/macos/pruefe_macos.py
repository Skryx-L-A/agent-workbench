#!/usr/bin/env python3
"""Läufer für den Nachtrag „Such-Socket unter macOS“ zu Vertrag 4. Nur Standardbibliothek.

Aufruf: RANGKERN_BIN=<programm> MCP_BIN=<programm> python3 pruefe_macos.py [--vertrag4] [fall ...]

Er fährt die Fälle mit `orte` `mac` (README Abschnitt 7) aus `faelle/`, `erwartet/`, `mcp_faelle.json` und
`signatur_faelle.json`, dazu die Fälle mit Ort `mac-profil`, wenn `/usr/bin/sandbox-exec` und der Python der Command Line Tools
da sind (Profil nach dem Vertrag macOS-Einschluss, Abschnitt „Profil“). Alle anderen meldet er als übersprungen mit Grund,
nie als grün. Mit `--vertrag4` fährt er zusätzlich die Läufer von Vertrag 4
(`../pruefe.py`, `../mcp/pruefe_mcp.py`) gegen `../sichtbar` und `../mcp/faelle.json`, ohne den Fall `v-socket-rechte-0666`.
Jeder Fall hat einen eigenen Ordner mit 0700 unter SOCKET_BASIS (Vorgabe /tmp). Der Socket liegt in dessen Ordner `s`
(0700); sein Pfad hat höchstens 103 Byte, außer in den Fällen, die genau das prüfen.
Umgebung des Prüflings: nur PATH, LANG=C.UTF-8, TZ=UTC, HOME=<root>/home.
Exit 0: kein Fall rot. Exit 1: mindestens ein Fall rot. Exit 2: Aufruffehler (beide Programme fehlen).
"""
from __future__ import annotations

import importlib.util
import json
import os
import plistlib
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent
V4 = HIER.parent
FRIST = 10.0
SUN_PATH = 103     # sun_path fasst unter macOS 104 Byte, das NUL zählt mit
SOCKET_NAME = "such.sock"
SANDBOX_EXEC = "/usr/bin/sandbox-exec"
# Die Sonde läuft mit dem Python der Command Line Tools: Der Ordner ist ein Systempfad des Profils (Vertrag des Einschlusses).
SONDE_PYTHON = "/Library/Developer/CommandLineTools/usr/bin/python3"
GRUND = {"mac-profil": "braucht /usr/bin/sandbox-exec und den Python der Command Line Tools",
         "mac-zweites-konto": "braucht ein zweites lokales Konto (offener Punkt des Vertrags)",
         "bau": "ist ein Test im Crate (Naht für die euid)"}


def laden(name: str, pfad: Path):
    spec = importlib.util.spec_from_file_location(name, pfad)
    modul = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modul)
    return modul


V4P = laden("pruefe_v4", V4 / "pruefe.py")
V4M = laden("pruefe_v4_mcp", V4 / "mcp" / "pruefe_mcp.py")
Rot = V4P.Rot


class Aus(Exception):
    """Der Fall läuft hier nicht; der Text nennt den Grund."""


def profil_moeglich() -> bool:
    return os.access(SANDBOX_EXEC, os.X_OK) and os.access(SONDE_PYTHON, os.X_OK)


def aus_grund(orte: list[str]) -> str | None:
    if "mac" in orte or ("mac-profil" in orte and profil_moeglich()):
        return None
    return "; ".join(f"Ort {o}: {GRUND[o]}" for o in orte)


def umgebung(root: Path) -> dict:
    return {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8", "TZ": "UTC", "HOME": str(root / "home")}


def neuer_ordner(praefix: str) -> Path:
    # mkdtemp legt den Ordner mit 0700 an
    return Path(tempfile.mkdtemp(prefix=praefix, dir=os.environ.get("SOCKET_BASIS", "/tmp")))


def lang_name(root: Path, bytes_: int) -> str:
    """Name des Socketordners, sodass `<root>/<name>/such.sock` genau `bytes_` Byte hat."""
    n = bytes_ - len(os.fsencode(str(root))) - 2 - len(SOCKET_NAME)
    if n < 1:
        raise Rot(f"SOCKET_BASIS zu lang für einen Pfad mit {bytes_} Byte")
    return "s" + "x" * (n - 1)


def zustand(pfad: Path):
    """Was am Pfad liegt, ohne Verweisen zu folgen."""
    if os.path.islink(pfad):
        return ("verweis", os.readlink(pfad))
    if not os.path.lexists(pfad):
        return None
    modus = os.lstat(pfad).st_mode
    if stat.S_ISREG(modus):
        return ("datei", pfad.read_bytes())
    return ("art", stat.S_IFMT(modus))


class Aufbau:
    """Ordner, Socketpfad und Vorbedingungen eines Falls."""

    def __init__(self, root: Path, fall: dict):
        auf = {k: v for a in fall.get("aufbau", []) for k, v in a.items()}
        self.relativ = bool(auf.get("socket_relativ"))
        name = lang_name(root, auf["socket_bytes"]) if "socket_bytes" in auf else "s"
        if auf.get("socket_ordner_verweis"):
            echt = root / "echt"
            echt.mkdir(mode=0o700)
            os.symlink(echt, root / name)
            self.echt = echt
        else:
            (root / name).mkdir(mode=0o700)
        self.ordner = root / name
        self.sock = self.ordner / SOCKET_NAME
        self.ziel = root / "ziel" / "leer.sock"
        if auf.get("socket_verweis_ins_leere"):
            self.ziel.parent.mkdir(mode=0o700)
            os.symlink(self.ziel, self.sock)
        self.modus = auf.get("socket_ordner_modus")
        self.root = root
        if not auf.get("socket_bytes") and len(os.fsencode(str(self.sock))) > SUN_PATH:
            raise Rot(f"Socketpfad {len(os.fsencode(str(self.sock)))} Byte; SOCKET_BASIS ist zu lang")

    def arg_sock(self) -> str:
        return "such.sock" if self.relativ else str(self.sock)

    def nach_dateien(self) -> None:
        if self.modus:
            os.chmod(self.ordner, int(self.modus, 8))


def ersetzen(wert: str, auf: Aufbau) -> str:
    return wert.replace("{root}", str(auf.root)).replace("{sock}", auf.arg_sock())


def nachher_pruefen(erw: dict, auf: Aufbau) -> None:
    nach = erw.get("nachher", {})
    if "ordner_modus" in nach:
        modus = f"{stat.S_IMODE(os.stat(auf.ordner).st_mode):04o}"
        if modus != nach["ordner_modus"]:
            raise Rot(f"Start: Ordner hat jetzt {modus}, erwartet unverändert {nach['ordner_modus']}")
    if nach.get("verweis_bleibt") and not (os.path.islink(auf.ordner) or os.path.islink(auf.sock)):
        raise Rot("Start: der Verweis ist nicht mehr da")
    if nach.get("ziel_ohne_socket") and os.path.lexists(auf.ziel):
        raise Rot("Start: am Ziel des Verweises liegt etwas, obwohl dort kein Socket entstehen darf")
    if nach.get("ziel_leer") and os.path.lexists(auf.echt / SOCKET_NAME):
        raise Rot("Start: im Ordner hinter dem Verweis liegt ein Socket")


def fall_laufen(prog: str, fall: dict, erw: dict) -> None:
    root = neuer_ordner("ss-")
    rechte: list[Path] = []
    proc = None
    agent = None
    try:
        auf = Aufbau(root, fall)
        for d in fall["dirs"]:
            (root / d).mkdir(parents=True, exist_ok=True)
        for rel, text in fall["files"].items():
            p = root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text, encoding="utf-8")
        auf.nach_dateien()
        vorher = zustand(auf.sock)
        argv = [prog] + [ersetzen(a, auf) for a in fall["args"]]
        proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                env=umgebung(root), cwd=root)
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
            if zustand(auf.sock) != vorher:
                raise Rot(f"Start: am Socketpfad lag {vorher}, jetzt {zustand(auf.sock)}")
            if auf.relativ and os.path.lexists(root / "such.sock"):
                raise Rot("Start: der relative Socketpfad hat eine Datei im Arbeitsordner angelegt")
            nachher_pruefen(erw, auf)
            return
        zeile = V4P.zeile_lesen(proc.stdout, FRIST)
        if zeile is None or not V4P.gleich(json.loads(zeile or b"null"), {"bereit": True}):
            raise Rot(f"Start: Bereitschaftszeile {{\"bereit\": true}} erwartet, bekommen {zeile!r}")
        modus = os.lstat(auf.sock).st_mode
        if not stat.S_ISSOCK(modus):
            raise Rot("Start: am Socketpfad liegt kein Socket")
        if f"{stat.S_IMODE(modus):04o}" != erw["socket_rechte"]:
            raise Rot(f"Start: Socket mit Rechten {stat.S_IMODE(modus):04o}, erwartet {erw['socket_rechte']}")
        verbindung = None
        lesen = None
        antworten: list[bytes] = []
        for st in fall["steps"]:
            if "profil" in st:
                if agent is None:
                    agent = Agent(root, auf.sock)
                antworten.append(agent.schritt(st, proc.pid))
            elif "verbindung" in st:
                if verbindung:
                    verbindung.close()
                verbindung = None
            elif "write" in st:
                for rel, text in st["write"].items():
                    (root / rel).write_text(text, encoding="utf-8")
            elif "chmod" in st:
                if os.geteuid() == 0:
                    raise Rot("Fall mit chmod läuft nicht als root")
                for rel, m in st["chmod"].items():
                    os.chmod(root / rel, int(m, 8))
                    rechte.append(root / rel)
            else:
                if verbindung is None:
                    verbindung = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    verbindung.settimeout(FRIST)
                    verbindung.connect(str(auf.sock))
                    lesen = verbindung.makefile("rb")
                daten = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":")) if "request" in st \
                    else st["line"]
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
            V4P.antwort_pruefen(nr, e, roh)
        proc.send_signal(signal.SIGTERM)
        try:
            code = proc.wait(FRIST)
        except subprocess.TimeoutExpired:
            raise Rot(f"Ende: nach SIGTERM läuft der Prozess nach {FRIST} s noch")
        if code != erw["ende"]["exit"]:
            raise Rot(f"Ende: Exit {code}, erwartet {erw['ende']['exit']}")
        if erw["ende"].get("socket_weg") and os.path.lexists(auf.sock):
            raise Rot("Ende: Socket liegt nach dem Ende noch da")
        if erw["ende"].get("ordner_bleibt") and not auf.ordner.is_dir():
            raise Rot("Ende: der Ordner des Sockets ist weg; ihn räumt der Starter auf")
        V4P.protokoll_pruefen(root, erw["protokoll"])
        for name in erw.get("ohne_aktionsname", []):
            for datei in (root / "protokoll").glob("brain-*.jsonl"):
                if f'"aktionsname":"{name}"' in datei.read_text(encoding="utf-8"):
                    raise Rot(f"Protokoll: Zeile {name}, die es hier nicht geben darf")
        stderr = proc.stderr.read().decode("utf-8", "replace").splitlines()
        if "stderr_ohne_praefix" in erw and any(z.startswith(erw["stderr_ohne_praefix"]) for z in stderr):
            raise Rot(f"stderr: Zeile mit {erw['stderr_ohne_praefix']!r}, die es hier nicht geben darf")
        if "stderr_zeile" in erw and stderr.count(erw["stderr_zeile"]) != 1:
            raise Rot(f"stderr: die Zeile {erw['stderr_zeile']!r} steht {stderr.count(erw['stderr_zeile'])}-mal da, erwartet 1-mal")
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
            if not V4P.gleich(got, mz["inhalt"]):
                raise Rot(f"Zählung: {got}, erwartet {mz['inhalt']}")
    finally:
        if agent is not None:
            agent.schliessen()
        if proc and proc.poll() is None:
            proc.kill()
            proc.wait()
        for p in rechte:
            if p.exists():
                os.chmod(p, 0o755)
        for p in root.rglob("*"):
            try:
                if p.is_dir() and not p.is_symlink():
                    os.chmod(p, 0o700)
            except OSError:
                pass
        shutil.rmtree(root, ignore_errors=True)


# ---------------------------------------------------------------- Profil des Einschlusses

SYSTEMPFADE = ["/usr", "/bin", "/sbin", "/System", "/Library/Apple", "/Library/Developer/CommandLineTools", "/private/etc",
               "/private/var/db/timezone", "/private/var/db/dyld", "/private/var/select"]

SONDE = r"""
import errno, json, os, socket, sys
def aus(d):
    sys.stdout.write(json.dumps(d, separators=(",", ":")) + "\n")
def fehler(e):
    return {"errno": errno.errorcode.get(e.errno, str(e.errno))}
art = sys.argv[1]
if art == "lesen":
    try:
        with open(sys.argv[2], "rb") as f:
            f.read(1)
        aus({"lesen": "gelingt"})
    except OSError as e:
        aus(fehler(e))
elif art in ("frage", "verbinde"):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(10)
    try:
        s.connect(sys.argv[2])
    except OSError as e:
        aus(fehler(e))
        sys.exit(0)
    if art == "verbinde":
        aus({"verbunden": True})
        sys.exit(0)
    s.sendall(sys.argv[3].encode("utf-8") + b"\n")
    sys.stdout.buffer.write(s.makefile("rb").readline())
elif art == "signal":
    try:
        os.kill(int(sys.argv[2]), int(sys.argv[3]))
        aus({"kill": "gelingt"})
    except OSError as e:
        aus({"kill": "scheitert"} if e.errno == errno.EPERM else {"kill": "scheitert"} | fehler(e))
"""


def profil_text(arbeit: Path, heim: Path, programme: list[Path], sock: Path, merkmal: Path, geschuetzt: list[Path],
                port: int) -> str:
    """Das Profil nach dem Vertrag macOS-Einschluss, Abschnitt „Profil“, Zeile für Zeile. Programmordner ist der Ordner des
    Pythons der Sonde: Er liegt in einem Systempfad, aber Python löst beim Start seinen Pfad auf und braucht dafür die
    Metadaten der Vorfahren (gemessen 22.11.: ohne sie `realpath: … Operation not permitted`)."""
    def q(p) -> str:
        return '"' + str(p) + '"'
    vorfahren = sorted({str(a) for p in (arbeit, heim, *programme) for a in p.parents})
    anmeldung = [Path.home() / ".ssh", Path.home() / "Library" / "Keychains"]
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
        "(allow file-read* file-map-executable " + " ".join(f"(subpath {q(s)})" for s in [*SYSTEMPFADE, *programme]) + ")",
        "(allow file-read-metadata " + " ".join(f"(literal {q(v)})" for v in vorfahren) + ")",
        f"(allow file-read* file-write* (subpath {q(arbeit)}) (subpath {q(heim)}))",
        f'(allow network-outbound (remote ip "localhost:{port}"))',
        f"(allow network-outbound (remote unix-socket (path-literal {q(sock)})))",
        "(deny file-read* file-write* " + " ".join(f"(subpath {q(g)})" for g in geschuetzt + anmeldung) + ")",
        '(deny sysctl-read (sysctl-name "kern.procargs2") (sysctl-name "kern.procargs") (sysctl-name-prefix "kern.proc."))',
        "(deny process-info* (target others))",
        f"(allow file-write-data (literal {q(merkmal)}))",
    ]
    return "\n".join(zeilen) + "\n"


class Agent:
    """Ein Agent im Profil des Einschlusses: Arbeitsordner und Sitzungsheim unter `<root>/agent`, die Sonde im Arbeitsordner.
    Vorher die Positivkontrolle des Profils: Eine Notiz des Vault ist für die Sonde gesperrt (sonst wirkt das Profil nicht)."""

    def __init__(self, root: Path, sock: Path):
        self.root = Path(os.path.realpath(root))
        self.sock = Path(os.path.realpath(sock))
        if len(os.fsencode(str(self.sock))) > SUN_PATH:
            raise Rot(f"Profil: Socketpfad {self.sock} hat mehr als {SUN_PATH} Byte")
        self.arbeit = self.root / "agent" / "arbeit"
        self.heim = self.root / "agent" / "heim"
        for d in (self.arbeit, self.heim, self.heim / "tmp"):
            d.mkdir(parents=True, mode=0o700)
        (self.arbeit / "sonde.py").write_text(SONDE, encoding="utf-8")
        self.merkmal = self.root / "merkmal"
        self.merkmal.write_text("", encoding="utf-8")
        frei = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        frei.bind(("127.0.0.1", 0))
        port = frei.getsockname()[1]
        frei.close()
        geschuetzt = [self.root / n for n in ("vault", "aux", "protokoll", self.sock.parent.name)]
        self.profil = self.root / "profil.sb"
        programme = [Path(SONDE_PYTHON).parent]
        self.profil.write_text(profil_text(self.arbeit, self.heim, programme, self.sock, self.merkmal, geschuetzt, port),
                               encoding="utf-8")
        self.fremd = None
        probe = next((self.root / "vault").rglob("*.md"), None)
        if probe is None:
            raise Rot("Profil: der Fall hat keine Notiz für die Positivkontrolle")
        aussen = json.loads(self.sonde("lesen", str(probe), im_profil=False))
        if aussen != {"lesen": "gelingt"}:
            raise Rot(f"Profil: Positivkontrolle ohne Profil liest die Notiz nicht ({aussen})")
        innen = json.loads(self.sonde("lesen", str(probe)))
        if innen != {"errno": "EPERM"}:
            raise Rot(f"Profil wirkt nicht: die Sonde im Profil liest eine Notiz des Vault ({innen})")

    def sonde(self, *args: str, im_profil: bool = True) -> bytes:
        argv = [SONDE_PYTHON, "-I", "-B", str(self.arbeit / "sonde.py"), *args]
        if im_profil:
            argv = [SANDBOX_EXEC, "-f", str(self.profil), *argv]
        env = {"PATH": "/usr/bin:/bin", "HOME": str(self.heim), "TMPDIR": str(self.heim / "tmp"), "LANG": "C.UTF-8"}
        r = subprocess.run(argv, capture_output=True, env=env, cwd=self.arbeit, timeout=FRIST)
        if r.returncode != 0 or r.stdout.count(b"\n") != 1:
            raise Rot(f"Sonde {args[0]}: Exit {r.returncode}, stdout {r.stdout[:120]!r}, stderr {r.stderr[:200]!r}")
        return r.stdout

    def schritt(self, st: dict, pid: int) -> bytes:
        art = st["profil"]
        if art == "eigener-socket":
            zeile = json.dumps(st["request"], ensure_ascii=False, separators=(",", ":"))
            return self.sonde("frage", str(self.sock), zeile)
        if art == "fremde-sitzung":
            fremd = self.sock.parent / "fremd.sock"
            self.fremd = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.fremd.bind(str(fremd))
            self.fremd.listen(4)
            # Positivkontrolle: Ohne Profil verbindet derselbe Weg zum Socket der anderen Sitzung.
            aussen = json.loads(self.sonde("verbinde", str(fremd), im_profil=False))
            if aussen != {"verbunden": True}:
                raise Rot(f"fremde Sitzung: Positivkontrolle ohne Profil verbindet nicht ({aussen})")
            return self.sonde("verbinde", str(fremd))
        if art == "prozess":
            # Positivkontrolle: Ohne Profil erreicht Signal 0 den Such-Prozess.
            aussen = json.loads(self.sonde("signal", str(pid), "0", im_profil=False))
            if aussen != {"kill": "gelingt"}:
                raise Rot(f"Prozess: Positivkontrolle ohne Profil, Signal 0 scheitert ({aussen})")
            return self.sonde("signal", str(pid), str(int(signal.SIGTERM)))
        raise Rot(f"unbekannter Profil-Schritt {art}")

    def schliessen(self) -> None:
        if self.fremd is not None:
            self.fremd.close()
            self.fremd = None


# ---------------------------------------------------------------- Signatur (decision of 22.11.2026)

GET_TASK_ALLOW = "com.apple.security.get-task-allow"


def rechte_von(prog: Path) -> dict:
    r = subprocess.run(["/usr/bin/codesign", "-d", "--entitlements", "-", "--xml", str(prog)], capture_output=True,
                       timeout=FRIST)
    if r.returncode != 0:
        raise Rot(f"codesign -d {prog.name}: Exit {r.returncode}: {r.stderr.decode('utf-8', 'replace').strip()[:200]}")
    return plistlib.loads(r.stdout) if r.stdout.strip() else {}


def verbotene_pruefen(prog: Path, verboten: list[str]) -> None:
    rechte = rechte_von(prog)
    for recht in verboten:
        if recht in rechte and rechte[recht] is not False:
            raise Rot(f"{prog.name} trägt das Recht {recht} = {rechte[recht]!r}")


def signatur_laufen(prog: str, fall: dict) -> None:
    verbotene_pruefen(Path(prog), fall["verbotene_rechte"])
    # Gegenprobe: eine Kopie mit get-task-allow, ad-hoc signiert, muss dieselbe Prüfung rot machen; sonst ist sie blind.
    ordner = neuer_ordner("sg-")
    try:
        kopie = ordner / Path(prog).name
        shutil.copy2(prog, kopie)
        plist = ordner / "rechte.plist"
        plist.write_bytes(plistlib.dumps({GET_TASK_ALLOW: True}))
        r = subprocess.run(["/usr/bin/codesign", "-f", "-s", "-", "--entitlements", str(plist), str(kopie)],
                           capture_output=True, timeout=FRIST)
        if r.returncode != 0:
            raise Rot(f"Gegenprobe: codesign der Kopie scheitert ({r.stderr.decode('utf-8', 'replace').strip()[:200]})")
        try:
            verbotene_pruefen(kopie, fall["verbotene_rechte"])
        except Rot:
            return
        raise Rot("Gegenprobe: die Kopie mit get-task-allow geht durch die Prüfung; sie ist blind")
    finally:
        shutil.rmtree(ordner, ignore_errors=True)


# ---------------------------------------------------------------- such-mcp

def mcp_laufen(prog: str, fall: dict, werkzeug: dict) -> None:
    root = neuer_ordner("mm-")
    proc = None
    try:
        (root / "home").mkdir()
        argv = list(fall["argv"])
        sock = None
        ersatz = {}
        for n in (103, 104):
            if any("{sock%d}" % n in a for a in argv):
                name = lang_name(root, n)
                (root / name).mkdir(mode=0o700)
                ersatz["{sock%d}" % n] = str(root / name / SOCKET_NAME)
                if n == 103:
                    sock = root / name / SOCKET_NAME
        if sock is None:
            (root / "s").mkdir(mode=0o700)
            sock = root / "s" / SOCKET_NAME
        ersatz["{sock}"] = str(sock)
        for k, v in ersatz.items():
            argv = [a.replace(k, v) for a in argv]
        if "start_exit" in fall:
            lauf = subprocess.run([prog] + argv, stdin=subprocess.DEVNULL, capture_output=True, env=umgebung(root),
                                  cwd=root, timeout=FRIST)
            if lauf.returncode != fall["start_exit"] or lauf.stdout:
                raise Rot(f"Start: Exit {lauf.returncode} und stdout {lauf.stdout[:80]!r}, erwartet Exit {fall['start_exit']}")
            return
        stand_in = None
        if not fall.get("kein_socket"):
            stand_in = V4M.StandIn(sock, fall.get("socket_antworten", []))
            stand_in.start()
        proc = subprocess.Popen([prog] + argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                env=umgebung(root), cwd=root)
        for nr, st in enumerate(fall["schritte"], 1):
            daten = st["roh"] if "roh" in st else json.dumps(st["sende"], ensure_ascii=False, separators=(",", ":"))
            proc.stdin.write(daten.encode("utf-8") + b"\n")
            proc.stdin.flush()
            if "erwarte" in st:
                roh = V4M.zeile_lesen(proc.stdout, FRIST)
                if roh is None:
                    raise Rot(f"Schritt {nr}: keine Antwort binnen {FRIST} s")
                try:
                    got = json.loads(roh.decode("utf-8"))
                except ValueError:
                    raise Rot(f"Schritt {nr}: Antwort ist kein JSON: {roh[:120]!r}")
                V4M.pruefen(nr, st["erwarte"], got, werkzeug)
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
            if not V4M.gleich(got, fall["socket_zeilen"]):
                raise Rot(f"Socket: empfangen {got}, erwartet {fall['socket_zeilen']}")
    finally:
        if proc and proc.poll() is None:
            proc.kill()
            proc.wait()
        shutil.rmtree(root, ignore_errors=True)


# ---------------------------------------------------------------- Läufe

class Zaehler:
    def __init__(self):
        self.gruen = self.rot = self.aus = 0

    def melden(self, art: str, name: str, text: str = "") -> None:
        if art == "grün":
            self.gruen += 1
            print(f"grün  {name}")
        elif art == "ROT":
            self.rot += 1
            print(f"ROT   {name}: {text}")
        else:
            self.aus += 1
            print(f"aus   {name}: {text}")


def lauf(z: Zaehler, name: str, orte: list[str], prog: str, var: str, fn) -> None:
    grund = aus_grund(orte)
    if grund:
        z.melden("aus", name, grund)
    elif not prog:
        z.melden("aus", name, f"{var} fehlt")
    else:
        try:
            fn()
            z.melden("grün", name)
        except (Rot, OSError, ValueError, subprocess.TimeoutExpired) as exc:
            z.melden("ROT", name, str(exc))


def vertrag4(z: Zaehler, rang: str, mcp: str) -> None:
    satz = V4 / "sichtbar"
    for p in sorted((satz / "faelle").glob("*.json")):
        name = "vertrag4:" + p.stem
        if p.stem == "v-socket-rechte-0666":
            z.melden("aus", name, "unter macOS ersetzt durch mac-a-rechte-0666")
            continue
        fall = json.loads(p.read_text(encoding="utf-8"))
        erw = json.loads((satz / "erwartet" / p.name).read_text(encoding="utf-8"))
        lauf(z, name, ["mac"], rang, "RANGKERN_BIN", lambda: V4P.fall_laufen(rang, fall, erw))
    daten = json.loads((V4 / "mcp" / "faelle.json").read_text(encoding="utf-8"))
    for f in daten["faelle"]:
        lauf(z, "vertrag4-mcp:" + f["name"], ["mac"], mcp, "MCP_BIN", lambda: V4M.fall_laufen(mcp, f, daten["werkzeug"]))


def main() -> int:
    args = sys.argv[1:]
    mit_v4 = "--vertrag4" in args
    namen = [a for a in args if a != "--vertrag4"]
    if any(a.startswith("-") for a in namen):
        print("Aufruf: pruefe_macos.py [--vertrag4] [fall ...]", file=sys.stderr)
        return 2
    rang, mcp = os.environ.get("RANGKERN_BIN", ""), os.environ.get("MCP_BIN", "")
    if not rang and not mcp:
        print("pruefe_macos.py: RANGKERN_BIN und MCP_BIN fehlen", file=sys.stderr)
        return 2
    z = Zaehler()
    such = [p.stem for p in sorted((HIER / "faelle").glob("*.json"))]
    mcp_daten = json.loads((HIER / "mcp_faelle.json").read_text(encoding="utf-8"))
    sig_namen = {f["name"] for f in json.loads((HIER / "signatur_faelle.json").read_text(encoding="utf-8"))["faelle"]}
    for n in namen:
        if n not in such and n not in {f["name"] for f in mcp_daten["faelle"]} and n not in sig_namen:
            print(f"pruefe_macos.py: unbekannter Fall {n}", file=sys.stderr)
            return 2
    for name in such:
        if namen and name not in namen:
            continue
        fall = json.loads((HIER / "faelle" / f"{name}.json").read_text(encoding="utf-8"))
        erw = json.loads((HIER / "erwartet" / f"{name}.json").read_text(encoding="utf-8"))
        lauf(z, name, fall["orte"], rang, "RANGKERN_BIN", lambda: fall_laufen(rang, fall, erw))
    for f in mcp_daten["faelle"]:
        if namen and f["name"] not in namen:
            continue
        lauf(z, f["name"], f["orte"], mcp, "MCP_BIN", lambda: mcp_laufen(mcp, f, mcp_daten["werkzeug"]))
    sig_daten = json.loads((HIER / "signatur_faelle.json").read_text(encoding="utf-8"))
    for f in sig_daten["faelle"]:
        if namen and f["name"] not in namen:
            continue
        lauf(z, f["name"], f["orte"], rang, "RANGKERN_BIN", lambda: signatur_laufen(rang, f))
    if mit_v4 and not namen:
        vertrag4(z, rang, mcp)
    print(f"{z.gruen} grün, {z.rot} rot, {z.aus} übersprungen von {z.gruen + z.rot + z.aus}")
    return 1 if z.rot else 0


if __name__ == "__main__":
    sys.exit(main())
