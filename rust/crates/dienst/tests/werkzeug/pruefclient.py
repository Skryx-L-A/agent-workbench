"""Prüfclient of the `dienst` contract (rust/spec/VERTRAEGE.md, section 8). Standard library only.

  DI_BIN=<dienst program> python3 -I -B pruefclient.py <fall.json>

Reads one case, writes the Startdatei, starts the service, talks to it over the Unix socket as one or more viewers and
writes ONE JSON object on stdout (exit 0): {"start": {...}, "ergebnisse": [...], "ende": {...}}.
Exit 2: the case is invalid. Exit 3: the environment is wrong (no DI_BIN, no lsof, no candidate folder).

Environment: DI_BIN (required, the program under test), DI_TMP (base folder for the throw-away folders, default /tmp;
the socket path must stay under 100 bytes), DI_REF_DIR (folder with ra_ref.py, default referenz/ next to this file).
The decoder of the viewer is the Python reference of the `rasterauszug` contract (ra_ref.py), never a mutant.
"""
from __future__ import annotations

import json
import os
import re
import selectors
import stat
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

HIER = os.path.dirname(os.path.abspath(__file__))
MUT = os.environ.pop("REF_GEGENPROBE", "")
sys.path.insert(0, os.environ.get("DI_REF_DIR") or os.path.join(HIER, "referenz"))
import ra_ref  # noqa: E402

FRIST = 20.0
LINUX = sys.platform.startswith("linux")
# addendum of 21.11.2026: the witness of a case (`vorzeuge`), started before the service, in a session of its own. Ignores SIGHUP,
# writes `TERM` into <ordner>/vorzeuge.term on SIGTERM, writes its pid into <base>/vorzeuge.pid once the handler stands, ends by itself after 8 s.
VORZEUGE = (
    "import os, signal, sys, time\n"
    "m, b = sys.argv[1], sys.argv[2]\n"
    "def t(*a):\n"
    "    open(m, 'w').write('TERM\\n')\n"
    "    os._exit(0)\n"
    "signal.signal(signal.SIGTERM, t)\n"
    "signal.signal(signal.SIGHUP, signal.SIG_IGN)\n"
    "open(b + '.tmp', 'w').write('%d\\n' % os.getpid())\n"
    "os.rename(b + '.tmp', b)\n"
    "time.sleep(8)\n"
)


def ohne_marker(namen):
    """Where the signal is not part of the case (not Linux, no /proc) the markers `*.term` are no part of the result: the case checks the files only."""
    return namen if LINUX else [n for n in namen if not n.endswith(".term")]
ARTEN = {"Ereignis": 0x01, "Uhr": 0x10, "Stumm": 0x11, "Zustand": 0x12, "Kernfehler": 0x13}


class Ungueltig(Exception):
    pass


class Umgebung(Exception):
    pass


# --- Kernabbild (Schritt 2a): a program that shields its memory from core images (`PR_SET_DUMPABLE 0`) is invisible to `lsof` of the same user
# (measured on Linux: 0 lines, `/proc/<pid>/fd` is "Permission denied"). So the program runs in its own user namespace without capabilities
# (`unshare -Ur`, then `setpriv` drops them): the observer outside, same user, owns that namespace and sees it (measured: 1 line). The
# program starts as the same process (no fork: `unshare` and `setpriv` exec), so `proc.pid` stays its pid. Other systems have no
# `PR_SET_DUMPABLE`: no prefix. `DI_NS=aus` (a counter-check of the tests, never a setting of the product) starts it without the
# namespace: the program is then blind for `lsof` and the positive control below must end the run.
NS_PRAEFIX = ["unshare", "-Ur", "--", "setpriv", "--bounding-set=-all", "--inh-caps=-all", "--"]
_NS_GEPRUEFT = []


def beobacht_praefix():
    """The command prefix under which the program under test starts ([] where nothing is needed)."""
    if sys.platform != "linux" or os.environ.get("DI_NS") == "aus":
        return []
    if not _NS_GEPRUEFT:
        try:
            r = subprocess.run(NS_PRAEFIX + ["true"], capture_output=True, timeout=20)
        except (OSError, subprocess.SubprocessError) as e:
            raise Umgebung("Nutzer-Namensraum nicht benutzbar (unshare oder setpriv fehlt): %s" % e)
        if r.returncode != 0:
            raise Umgebung("Nutzer-Namensraum nicht erlaubt (unshare -Ur): %s" % r.stderr.decode("utf-8", "replace").strip()[:200])
        _NS_GEPRUEFT.append(True)
    return NS_PRAEFIX


def lsof_zeilen(pid, *args):
    """The lines of `lsof -nP -a -p <pid> <args>` without the head line."""
    try:
        r = subprocess.run(["lsof", "-nP", "-a", "-p", str(pid), *args], capture_output=True, timeout=20)
    except FileNotFoundError:
        raise Umgebung("lsof fehlt")
    return [z for z in r.stdout.decode("utf-8", "replace").split("\n")[1:] if z.strip()]


def rahmen(art, nutz):
    return struct.pack(">IB", 1 + len(nutz), art) + nutz


def jb(o):
    return json.dumps(o, ensure_ascii=False, sort_keys=True).encode("utf-8")


class Betrachter:
    def __init__(self, sock, deflate):
        self.sock = sock
        self.buf = bytearray()
        self.ver = ra_ref.Verbindung(deflate)
        self.getrennt = False
        self.gesehen = 0
        self.roh = bytearray()
        self.nachrichten, self.ablehnungen, self.fehler, self.abbild_fehler = [], [], [], []
        self.abl_voll = []
        self.antwort = None
        self.quittung = 0
        self.liste = None
        self.pid = None

    def abbild(self):
        for nr, ab in sorted(self.ver.empfang.items()):
            return {"nr": nr, "groesse": [ab.spalten, ab.zeilen], "invers": ab.invers, "cursor": list(ab.cursor),
                    "zeilen": [re.sub(r"PID=[0-9]+", "PID=#", ra_ref.zeile_schreiben(z)) for z in ab.lines]}
        return None

    def abbilder(self):
        """Nachtrag 20.11. (DI12): the pictures of all sessions, by number of the frame."""
        return {str(nr): {"groesse": [ab.spalten, ab.zeilen], "invers": ab.invers, "cursor": list(ab.cursor),
                          "zeilen": [re.sub(r"PID=[0-9]+", "PID=#", ra_ref.zeile_schreiben(z)) for z in ab.lines]}
                for nr, ab in sorted(self.ver.empfang.items())}

    def text(self, nr=None):
        for n, ab in sorted(self.ver.empfang.items()):
            if nr is None or n == nr:
                return ["".join(l[3] for l in z) for z in ab.lines]
        return []

    def alle_texte(self):
        return [["".join(l[3] for l in z) for z in ab.lines] for _, ab in sorted(self.ver.empfang.items())]


class Lauf:
    def __init__(self, fall):
        self.fall = fall
        self.base = tempfile.mkdtemp(prefix="di-", dir=os.environ.get("DI_TMP") or "/tmp")
        self.o = os.path.join(self.base, "o")
        self.proc = None
        self.viewer = {}
        self.erste = None
        self.t_uhr = 0
        self.sel = selectors.DefaultSelector()
        st = fall.get("start") or {}
        self.pruefmodus = bool(st.get("pruefmodus", False))
        self.deflate = st.get("deflate", True) if isinstance(st, dict) else True
        self.pid_kind = None
        self.vorzeuge = None
        self.pids = set()
        self.gemessen = None
        self.laufkind = None

    # ---------- folder and Startdatei
    def ordner(self):
        sp = self.fall.get("ordner", {"art": "verzeichnis", "modus": "0700"})
        art = sp.get("art", "verzeichnis")
        modus = int(sp.get("modus", "0700"), 8)
        pfad = self.o
        if art == "verzeichnis":
            os.mkdir(self.o)
        elif art == "link":
            os.mkdir(os.path.join(self.base, "ziel"), 0o700)
            os.symlink(os.path.join(self.base, "ziel"), self.o)
        elif art == "datei":
            open(self.o, "w").close()
        elif art == "absolut":
            for k in sp["absolut"]:
                try:
                    st = os.lstat(k)
                except OSError:
                    continue
                if stat.S_ISDIR(st.st_mode) and st.st_uid != os.geteuid() and stat.S_IMODE(st.st_mode) == 0o700:
                    pfad = k
                    break
            else:
                raise Umgebung("kein Ordner der Liste gehoert einem anderen Benutzer mit Modus 0700")
        elif art != "fehlt":
            raise Ungueltig("ordner.art")
        if art == "verzeichnis":
            vorher = sp.get("socket_vorher")
            name = ((self.fall.get("start") or {}).get("socket") or {}).get("name", "dienst.sock")
            if vorher == "datei":
                open(os.path.join(self.o, name), "w").close()
            elif vorher == "link":
                os.symlink("/nonexistent-di", os.path.join(self.o, name))
            for u in sp.get("unter", []):          # DI12: folders inside the socket folder before the start (a name Starten wants to use)
                os.mkdir(os.path.join(self.o, u), 0o700)
            os.chmod(self.o, modus)
        self.pfad_ordner = pfad
        self.sitzungen_vorbereiten()
        self.bestand_vorbereiten()
        self.marker_datei_vorbereiten()
        self.ziellisten_vorbereiten()
        self.gespraeche_vorbereiten()
        self.firmenvorgabe_vorbereiten()
        name = ((self.fall.get("start") or {}).get("socket") or {}).get("name", "dienst.sock")
        self.pfad_socket = os.path.join(pfad, name if isinstance(name, str) and "/" not in name and name else "dienst.sock")

    def bestand_vorbereiten(self):
        """Nachtrag 26.11.2026, DI-L5: the folder of lauf.bestand and, with `vorher`, the file of an earlier run."""
        sp = self.fall.get("bestand")
        self.bestand_dir = os.path.join(self.base, "bestand")
        self.bestand_datei = os.path.join(self.bestand_dir, "bestand.json")
        if sp is None:
            return
        os.mkdir(self.bestand_dir, 0o700)
        if "vorher" in sp:
            with open(self.bestand_datei, "w", encoding="utf-8") as h:
                json.dump(sp["vorher"], h, ensure_ascii=False)
            os.chmod(self.bestand_datei, int(sp.get("modus", "0600"), 8))
        os.chmod(self.bestand_dir, int(sp.get("ordner_modus", "0700"), 8))

    def sitzungen_vorbereiten(self):
        """Kennung (addendum of 21.11.2026): the case field `sitzungen` makes `<base>/sitzungen`, the folder of `lauf.sitzungen`, a throw-away folder of the case.
        `vorher`: entries made before the start, each {name, art} with art `ordner`, `datei`, `link_tot` (a link to nothing) or `link_ordner` (a link to a folder);
        `fehlt`: the folder is not made at all; `modus`: the mode of the folder after the entries (default 0700)."""
        sp = self.fall.get("sitzungen")
        self.sitz_dir = os.path.join(self.base, "sitzungen")
        if sp is None or sp.get("fehlt"):
            return
        os.mkdir(self.sitz_dir, 0o700)
        for e in sp.get("vorher", []):
            pf = os.path.join(self.sitz_dir, e["name"])
            art = e["art"]
            if art == "ordner":
                os.mkdir(pf, 0o700)
            elif art == "datei":
                open(pf, "w").close()
            elif art == "link_tot":
                os.symlink("/nonexistent-di-sitzung", pf)
            elif art == "link_ordner":
                ziel = os.path.join(self.base, "ziel-" + e["name"])
                os.mkdir(ziel, 0o700)
                os.symlink(ziel, pf)
            else:
                raise Ungueltig("sitzungen.vorher.art")
        os.chmod(self.sitz_dir, int(sp.get("modus", "0700"), 8))

    def gespraeche_vorbereiten(self):
        """Nachtrag 27.11.2026, DI-L3: the case field `gespraeche` {"vorher": [<Kennung>...]} makes `<base>/gespraeche` (0700), the folder of
        lauf.gespraeche (placeholder `@gespraeche`), with one conversation per name: gespraech.json (0600) and a state folder of the harness with
        nested files, a link to the folder `<base>/aussen` and a link to a file in it. `<base>/aussen` holds two files; nothing may touch them."""
        sp = self.fall.get("gespraeche")
        self.gs_dir = os.path.join(self.base, "gespraeche")
        self.aussen_dir = os.path.join(self.base, "aussen")
        if sp is None:
            return
        os.mkdir(self.aussen_dir, 0o700)
        os.mkdir(os.path.join(self.aussen_dir, "unter"), 0o700)
        for rel in ("a.txt", "unter/b.txt"):
            with open(os.path.join(self.aussen_dir, rel), "w") as h:
                h.write("aussen\n")
        os.mkdir(self.gs_dir, 0o700)
        for name in sp.get("vorher", []):
            self.gespraech_anlegen(name)
        os.chmod(self.gs_dir, int(sp.get("modus", "0700"), 8))       # `modus`: the mode of lauf.gespraeche after the entries

    def gespraech_anlegen(self, name):
        """One conversation `<base>/gespraeche/<name>` as the start of `lauf` makes it (case field `gespraeche`, step GespraechAnlegen)."""
        g = os.path.join(self.gs_dir, name)
        os.mkdir(g, 0o700)
        with open(os.path.join(g, "gespraech.json"), "w") as h:
            json.dump({"fassung": 1, "harness": "claude", "arbeitsordner": "/heim/beispiel/projekt",
                       "kennung": "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4b"}, h)
        os.chmod(os.path.join(g, "gespraech.json"), 0o600)
        os.makedirs(os.path.join(g, ".claude", "projects", "p"), 0o700)
        with open(os.path.join(g, ".claude", "projects", "p", "verlauf.jsonl"), "w") as h:
            h.write("{}\n")
        os.symlink(self.aussen_dir, os.path.join(g, ".claude", "nach-aussen"))
        os.symlink(os.path.join(self.aussen_dir, "a.txt"), os.path.join(g, ".claude", "datei-aussen"))

    def firmenvorgabe_vorbereiten(self):
        """Nachtrag 27.11.2026, Firmenvorgabe at the start: the case field `firmenvorgabe` {"inhalt": <JSON> | "text": <raw text>, "modus": "0644",
        "ordner_modus": "0755", "art": "datei" | "verweis"} makes `<base>/vorgabe/firmenvorgabe.json` (placeholder `@firmenvorgabe`); `verweis` makes the
        name a link to a valid file next to the folder. The step VorgabeAendern replaces the file while the service runs."""
        sp = self.fall.get("firmenvorgabe")
        self.fv_dir = os.path.join(self.base, "vorgabe")
        self.fv_datei = os.path.join(self.fv_dir, "firmenvorgabe.json")
        if sp is None:
            return
        os.mkdir(self.fv_dir, 0o700)
        roh = sp["text"] if "text" in sp else json.dumps(sp.get("inhalt"), sort_keys=True, indent=1) + "\n"
        if sp.get("art") == "verweis":
            ziel = os.path.join(self.base, "vorgabe-ziel.json")
            with open(ziel, "w", encoding="utf-8") as h:
                h.write(roh)
            os.chmod(ziel, 0o644)
            os.symlink(ziel, self.fv_datei)
        else:
            with open(self.fv_datei, "w", encoding="utf-8") as h:
                h.write(roh)
            os.chmod(self.fv_datei, int(sp.get("modus", "0644"), 8))
        os.chmod(self.fv_dir, int(sp.get("ordner_modus", "0755"), 8))

    def ziellisten_vorbereiten(self):
        """Ziellisten, Laufzeit, Zaehldateien (addendum of 21.11.2026): the case fields `ziellisten`, `laufzeit` and `marker_zaehl` each make a throw-away folder
        of the case (`<base>/ziellisten`, `<base>/laufzeit`, `<base>/marker-zaehl`), the folder of `lauf.ziellisten`, `lauf.laufzeit` and `lauf.marker_ordner`. `vorher`: entries made before
        the start, each {name, art} with art `ordner`, `datei`, `link_tot` or `link_ordner` (the name is the whole file name, for example `s-1.zielliste.json`); `fehlt`: the folder
        is not made at all; `modus`: the mode of the folder after the entries (default 0700). The Startdatei names them with `@ziellisten`, `@laufzeit` and `@marker_zaehl`."""
        self.zl_dir = os.path.join(self.base, "ziellisten")
        self.lz_dir = os.path.join(self.base, "laufzeit")
        self.mz_dir = os.path.join(self.base, "marker-zaehl")
        for feld, verz in (("ziellisten", self.zl_dir), ("laufzeit", self.lz_dir), ("marker_zaehl", self.mz_dir)):
            sp = self.fall.get(feld)
            if sp is None or sp.get("fehlt"):
                continue
            os.mkdir(verz, 0o700)
            for e in sp.get("vorher", []):
                pf = os.path.join(verz, e["name"])
                art = e["art"]
                if art == "ordner":
                    os.mkdir(pf, 0o700)
                elif art == "datei":
                    with open(pf, "w") as h:
                        h.write("{}")
                elif art == "link_tot":
                    os.symlink("/nonexistent-di-eintrag", pf)
                elif art == "link_ordner":
                    ziel = os.path.join(self.base, "eziel-" + feld + "-" + e["name"])
                    os.mkdir(ziel, 0o700)
                    os.symlink(ziel, pf)
                else:
                    raise Ungueltig(feld + ".vorher.art")
            os.chmod(verz, int(sp.get("modus", "0700"), 8))

    def marker_datei_vorbereiten(self):
        """Marker (addendum of 21.11.2026): the case field `marker_datei` makes `<base>/mk/marker.json`, the file `lauf.marker_datei` names.
        `art`: `datei` (default), `fehlt` (folder, no file), `ordner_fehlt` (no folder), `link` (the file is a link to a good file), `ordner_link` (the folder is a link
        to a good folder); `inhalt`: a JSON value written as the content, or `roh`: the exact text; `auffuellen`: bytes of blanks added; `modus`: mode of the file
        (default 0600); `ordner_modus`: mode of the folder (default 0700)."""
        sp = self.fall.get("marker_datei")
        self.mk_ordner = os.path.join(self.base, "mk")
        self.mk_datei = os.path.join(self.mk_ordner, "marker.json")
        if sp is None:
            return
        art = sp.get("art", "datei")
        if art == "ordner_fehlt":
            return
        if "hex" in sp:
            daten = bytes.fromhex(sp["hex"])
        elif "roh" in sp:
            daten = sp["roh"].encode("utf-8")
        else:
            daten = json.dumps(sp.get("inhalt"), ensure_ascii=False).encode("utf-8")
        daten += b" " * int(sp.get("auffuellen", 0))
        echt = self.mk_ordner
        if art == "ordner_link":
            echt = os.path.join(self.base, "mk-echt")
            os.mkdir(echt, 0o700)
            os.symlink(echt, self.mk_ordner)
        else:
            os.mkdir(self.mk_ordner, 0o700)
        if art == "fehlt":
            pass
        elif art == "fifo":
            os.mkfifo(self.mk_datei, 0o600)
        elif art == "link":
            ziel = os.path.join(self.base, "ziel.json")
            with open(ziel, "wb") as h:
                h.write(daten)
            os.chmod(ziel, 0o600)
            os.symlink(ziel, self.mk_datei)
        else:
            f = os.path.join(echt, "marker.json")
            with open(f, "wb") as h:
                h.write(daten)
            os.chmod(f, int(sp.get("modus", "0600"), 8))
        os.chmod(echt, int(sp.get("ordner_modus", "0700"), 8))

    def ersetze(self, x):
        if isinstance(x, str):
            if x == "@dienst":
                return os.path.abspath(os.environ["DI_BIN"])
            if x == "@laufkind":
                return self.laufkind_datei()
            if x == "@einstellungen":
                # addendum of 20.11.2026: a copy of the demonstration settings file of contract 2 (einstellungen/vorfuehrung.json next to this file);
                # a case with "einstellungen_inhalt" gets a file of its own in its throw-away folder (the step Einstellungen changes it)
                if "einstellungen_inhalt" in self.fall:
                    f = os.path.join(self.base, "einst.json")
                    if not os.path.exists(f):
                        with open(f, "w") as h:
                            json.dump(self.fall["einstellungen_inhalt"], h)
                    return f
                return os.environ.get("DI_EINST") or os.path.join(HIER, "einstellungen", "vorfuehrung.json")
            if x == "@bestand":
                return self.bestand_datei
            if x == "@gespraeche":
                return self.gs_dir
            if x == "@firmenvorgabe":
                return self.fv_datei
            return x.replace("@ziellisten", self.zl_dir).replace("@laufzeit", self.lz_dir).replace("@marker_zaehl", self.mz_dir).replace("@marker_datei", self.mk_datei).replace("@sitzungen", self.sitz_dir).replace("@ordner", self.pfad_ordner)
        if isinstance(x, list):
            return [self.ersetze(v) for v in x]
        if isinstance(x, dict):
            return {k: self.ersetze(v) for k, v in x.items()}
        return x

    def laufkind_datei(self):
        """DI12: the program of the sessions of Starten in the cases is a small wrapper that runs `<DI_BIN> --kind lauf <argv...>`; it keeps the
        environment exactly as given and lets the fixed child see argv as the service gave it."""
        if self.laufkind is None:
            self.laufkind = os.path.join(self.base, "laufkind")
            dienst = os.path.abspath(os.environ["DI_BIN"])
            with open(self.laufkind, "w") as h:
                h.write("#!/usr/bin/python3 -I\nimport os, sys\nos.execv(%r, [%r, \"--kind\", \"lauf\"] + sys.argv)\n" % (dienst, dienst))
            os.chmod(self.laufkind, 0o755)
        return self.laufkind

    def startdatei(self):
        f = os.path.join(self.base, "start.json")
        if "start_roh" in self.fall:
            roh = self.ersetze(self.fall["start_roh"])
            if "\x00" in roh:
                raise Ungueltig("start_roh")
            daten = roh.encode("utf-8", "surrogatepass")
        else:
            daten = json.dumps(self.ersetze(self.fall["start"]), ensure_ascii=False).encode("utf-8")
        daten += b" " * int(self.fall.get("start_auffuellen", 0))
        with open(f, "wb") as h:
            h.write(daten)
        return f

    # ---------- start
    def vorzeuge_starten(self):
        """The case wants a process that is older than every child of the service (a session leader of its own); its pid is in <base>/vorzeuge.pid."""
        bereit = os.path.join(self.base, "vorzeuge.pid")
        self.vorzeuge = subprocess.Popen([sys.executable, "-I", "-c", VORZEUGE, os.path.join(self.o, "vorzeuge.term"), bereit],
                                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        t0 = time.monotonic()
        while not os.path.exists(bereit):
            if time.monotonic() - t0 > 5:
                raise Umgebung("vorzeuge")
            time.sleep(0.01)
        time.sleep(0.05)

    def start(self):
        self.ordner()
        if self.fall.get("vorzeuge"):
            self.vorzeuge_starten()
        f = self.startdatei()
        env = dict(os.environ)
        if MUT:
            env["REF_GEGENPROBE"] = MUT
        self.so = open(os.path.join(self.base, "stdout"), "wb")
        self.se = open(os.path.join(self.base, "stderr"), "wb")
        self.proc = subprocess.Popen([*beobacht_praefix(), os.environ["DI_BIN"], f], stdin=subprocess.DEVNULL, stdout=self.so, stderr=self.se,
                                     env=env, cwd=self.base)
        t0 = time.monotonic()
        while time.monotonic() - t0 < 10.0:
            rc = self.proc.poll()
            if rc is not None:
                self.se.flush()
                with open(os.path.join(self.base, "stderr"), "rb") as h:
                    zeilen = [z for z in h.read().decode("utf-8", "replace").split("\n") if z.strip()]
                return {"ok": False, "exitcode": rc, "stderr_zeilen": len(zeilen),
                        "stderr_beginn": bool(zeilen) and zeilen[0].startswith("wb-dienst: "),
                        "socket_da": os.path.lexists(self.pfad_socket)}
            try:
                if stat.S_ISSOCK(os.stat(self.pfad_socket).st_mode):
                    break
            except OSError:
                pass
            time.sleep(0.02)
        else:
            return {"ok": False, "fehler": "zeit"}
        return {"ok": True}

    def messen(self):
        """Modes and network sockets, taken once the service listens (after the first connect or at the end)."""
        if self.gemessen is not None:
            return self.gemessen
        st = os.stat(self.pfad_socket)
        od = os.stat(self.pfad_ordner)
        # positive control: lsof must see this program, shown by its own Unix socket in the same kind of output; without that, a count of 0
        # network sockets would mean nothing (a program that is not observable shows 0 lines for everything)
        unix = lsof_zeilen(self.proc.pid, "-U")
        ordner, name = os.path.basename(os.path.dirname(self.pfad_socket)), os.path.basename(self.pfad_socket)
        if not any(ordner in z and name in z for z in unix):
            raise Umgebung("Positivkontrolle: lsof sieht den Unix-Socket des Dienstes nicht (%d Zeilen mit -U); der Dienst waere fuer die "
                           "Pruefung der Netzwerk-Sockets unsichtbar" % len(unix))
        zeilen = lsof_zeilen(self.proc.pid, "-i")
        self.gemessen = {"ordner_modus": "%04o" % (od.st_mode & 0o7777), "socket_modus": "%04o" % (st.st_mode & 0o7777),
                         "netz_sockets": len(zeilen)}
        return self.gemessen

    # ---------- reading
    def verarbeite(self, v):
        b = v.buf
        while len(b) >= 4:
            n = struct.unpack(">I", bytes(b[:4]))[0]
            if len(b) < 4 + n:
                break
            art, nutz = b[4], bytes(b[5:4 + n])
            del b[:4 + n]
            v.gesehen += 1
            if art == 0x05:
                try:
                    v.ver.empfange(nutz)
                except ra_ref.Fehler as e:
                    v.abbild_fehler.append(e.code)
                continue
            if art == 0x11:
                v.quittung += 1
                continue
            try:
                o = json.loads(nutz.decode("utf-8"))
            except ValueError:
                v.fehler.append("json_vom_dienst")
                continue
            if art == 0x06:
                # Nachtrag 27.11.2026 (DI-E5): the Ereignisliste only in cases with the field `ereignisliste`; the frame counts for Verbinden
                if o.get("art") == "Ereignisliste" and not self.fall.get("ereignisliste"):
                    continue
                v.nachrichten.append(o)
                if o.get("art") == "Sitzungsliste":
                    v.liste = o
            elif art == 0x07:
                v.ablehnungen.append(o.get("grund"))
                v.abl_voll.append(o)
            elif art == 0x08:
                v.fehler.append(o.get("fehler"))
            elif art == 0x12:
                v.antwort = o
            else:
                v.fehler.append("art_%d" % art)

    def lesen(self, timeout):
        """Reads whatever is there for up to `timeout` s; returns whether any byte came."""
        gelesen = False
        for key, _ in self.sel.select(timeout):
            v = key.data
            try:
                d = v.sock.recv(65536)
            except (BlockingIOError, InterruptedError):
                continue
            except OSError:
                d = b""
            if not d:
                v.getrennt = True
                try:
                    self.sel.unregister(v.sock)
                except (KeyError, ValueError):
                    pass
                continue
            gelesen = True
            v.roh += d
            v.buf += d
            self.verarbeite(v)
            for t in v.text():
                if "PID=" in t:
                    try:
                        self.pid_kind = int(t.split("PID=")[1].split()[0])
                    except (ValueError, IndexError):
                        pass
            for zl in v.alle_texte():       # DI12: every session's child reports its PID (the fixed child `warte`)
                for t in zl:
                    if "PID=" in t:
                        try:
                            self.pids.add(int(t.split("PID=")[1].split()[0]))
                        except (ValueError, IndexError):
                            pass
        return gelesen

    def sende(self, v, daten):
        try:
            v.sock.settimeout(10)
            v.sock.sendall(daten)
            v.sock.setblocking(False)
            return True
        except OSError:
            v.getrennt = True
            return False

    def wahl(self, schritt):
        n = schritt.get("betrachter")
        if n not in self.viewer:
            raise Ungueltig("Betrachter unbekannt: %r" % (n,))
        return self.viewer[n]

    # ---------- steps
    def schritt(self, s):
        art = s.get("art")
        if art == "Verbinden":
            n = s["betrachter"]
            if n in self.viewer:
                raise Ungueltig("Betrachter doppelt")
            t0 = time.monotonic()
            ziel = self.pfad_socket
            if s.get("weg") == "lokal":       # Nachtrag 26.11.2026, DI-L11
                kanal = (self.fall.get("start") or {}).get("lokal", {}).get("kanal", "")
                ziel = os.path.join(self.pfad_ordner, kanal + ".sock")
            while True:
                sk = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                sk.settimeout(10)
                try:
                    sk.connect(ziel)
                    break
                except (ConnectionRefusedError, FileNotFoundError):
                    sk.close()
                    if time.monotonic() - t0 > 5 or self.proc.poll() is not None:
                        raise
                    time.sleep(0.02)
            if s.get("weg") == "lokal":
                sk.settimeout(5)
                try:
                    sk.sendall(bytes.fromhex(s.get("hallo", "706b310a")))
                    antwort = b""
                    while len(antwort) < 4:
                        d = sk.recv(4 - len(antwort))
                        if not d:
                            break
                        antwort += d
                except OSError:
                    antwort = b""
                if antwort != b"pk1\n":
                    sk.close()
                    return {"ok": False, "fehler": "abgewiesen"}
            sk.setblocking(False)
            v = Betrachter(sk, bool(s.get("deflate", self.deflate)))
            self.viewer[n] = v
            if s.get("lesen", True):
                self.sel.register(sk, selectors.EVENT_READ, v)
            if self.erste is None:
                self.erste = n
            self.messen()
            if s.get("warte_erste", True) and s.get("lesen", True):
                t0 = time.monotonic()
                while v.gesehen == 0 and not v.getrennt and time.monotonic() - t0 < 5:
                    self.lesen(0.05)
            return {"ok": True}
        if art in ("Ereignis", "Uhr", "Stumm", "Kernfehler", "Rahmen"):
            v = self.wahl(s) if "betrachter" in s else self.viewer[self.erste]
            if art == "Rahmen":
                a = s["art_rahmen"]
                a = ARTEN[a] if isinstance(a, str) else int(a)
                nutz = bytes.fromhex(s["hex"]) if "hex" in s else jb(s["json"])
                nutz += b" " * int(s.get("auffuellen_bis", 0) - 1 - len(nutz)) if s.get("auffuellen_bis") else b""
                daten = rahmen(a, nutz)
            elif art == "Ereignis":
                daten = rahmen(0x01, jb(s["ereignis"]))
            elif art == "Uhr":
                self.t_uhr = s["t"]
                daten = rahmen(0x10, jb({"t": s["t"]}))
            elif art == "Stumm":
                daten = rahmen(0x11, jb({"an": s["an"]}))
                v.quittung = 0
            else:
                daten = rahmen(0x13, b"")
            ok = self.sende(v, daten)
            if art == "Stumm" and ok:
                t0 = time.monotonic()
                while v.quittung == 0 and not v.getrennt and time.monotonic() - t0 < 5:
                    self.lesen(0.02)
                ok = v.quittung > 0
            self.lesen(0)
            return {"ok": ok}
        if art == "Roh":
            v = self.wahl(s)
            ok = self.sende(v, bytes.fromhex(s["hex"]))
            if s.get("schliessen_schreiben"):
                try:
                    v.sock.shutdown(socket.SHUT_WR)
                except OSError:
                    pass
            self.lesen(0)
            return {"ok": ok}
        if art == "Trennen":
            v = self.wahl(s)
            try:
                self.sel.unregister(v.sock)
            except (KeyError, ValueError):
                pass
            v.sock.close()
            v.getrennt = True
            return {"ok": True}
        if art == "Lesen":
            # starts or stops reading of one viewer: a viewer that does not read makes the queue of the service grow (DI5)
            v = self.wahl(s)
            if not v.getrennt:
                try:
                    if s.get("an", True):
                        self.sel.register(v.sock, selectors.EVENT_READ, v)
                    else:
                        self.sel.unregister(v.sock)
                except (KeyError, ValueError):
                    pass
            return {"ok": True}
        if art == "Warte":
            return self.warte(s)
        if art == "Zustand":
            return self.zustand(s)
        if art == "Einstellungen":
            # addendum of 20.11.2026: writes the settings file of the case anew (the service reads it at every Starten)
            f = os.path.join(self.base, "einst.json")
            with open(f, "w") as h:
                json.dump(s["inhalt"], h)
            return {"ok": True}
        if art == "MarkerDatei":
            # addendum of 21.11.2026: writes the marker file anew while the service runs (the service read it once at its start and never again)
            with open(self.mk_datei, "w") as h:
                json.dump(s["inhalt"], h)
            return {"ok": True}
        if art == "Ordner":
            # DI12 point 12: the content of the socket folder now. `warte_auf`: names that must be there (up to `frist` s; the markers `*.term` only on
            # Linux); `pause`: seconds to wait first (for a signal that must NOT come); `lesen`: names of files whose content is part of the result
            t0 = time.monotonic()
            noetig = ohne_marker(list(s.get("warte_auf", [])))
            while time.monotonic() - t0 < float(s.get("frist", 5)):
                if all(os.path.lexists(os.path.join(self.pfad_ordner, n)) for n in noetig):
                    break
                self.lesen(0.02)
            t0 = time.monotonic()
            while time.monotonic() - t0 < float(s.get("pause", 0)):
                self.lesen(0.02)
            try:
                namen = ohne_marker(sorted(os.listdir(self.pfad_ordner)))
            except OSError:
                namen = None
            inhalte = {}
            for n in s.get("lesen", []):
                try:
                    with open(os.path.join(self.pfad_ordner, n), "rb") as h:
                        inhalte[n] = re.sub(r"[0-9]+", "#", h.read(200).decode("utf-8", "replace"))      # a pid is not part of the result
                except OSError:
                    inhalte[n] = None
            return {"ok": True, "namen": namen, "inhalte": inhalte}
        if art == "GespraechAnlegen":
            # Nachtrag 27.11.2026: the conversation of a session after its start, as `lauf` makes it from stage B on (the number is handed out first;
            # an entry before the start would take the number by DI12 point 15); the mode of lauf.gespraeche stays as the case set it
            modus = os.stat(self.gs_dir).st_mode & 0o7777
            os.chmod(self.gs_dir, 0o700)
            try:
                self.gespraech_anlegen(s["sitzung"])
            finally:
                os.chmod(self.gs_dir, modus)
            return {"ok": True}
        if art == "VorgabeAendern":
            # Nachtrag 27.11.2026: replaces the Firmenvorgabe while the service runs (new file next to it, rename), as the helper or the IT would
            neu = self.fv_datei + ".neu"
            with open(neu, "w", encoding="utf-8") as h:
                h.write(json.dumps(s["inhalt"], sort_keys=True, indent=1) + "\n")
            os.chmod(neu, 0o644)
            os.rename(neu, self.fv_datei)
            return {"ok": True}
        if art == "Pause":
            t0 = time.monotonic()
            while time.monotonic() - t0 < s.get("sekunden", 0.1):
                self.lesen(0.02)
            return {"ok": True}
        raise Ungueltig("Schritt %r" % (art,))

    def erfuellt(self, v, bis):
        if "liste" in bis:
            # DI12: the last Sitzungsliste holds an entry that matches every given field
            return bool(v.liste) and any(all(x.get(k) == w for k, w in bis["liste"].items()) for x in v.liste.get("sitzungen", []))
        if "text" in bis:
            return any(bis["text"] in t for t in v.text(bis.get("nr")))
        if bis.get("kindende"):
            return bool(v.liste) and any(x.get("zustand") != "laeuft" for x in v.liste.get("sitzungen", []))
        if "nachricht" in bis:
            return any(all(n.get(k) == w for k, w in bis["nachricht"].items()) for n in v.nachrichten)
        if "titel" in bis:
            return bool(v.liste) and any(x.get("titel") == bis["titel"] for x in v.liste.get("sitzungen", []))
        if "ablehnung" in bis:
            return bis["ablehnung"] in v.ablehnungen
        if "fehler" in bis:
            return bis["fehler"] in v.fehler
        if bis.get("getrennt"):
            return v.getrennt
        if "cursor" in bis:
            return any(list(ab.cursor) == bis["cursor"] for _, ab in sorted(v.ver.empfang.items()))
        raise Ungueltig("Warte.bis")

    def warte(self, s):
        v = self.wahl(s)
        bis = s["bis"]
        frist = float(s.get("frist", FRIST))
        takt = s.get("takt", 0)
        t0 = time.monotonic()
        while not self.erfuellt(v, bis):
            if time.monotonic() - t0 > frist:
                return {"ok": False, "fehler": "zeit"}
            if v.getrennt and not bis.get("getrennt"):
                return {"ok": False, "fehler": "getrennt"}
            if not self.lesen(0.02) and takt and self.pruefmodus and not v.getrennt:
                self.t_uhr += takt
                self.sende(v, rahmen(0x10, jb({"t": self.t_uhr})))
        return {"ok": True}

    def zustand(self, s):
        v = self.wahl(s)
        dienst = None
        if self.pruefmodus and not v.getrennt:
            v.antwort = None
            if self.sende(v, rahmen(0x12, b"")):
                t0 = time.monotonic()
                while v.antwort is None and not v.getrennt and time.monotonic() - t0 < FRIST:
                    self.lesen(0.02)
            dienst = v.antwort
            if dienst is not None:
                # the PID of the child is not part of the result (DI12: the fixed child `warte` prints it into the screen of the carrier)
                tz = dienst.get("dienst", {}).get("traeger_zeilen")
                if isinstance(tz, list):
                    dienst["dienst"]["traeger_zeilen"] = [re.sub(r"PID=[0-9]+", "PID=#", z) for z in tz]
                elif isinstance(tz, dict):
                    dienst["dienst"]["traeger_zeilen"] = {k: [re.sub(r"PID=[0-9]+", "PID=#", z) for z in zl] for k, zl in tz.items()}
            if dienst is not None:
                # Nachtrag 26.11.2026, DI-L11: the account of this run becomes a placeholder
                import pwd
                # under Linux the service runs in the user namespace of beobacht_praefix (section 16): there the own uid is 0
                eigen_uid = 0 if beobacht_praefix() else os.geteuid()
                eigen = {"uid": eigen_uid, "name": pwd.getpwuid(eigen_uid).pw_name,
                         "system": "linux" if sys.platform.startswith("linux") else "macos"}
                for b in (dienst.get("dienst", {}).get("betrachter") or {}).values():
                    w = b.get("wer") if isinstance(b, dict) else None
                    if isinstance(w, dict):
                        for k, platz in (("uid", "@uid"), ("name", "@konto"), ("system", "@system")):
                            if w.get(k) == eigen[k]:
                                w[k] = platz
            if dienst is not None and not s.get("grenze"):
                dienst = json.loads(json.dumps(dienst))
                dienst.get("dienst", {}).pop("warteschlange_max", None)     # only shown when the step asks for it (grenze)
            if dienst is not None and not s.get("voll"):
                dienst = json.loads(json.dumps(dienst))
                dienst.get("dienst", {}).pop("traeger_zeilen", None)
                dienst.get("dienst", {}).pop("kind", None)
                for x in dienst.get("kern", {}).get("sitzungen", []):
                    x.pop("zustand", None)
                    x.pop("code", None)
                    x.pop("cursor", None)       # depends on whether the output has arrived, like zustand and code (Nachtrag 20.11.)
        else:
            t0 = time.monotonic()
            while time.monotonic() - t0 < 0.15:
                if self.lesen(0.03):
                    t0 = time.monotonic()
        nach = v.nachrichten
        if not s.get("mit_t"):
            nach = [{k: w for k, w in n.items() if k != "t"} for n in nach]
        r = {"ok": True, "abbild": v.abbild(), "nachrichten": nach, "ablehnungen": list(v.ablehnungen),
             "fehler": list(v.fehler), "abbild_fehler": list(v.abbild_fehler), "getrennt": v.getrennt, "antwort": dienst}
        if s.get("abbilder"):
            r["abbilder"] = v.abbilder()
        if s.get("ablehnungen_voll"):
            r["ablehnungen_voll"] = list(v.abl_voll)       # the whole payloads (grund, code, text), addendum of 20.11.2026
        if s.get("ohne_nachrichten"):
            r["nachrichten"] = len(nach)
        if s.get("nachrichten_aus"):
            r["nachrichten"] = None       # the number depends on the kernel buffer (DI5, bound in the operating mode)
        v.nachrichten, v.ablehnungen, v.fehler, v.abbild_fehler = [], [], [], []
        v.abl_voll = []
        return r

    # ---------- end
    def ende(self):
        for v in self.viewer.values():
            try:
                v.sock.close()
            except OSError:
                pass
        rc = None
        if self.proc and self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                rc = self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
                rc = "kill"
        elif self.proc:
            rc = self.proc.returncode
        kind = None
        if self.fall.get("kind_pruefen") and self.pid_kind:
            time.sleep(0.2)
            try:
                os.kill(self.pid_kind, 0)
                kind = False
            except ProcessLookupError:
                kind = True
            except PermissionError:
                kind = False
        r = {"exitcode": rc, "socket_entfernt": not os.path.lexists(self.pfad_socket), "kind_beendet": kind}
        lokal = (self.fall.get("start") or {}).get("lokal") if isinstance(self.fall.get("start"), dict) else None
        if isinstance(lokal, dict) and isinstance(lokal.get("kanal"), str):
            r["lokal_entfernt"] = not os.path.lexists(os.path.join(self.pfad_ordner, lokal["kanal"] + ".sock"))
        if self.fall.get("bestand") is not None:
            # Nachtrag 26.11.2026, DI-L5: the Bestand after the end; the throw-away paths become their placeholders
            try:
                with open(self.bestand_datei, encoding="utf-8") as h:
                    r["bestand"] = json.loads(h.read().replace(self.pfad_ordner, "@ordner"))
                r["bestand_modus"] = "%04o" % stat.S_IMODE(os.lstat(self.bestand_datei).st_mode)
            except (OSError, ValueError):
                r["bestand"] = None
            try:
                r["bestand_rest"] = sorted(x for x in os.listdir(self.bestand_dir) if x != "bestand.json")
            except OSError:
                r["bestand_rest"] = None
        if self.fall.get("gespraeche") is not None:
            # Nachtrag 27.11.2026, DI-L3: the conversations after the end, and everything under <base>/aussen (nothing may be gone there)
            try:
                r["gespraeche_inhalt"] = sorted(os.listdir(self.gs_dir))
            except OSError:
                r["gespraeche_inhalt"] = None
            aussen = []
            for wurzel, dirs, files in os.walk(self.aussen_dir):
                rel = os.path.relpath(wurzel, self.aussen_dir)
                aussen += [os.path.normpath(os.path.join(rel, x)) + "/" for x in dirs] + [os.path.normpath(os.path.join(rel, x)) for x in files]
            r["aussen_inhalt"] = sorted(aussen) if os.path.isdir(self.aussen_dir) else None
        if self.fall.get("firmenvorgabe") is not None or self.fall.get("gespraeche") is not None:
            # Nachtrag 27.11.2026: the lines `wb-dienst: firmenvorgabe nicht geladen: <code>` and `wb-dienst: gespraech nicht geloescht: <code>`
            try:
                with open(os.path.join(self.base, "stderr"), "rb") as h:
                    zz = h.read().decode("utf-8", "replace").split("\n")
            except OSError:
                zz = []
            r["vorgabe_protokoll"] = [mo.group(1) for mo in (re.match(r"wb-dienst: firmenvorgabe nicht geladen: ([a-z0-9_.]+)$", z) for z in zz) if mo]
            r["gespraech_protokoll"] = [mo.group(1) for mo in (re.match(r"wb-dienst: gespraech nicht geloescht: ([a-z0-9_.]+)$", z) for z in zz) if mo]
        if self.fall.get("kinder_pruefen"):
            # DI12: every child reported with PID= (any session) is gone after the end of the service
            time.sleep(0.3)
            weg = True
            for pid in sorted(self.pids):
                try:
                    os.kill(pid, 0)
                    weg = False
                except ProcessLookupError:
                    pass
                except PermissionError:
                    weg = False
            r["kinder_beendet"] = weg and bool(self.pids)
        if self.fall.get("ordner_pruefen"):
            # DI12: after the end the socket folder holds nothing the service made (sub-folders of the sessions, socket)
            if self.fall.get("ende_warte"):
                time.sleep(float(self.fall["ende_warte"]))        # a signal at the end of the service arrives a moment later
            try:
                r["ordner_inhalt"] = ohne_marker(sorted(os.listdir(self.pfad_ordner)))
            except OSError:
                r["ordner_inhalt"] = None
        for feld, verz in (("ziellisten", self.zl_dir), ("laufzeit", self.lz_dir), ("marker_zaehl", self.mz_dir)):
            if self.fall.get(feld) is not None and not self.fall[feld].get("fehlt"):
                # the service makes nothing under lauf.ziellisten, lauf.laufzeit and lauf.marker_ordner and removes nothing there, except an old own list by DI12 point 15a
                try:
                    os.chmod(verz, 0o700)
                    r[feld + "_inhalt"] = sorted(os.listdir(verz))
                except OSError:
                    r[feld + "_inhalt"] = None
        if self.fall.get("sitzungen") is not None and not self.fall["sitzungen"].get("fehlt"):
            # Kennung: the service makes nothing and removes nothing under lauf.sitzungen; the entries after the end are the ones the case made
            try:
                os.chmod(self.sitz_dir, 0o700)
                r["sitzungen_inhalt"] = sorted(os.listdir(self.sitz_dir))
            except OSError:
                r["sitzungen_inhalt"] = None
        if self.fall.get("protokoll"):
            # the protocol of the service is its stderr (DI5): one line `wb-dienst: betrachter <name> getrennt: <grund> (...)` per cut
            try:
                with open(os.path.join(self.base, "stderr"), "rb") as h:
                    zeilen = h.read().decode("utf-8", "replace").split("\n")
            except OSError:
                zeilen = []
            r["protokoll"] = [{"betrachter": mo.group(1), "grund": mo.group(2)} for mo in
                              (re.match(r"wb-dienst: betrachter (b[0-9]+) getrennt: ([a-z_]+)", z) for z in zeilen) if mo]
            # Nachtrag 26.11.2026, DI-L11 point 5: one line per rejection on the local way, only the reason
            lokal_z = [mo.group(1) for mo in (re.match(r"wb-dienst: lokal abgewiesen: ([a-z0-9_.:-]+)$", z) for z in zeilen) if mo]
            if lokal_z:
                r["lokal_protokoll"] = lokal_z
            if self.fall.get("geheim"):
                # the protocol line holds no content of the session (decision of 20.11.2026): none of the words, and none in Base64, stands in any line of stderr
                import base64
                worte = [w for w in self.fall["geheim"] if w]
                worte += [base64.b64encode(w.encode()).decode().rstrip("=") for w in worte]
                r["protokoll_ohne_inhalt"] = not any(w in z for z in zeilen for w in worte)
        return r

    def aufraeumen(self):
        try:
            if os.path.isdir(self.o) and not os.path.islink(self.o):
                os.chmod(self.o, 0o700)
            for k in ("ziellisten", "laufzeit", "marker-zaehl"):
                if os.path.isdir(os.path.join(self.base, k)):
                    os.chmod(os.path.join(self.base, k), 0o700)
            for k in ("mk", "mk-echt"):
                if os.path.isdir(os.path.join(self.base, k)) and not os.path.islink(os.path.join(self.base, k)):
                    os.chmod(os.path.join(self.base, k), 0o700)
            if os.path.isdir(os.path.join(self.base, "sitzungen")):
                os.chmod(os.path.join(self.base, "sitzungen"), 0o700)
            if os.path.isdir(os.path.join(self.base, "gespraeche")) and not os.path.islink(os.path.join(self.base, "gespraeche")):
                os.chmod(os.path.join(self.base, "gespraeche"), 0o700)
            if os.path.isdir(os.path.join(self.base, "vorgabe")) and not os.path.islink(os.path.join(self.base, "vorgabe")):
                os.chmod(os.path.join(self.base, "vorgabe"), 0o700)
        except OSError:
            pass
        if self.proc and self.proc.poll() is None:
            self.proc.kill()
        if self.vorzeuge and self.vorzeuge.poll() is None:
            self.vorzeuge.kill()
        if self.vorzeuge:
            self.vorzeuge.wait()
        shutil.rmtree(self.base, ignore_errors=True)


def main(argv):
    if len(argv) != 2 or "DI_BIN" not in os.environ:
        print("Aufruf: DI_BIN=<dienst> pruefclient.py <fall.json>", file=sys.stderr)
        return 3
    try:
        with open(argv[1], encoding="utf-8") as f:
            fall = json.load(f)
        if not isinstance(fall, dict) or not isinstance(fall.get("schritte", []), list) or not ("start" in fall or "start_roh" in fall):
            raise Ungueltig("Fall")
    except (OSError, ValueError, Ungueltig) as e:
        print("pruefclient: Fall ungueltig: %s" % e, file=sys.stderr)
        return 2
    lauf = Lauf(fall)
    try:
        start = lauf.start()
        erg = []
        if start["ok"]:
            for s in fall.get("schritte", []):
                erg.append(lauf.schritt(s))
            if lauf.proc.poll() is None:
                if lauf.gemessen is None:
                    sk = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                    t0 = time.monotonic()
                    while True:
                        try:
                            sk.connect(lauf.pfad_socket)
                            break
                        except (ConnectionRefusedError, FileNotFoundError):
                            if time.monotonic() - t0 > 5:
                                break
                            time.sleep(0.02)
                    sk.close()
                start.update(lauf.messen())
        ende = lauf.ende()
        sys.stdout.write(json.dumps({"datensatz": fall.get("datensatz"), "start": start, "ergebnisse": erg, "ende": ende}, ensure_ascii=False, sort_keys=True) + "\n")
        return 0
    except Ungueltig as e:
        print("pruefclient: Fall ungueltig: %s" % e, file=sys.stderr)
        return 2
    except (Umgebung, KeyError, OSError) as e:
        print("pruefclient: Umgebung: %r" % (e,), file=sys.stderr)
        return 3
    finally:
        lauf.aufraeumen()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
