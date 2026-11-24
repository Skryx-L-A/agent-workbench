"""Outside test runner of the contract `datentor` (rust/spec/VERTRAEGE.md, section 6).

  DT_BIN=<program> DT_FAELLE=<set directory with faelle/ and erwartet/> python3 -I -B aussen_laeufer.py [name ...]

Each case file goes to the program on stdin. Exit 0 expected: stdout is one JSON object, compared (parsed, so key order and
whitespace are free) with `ausgabe` of erwartet/<name>.json. Exit 2 expected: stdout empty and exactly one line on stderr.
Exit 0 = all green, 1 = red cases, 2 = call error. DT_ROT=<file> writes the names of the red cases there, one per line.
Not named test_*.py, so that no suite without the program collects it.
"""
import json
import os
import subprocess
import sys
from pathlib import Path


def kurz(x):
    return json.dumps(x, ensure_ascii=False)[:120]


def unterschied(a, b, pfad="$"):
    """First difference of `a` (actual) against `b` (expected), or None. true and 1 differ."""
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


def pruefe(bin_, fall, erw):
    """Reason the case is red, or None."""
    try:
        p = subprocess.run([bin_], input=fall.read_bytes(), capture_output=True, timeout=30)
    except subprocess.TimeoutExpired:
        return "Zeitlimit 30 s"
    except OSError as e:
        return f"Programm nicht startbar: {e.strerror}"
    if p.returncode != erw["exit"]:
        return f"Exitcode {p.returncode} statt {erw['exit']}"
    if erw["exit"] == 2:
        if p.stdout:
            return "stdout nicht leer"
        zeilen = p.stderr.decode("utf-8", "replace").splitlines()
        if len(zeilen) != 1 or not zeilen[0].strip():
            return f"stderr hat {len(zeilen)} Zeilen statt genau einer"
        return None
    try:
        ist = json.loads(p.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return "stdout ist kein JSON in UTF-8"
    return unterschied(ist, erw["ausgabe"])


def main(argv):
    bin_, faelle = os.environ.get("DT_BIN"), os.environ.get("DT_FAELLE")
    if not bin_ or not faelle:
        print("DT_BIN und DT_FAELLE setzen", file=sys.stderr)
        return 2
    basis = Path(faelle)
    dateien = sorted((basis / "faelle").glob("*.json"))
    if argv[1:]:
        dateien = [d for d in dateien if d.stem in argv[1:]]
    if not dateien:
        print(f"keine Faelle unter {basis}/faelle", file=sys.stderr)
        return 2
    rot = []
    for d in dateien:
        erw_pfad = basis / "erwartet" / d.name
        grund = "Erwartung fehlt" if not erw_pfad.exists() else pruefe(
            bin_, d, json.loads(erw_pfad.read_text(encoding="utf-8")))
        if grund:
            rot.append(d.stem)
            print(f"ROT  {d.stem}: {grund}")
        else:
            print(f"OK   {d.stem}")
    print(f"{len(dateien) - len(rot)} von {len(dateien)} gruen")
    ziel = os.environ.get("DT_ROT")
    if ziel:
        Path(ziel).write_text("".join(f"{n}\n" for n in rot), encoding="utf-8")
    return 1 if rot else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
