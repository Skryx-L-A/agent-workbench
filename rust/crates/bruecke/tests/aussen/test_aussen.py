"""Outside test runner of the `bruecke` contract.

  BR_BIN=<bridge program> BR_FAELLE=<set directory with faelle/ and erwartet/> python3 -I -B test_aussen.py [name ...]

For each case the runner starts the Prüfclient (pruefclient.py next to this file; BR_PRUEFCLIENT names another one). The Prüfclient
starts the counterpart (the reference of the service) and the program BR_BIN, talks HTTP to the bridge and writes one JSON object. The runner compares that object
(parsed, so key order and whitespace are free) with erwartet/<name>.json. Exit 0 = all green, 1 = red cases, 2 = call error.
BR_ROT=<file> writes the names of the red cases there, one per line. BR_ZEIT=<s> sets the limit per case (default 90).
BR_NUR=<prefix,...> runs only the cases whose name starts with a prefix; BR_ERSTROT=1 stops after the first red case (both for the counter-check).

Entscheidung vom 20.11.2026 (Lehre aus dem Vertrag dienst: nach einem Zeitlimit blieben Dienst und Kinder stehen):
the Prüfclient runs in its own process group and gets its own throw-away base folder (BR_TMP). After every case, also after a
normal end, the runner ends the whole group (SIGTERM, after 2 s SIGKILL) and removes the folder. A child that the service put
into a session of its own (the PTY of the carrier) gets SIGHUP when the service ends; the runner does not track it further.
"""
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def gruppe_beenden(pgid):
    """Ends every process of the group `pgid`: SIGTERM, then SIGKILL after at most 2 s."""
    for sig, warte in ((signal.SIGTERM, 2.0), (signal.SIGKILL, 0.0)):
        try:
            os.killpg(pgid, sig)
        except (ProcessLookupError, PermissionError):
            return
        ende = time.monotonic() + warte
        while time.monotonic() < ende:
            try:
                os.killpg(pgid, 0)
            except (ProcessLookupError, PermissionError):
                return
            time.sleep(0.05)


def lauf(befehl, env, limit):
    """Runs the Prüfclient in its own process group and base folder; raises TimeoutExpired after `limit` seconds."""
    basis = tempfile.mkdtemp(prefix="bl-", dir=env.get("BR_TMP") or "/tmp")
    p = subprocess.Popen(befehl, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=dict(env, BR_TMP=basis),
                         start_new_session=True)
    try:
        out, err = p.communicate(timeout=limit)
    except subprocess.TimeoutExpired:
        gruppe_beenden(p.pid)
        try:
            p.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            p.kill()
        raise
    finally:
        gruppe_beenden(p.pid)
        shutil.rmtree(basis, ignore_errors=True)
    return subprocess.CompletedProcess(befehl, p.returncode, out, err)


def kurz(x):
    return json.dumps(x, ensure_ascii=False, sort_keys=True)[:160]


def unterschied(a, b, pfad="$"):
    """First difference of `a` (actual) against `b` (expected), or None."""
    if type(a) is not type(b):
        return f"{pfad}: {kurz(a)} statt {kurz(b)}"
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a:
                return f"{pfad}.{k}: fehlt"
            if k not in b:
                return f"{pfad}.{k}: zu viel"
            d = unterschied(a[k], b[k], f"{pfad}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list):
        for i, (x, y) in enumerate(zip(a, b)):
            d = unterschied(x, y, f"{pfad}[{i}]")
            if d:
                return d
        if len(a) != len(b):
            return f"{pfad}: Länge {len(a)} statt {len(b)}"
        return None
    if a != b:
        return f"{pfad}: {kurz(a)} statt {kurz(b)}"
    return None


def main(argv):
    bin_, faelle = os.environ.get("BR_BIN"), os.environ.get("BR_FAELLE")
    if not bin_ or not faelle:
        print("BR_BIN und BR_FAELLE setzen", file=sys.stderr)
        return 2
    client = os.environ.get("BR_PRUEFCLIENT") or str(Path(__file__).resolve().parent / "pruefclient.py")
    limit = float(os.environ.get("BR_ZEIT", "120"))
    basis = Path(faelle)
    dateien = sorted((basis / "faelle").glob("*.json"))
    if argv[1:]:
        dateien = [d for d in dateien if d.stem in argv[1:]]
    if os.environ.get("BR_NUR"):
        # Nachtrag 20.11.: only cases whose name starts with one of these prefixes (comma separated); the counter-check uses it
        # for the mutants of DI12, which only the cases di12_* can turn red
        pre = tuple(x for x in os.environ["BR_NUR"].split(",") if x)
        dateien = [d for d in dateien if d.stem.startswith(pre)]
    if not dateien:
        print(f"keine Faelle unter {basis}/faelle", file=sys.stderr)
        return 2
    rot = []
    for d in dateien:
        erw_pfad = basis / "erwartet" / d.name
        if not erw_pfad.exists():
            rot.append(d.stem)
            print(f"ROT  {d.stem}: Erwartung fehlt")
            continue
        erw = json.loads(erw_pfad.read_text(encoding="utf-8"))
        try:
            p = lauf([sys.executable, "-I", "-B", client, str(d)], dict(os.environ, BR_BIN=bin_), limit)
        except subprocess.TimeoutExpired:
            rot.append(d.stem)
            print(f"ROT  {d.stem}: Zeitlimit {limit:.0f} s")
            continue
        grund = None
        if p.returncode != erw["exit"]:
            grund = f"Exitcode {p.returncode} statt {erw['exit']}: {p.stderr.decode('utf-8', 'replace').strip()[-200:]}"
        elif erw["exit"] == 0:
            try:
                ist = json.loads(p.stdout)
            except json.JSONDecodeError:
                grund = "stdout ist kein JSON"
            else:
                grund = unterschied(ist, erw["ausgabe"])
        elif p.stdout.strip():
            grund = "stdout nicht leer bei Exitcode ungleich 0"
        if grund:
            rot.append(d.stem)
            print(f"ROT  {d.stem}: {grund}")
            if os.environ.get("BR_ERSTROT"):
                break       # the counter-check of an old mutant needs one red case, not all
    print(f"{len(dateien) - len(rot)} gruen, {len(rot)} rot von {len(dateien)} ({basis.name})")
    if os.environ.get("BR_ROT"):
        Path(os.environ["BR_ROT"]).write_text("".join(f"{n}\n" for n in rot), encoding="utf-8")
    return 1 if rot else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
