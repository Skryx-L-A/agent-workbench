#!/usr/bin/env -S -i /usr/bin/python3 -I -S -X utf8
"""wb_identitaet -- the one source for account, home and uid of this process (23.10.2026).

THE RULE. Who this process runs as and where its home lies come from the user database
(getpwuid of the real uid), never from HOME, USER, LOGNAME or any WB_* variable. This module reads
no environment variable and falls back to nothing: without an account entry the home is '' and
the caller stays fail closed (no human proof, no register, no tool from that home).

ONE MODE, NO SECOND CODE PATH. wb-identitaet, wb-stelle and the Stelle service (wb_stelle/dienst.py)
use these functions; so do the Werkbank tools that need their home on a trust path. Questions only
the service can answer (who in server mode, role, darf) go to the service through wb-identitaet and
stay fail closed when it does not answer. The test seam WB_TESTUMGEBUNG=1 stays in the tools; this
module knows no seam (removing the seam is step 2c).

hooks/lib/identitaet.py is a copy of this file so that hooks/ stands on its own; the parity test
(shell/tests/test-identitaet.py) keeps both equal.

THE STELLE DIRECTORY (Stufe 2b+, 27.10.2026). stelle_ordner() is the one place that names it, for the
service and for every person asking it. Without the key "stelle" in the root-owned system level
it is "<home of the own account>/.local/share/agent-workbench/stelle" (single seat, as before 2b+).
With stelle.konto it is "<home of getpwnam(stelle.konto)>/.local/share/agent-workbench/stelle", the
same for the service account and for every person. An account that does not exist, and a key
"stelle" that is no object or has no valid "konto", give '' (fail closed: the service ends with exit
78, wb-identitaet refuses with R-STELLE-PFAD). stelle.konto comes ONLY
from the system file (SYSTEM_PFADE), read the way shell/wb_naht_ziel.py reads tmuxSocket: a regular
file, not group or world writable, owner in SYSTEM_EIGENTUEMER, every directory up to
SYSTEM_KETTE_BIS likewise, at most MAX_BYTES, JSON without a duplicate key, schema 'wb-konfig/1',
a well-formed line gesperrt. Operator level, user level and the environment never choose the account.

A FIRM COMPUTER NEVER FALLS BACK TO THE SINGLE SEAT (second security review, finding 2, 4.11.2026). The question "is this a firm computer" has
the answer of wb_konfig.betriebsart() for the system file: a file that is there but not valid, or the mark MARKE_NAME beside the
system file, say yes. Then stelle_konto() and stelle_modus() give None and stelle_gruppe() gives None (fail closed: the gate
refuses, the door does not start, the service ends with exit 78), never FEHLT or "einzelplatz". Only a computer with neither
system file nor mark is a single seat without a Stelle, as before. The code is a copy of that check, not a call: hooks/lib/identitaet.py
is a byte copy that stands alone, and wb_konfig.py loads this module, so a call back would be a cycle. The suite
shell/tests/test-gate-systemdatei.py asks wb_konfig.betriebsart() for the same files and fails when the two answers differ.

THE STELLE GROUP (Stufe 2b+, 27.10.2026). stelle.gruppe, from the same system file only, names the group
through which persons reach stelle.sock when service and person run under different accounts.
stelle_gruppe() gives its gid; FEHLT without the key (single seat, nothing changes), None (fail closed:
wb-stelle ends with exit 78) for an unknown group, a value that is no name, or a group without a valid
stelle.konto. stelle_basis() is the home the Stelle directory hangs under; the service sets the group on
the directories from there down (rule R-STELLE-GRUPPE, docs/ENTSCHEIDUNGSSTELLE.md).

THE MODE OF THE STELLE (w65, 31.10.2026). stelle_modus() gives stelle.modus of the same system file:
"einzelplatz" without the file or the key "stelle", "einzelplatz" or "server" when the value stands
exactly so, else None (fail closed). Only a "server" lets the door (wb-identitaet) skip the run report
that the Stelle would refuse there with R-LAUF-SERVER.

TESTS rewrite the one line HEIMAT_FEST, and SYSTEM_PFADE, SYSTEM_EIGENTUEMER, SYSTEM_KETTE_BIS (one
line each), in a COPY of this module, never through a variable.

Command line (for Bash tools; they call it with a fixed interpreter under `env -i` and find it next
to themselves via realpath):
  wb_identitaet.py heimat     home of the real uid, without newline
  wb_identitaet.py kennung    real uid
  wb_identitaet.py name       account name of the real uid
  wb_identitaet.py stelle     the Stelle directory (see above)
  wb_identitaet.py stelle-modus  "einzelplatz" or "server" (stelle.modus, see stelle_modus()); no output
                              and exit 1 when the value is missing or invalid: ask the Stelle then
Exit 1: no account entry. Exit 2: wrong call.
"""
import grp
import json
import os
import pwd
import stat
import sys

HEIMAT_FEST = None   # Umschreibpunkt fuer Testkopien (eine Zeile), nie aus der Umgebung
SYSTEM_PFADE = {'linux': '/etc/agent-workbench/werkbank.json', 'darwin': '/Library/Application Support/agent-workbench/werkbank.json'}
SYSTEM_EIGENTUEMER = (0,)
SYSTEM_KETTE_BIS = '/'
SCHEMA = 'wb-konfig/1'
MAX_BYTES = 1 << 20
MARKE_NAME = 'firmenrechner'   # as wb_konfig.MARKE_NAME: lies beside the system file
STELLE_TEILE = ('.local', 'share', 'agent-workbench', 'stelle')
FEHLT = object()
# REGEL I4-TUER-ORT (decision 2026-11-07, task): the root-owned door of the installation. On the Mac /Library/Werkbank/bin (deploy/arbeitsplatz
# orte.WERKZEUG_BIN; /usr/local/bin can belong to the admin account there, and it no longer counts), under Linux /usr/local/bin as before. A test
# keeps both equal to orte.IDENTITAET_ORT. tuer_pfad() is the one reading every starter and the gate take the door from.
TUER_FEST = {'linux': '/usr/local/bin/wb-identitaet', 'darwin': '/Library/Werkbank/bin/wb-identitaet'}
TUER_SATZ = ("Die Tuer der Werkbank (wb-identitaet) fehlt am festen Ort %s, oder sie gehoert dort nicht root oder ist fuer andere "
             "schreibbar, darum startet nichts: Der Betreiber installiert die Werkbank neu.")


def kennung():
    """Real uid of this process."""
    return os.getuid()


def konto(uid=None):
    """Entry of the user database for uid (default: the real uid); None without an entry."""
    try:
        return pwd.getpwuid(kennung() if uid is None else uid)
    except KeyError:
        return None


def heimat(uid=None):
    """Home of uid (default: the real uid) from the user database; '' without an entry."""
    if uid is None and HEIMAT_FEST is not None:
        return HEIMAT_FEST
    k = konto(uid)
    return k.pw_dir if k is not None else ""


def name(uid=None):
    """Account name of uid (default: the real uid); '' without an entry."""
    k = konto(uid)
    return k.pw_name if k is not None else ""


def _kette_ok(pfad):
    """Every directory up to SYSTEM_KETTE_BIS belongs to SYSTEM_EIGENTUEMER and is not group or
    world writable (as wb_naht_ziel._kette_ok)."""
    d = os.path.dirname(os.path.realpath(pfad))
    bis = os.path.realpath(SYSTEM_KETTE_BIS)
    while True:
        try:
            st = os.stat(d)
        except OSError:
            return False
        if st.st_mode & 0o022 or st.st_uid not in SYSTEM_EIGENTUEMER:
            return False
        if d == bis or d == os.path.dirname(d):
            return True
        d = os.path.dirname(d)


def _system_daten(plattform=None):
    """Content of the valid system file as a dict, else None (absent or invalid: _firmenrechner() tells which of the two matters)."""
    def paare(liste):
        d = {}
        for k, v in liste:
            if k in d:
                raise ValueError("doppelter Schluessel '%s'" % k)
            d[k] = v
        return d

    plattform = sys.platform if plattform is None else plattform
    pfad = next((p for pl, p in SYSTEM_PFADE.items() if plattform.startswith(pl)), None)
    if not pfad:
        return None
    try:
        st = os.stat(pfad)
    except OSError:
        return None
    if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_BYTES:
        return None
    if st.st_mode & 0o022 or st.st_uid not in SYSTEM_EIGENTUEMER or not _kette_ok(pfad):
        return None
    try:
        with open(pfad, 'r', encoding='utf-8') as f:
            daten = json.load(f, object_pairs_hook=paare)
    except (OSError, ValueError, UnicodeDecodeError):
        return None
    if not isinstance(daten, dict) or daten.get('schema') != SCHEMA:
        return None
    g = daten.get('gesperrt', [])
    if not (isinstance(g, list) and all(isinstance(x, str) and x for x in g)):
        return None
    return daten


def _da(pfad):
    """True when something stands at PFAD (also a broken link or a folder we may not look into); only "not there" is False
    (as wb_konfig._betriebsart_aus: any other error counts as there)."""
    try:
        os.lstat(pfad)
    except (FileNotFoundError, NotADirectoryError):
        return False
    except OSError:
        return True
    return True


def _firmenrechner(plattform=None):
    """True when this is a firm computer for the Stelle: the system file stands there (the caller found it not valid) or the
    mark MARKE_NAME beside it. False only with neither, the single seat without a Stelle. Same answer as wb_konfig.betriebsart()
    gives for a system file that is not valid or absent (a valid file with betriebsart mandant but without the key "stelle"
    stays a seat without a Stelle here, as before)."""
    plattform = sys.platform if plattform is None else plattform
    pfad = next((p for pl, p in SYSTEM_PFADE.items() if plattform.startswith(pl)), None)
    return bool(pfad) and (_da(pfad) or _da(os.path.join(os.path.dirname(pfad), MARKE_NAME)))


def stelle_konto():
    """stelle.konto from the valid system file: FEHLT only when the computer has neither a system file nor the mark of a firm
    computer, or the valid file has no key "stelle" (single seat). An invalid or (with the mark) missing system file gives None:
    fail closed, as wb_konfig.betriebsart() says mandant there. Otherwise the raw value; "stelle" that is no object, or an object
    without "konto", gives None. A present key shows the intent of several accounts, so a typo stays
    fail closed and never falls back to the home of the caller."""
    daten = _system_daten()
    if daten is None:
        return None if _firmenrechner() else FEHLT
    if 'stelle' not in daten:
        return FEHLT
    stelle = daten['stelle']
    if not isinstance(stelle, dict):
        return None
    return stelle.get('konto')


def _konto_heimat(wert):
    """Absolute home of the account named WERT, else '' (no account, no name, a path as name)."""
    basis = ""
    if isinstance(wert, str) and wert and "/" not in wert:
        try:
            basis = pwd.getpwnam(wert).pw_dir
        except (KeyError, ValueError):
            basis = ""
        if not os.path.isabs(basis):
            basis = ""
    return basis


def stelle_basis():
    """The home the Stelle directory hangs under: the own home without the key "stelle", the home of
    stelle.konto with it, '' when stelle.konto names no account. The service sets the group of
    stelle.gruppe on the directories from here down to the Stelle directory."""
    wert = stelle_konto()
    return heimat() if wert is FEHLT else _konto_heimat(wert)


def stelle_ordner():
    """The Stelle directory for service and person alike; '' when stelle.konto names no account."""
    basis = stelle_basis()
    return os.path.join(basis, *STELLE_TEILE) if basis else ""


def stelle_gruppe():
    """gid of stelle.gruppe from the valid system file: FEHLT without the key (no group way, single
    seat). None (fail closed) for a group that does not exist, a value that is no non-empty name, a
    key "stelle" that is no object, or stelle.gruppe without a valid stelle.konto."""
    if _system_daten() is None and _firmenrechner():
        return None
    stelle = (_system_daten() or {}).get('stelle', FEHLT)
    if stelle is FEHLT or (isinstance(stelle, dict) and 'gruppe' not in stelle):
        return FEHLT
    wert = stelle.get('gruppe') if isinstance(stelle, dict) else None
    if not isinstance(wert, str) or not wert or ":" in wert or not _konto_heimat(stelle.get('konto')):
        return None
    try:
        return grp.getgrnam(wert).gr_gid
    except (KeyError, ValueError):
        return None


STELLE_MODI = ("einzelplatz", "server")


def stelle_modus():
    """stelle.modus from the valid system file: "einzelplatz" without a system file and mark, or with a valid file
    without the key "stelle" (single seat without a Stelle); with the key exactly "einzelplatz" or
    "server" when the value stands so. Else None (fail closed: a missing or invalid value, a "stelle"
    that is no object, or an invalid or missing system file of a firm computer, see _firmenrechner()): the caller asks the
    Stelle, which decides."""
    if _system_daten() is None and _firmenrechner():
        return None
    daten = _system_daten() or {}
    if 'stelle' not in daten:
        return "einzelplatz"
    stelle = daten['stelle']
    wert = stelle.get('modus') if isinstance(stelle, dict) else None
    return wert if isinstance(wert, str) and wert in STELLE_MODI else None


def tuer_fest(plattform=None):
    """TUER_FEST of this platform, '' for another one."""
    plattform = sys.platform if plattform is None else plattform
    return next((p for pl, p in TUER_FEST.items() if plattform.startswith(pl)), '')


def _tuer_vertraut(pfad):
    """The door at PFAD counts: its realpath is a regular file of SYSTEM_EIGENTUEMER, not group or world writable, and the chain
    above PFAD and above its realpath is the same (_kette_ok, the check of the system file)."""
    try:
        echt = os.path.realpath(pfad)
        st = os.stat(echt)
    except OSError:
        return False
    return (stat.S_ISREG(st.st_mode) and st.st_uid in SYSTEM_EIGENTUEMER and not st.st_mode & 0o022
            and _kette_ok(pfad) and _kette_ok(echt))


def tuer_pfad(neben=(), plattform=None, fest=None):
    """REGEL I4-TUER-ORT: the door a starter or the gate uses. The root-owned door at FEST (default tuer_fest()) counts in every mode.
    Without it a firm computer (stelle_konto() is not FEHLT: the door would not say einzelplatz) gets None: no start, never the door
    next to the caller in the home. At the single seat the first executable of NEBEN as before, '' without one (start without door)."""
    fest = tuer_fest(plattform) if fest is None else fest
    if fest and _tuer_vertraut(fest):
        return fest
    if stelle_konto() is not FEHLT:
        return None
    return next((p for p in neben if p and os.access(p, os.X_OK)), '')


def main(argv):
    if argv[:1] == ["tuer"]:
        # `tuer NEBEN...` for the starters in Bash: the path (maybe empty, exit 0) or one sentence and exit 3
        pfad = tuer_pfad(argv[1:])
        if pfad is None:
            sys.stderr.write(TUER_SATZ % (tuer_fest() or 'dieser Plattform') + "\n")
            return 3
        sys.stdout.write(pfad)
        return 0
    if argv == ["heimat"]:
        wert = heimat()
    elif argv == ["kennung"]:
        wert = str(kennung())
    elif argv == ["name"]:
        wert = name()
    elif argv == ["stelle"]:
        wert = stelle_ordner()
    elif argv == ["stelle-modus"]:
        wert = stelle_modus()
    else:
        sys.stderr.write("wb_identitaet.py heimat|kennung|name|stelle|stelle-modus|tuer [NEBEN...]\n")
        return 2
    if not wert:
        return 1
    sys.stdout.write(wert)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
