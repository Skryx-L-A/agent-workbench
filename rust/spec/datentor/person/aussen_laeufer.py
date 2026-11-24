"""Outside test runner of the part contract `datentor person` (person/VERTRAG.md, section 6).

  DT_BIN=<program> DT_FAELLE=<set directory with faelle/ and erwartet/> python3 -I -B aussen_laeufer.py [name ...]

Same runner as ../aussen_laeufer.py (loaded, not copied), with one difference: the program is called as `DT_BIN person`.
Exit 0 = all green, 1 = red cases, 2 = call error. Not named test_*.py, so that no suite without the program collects it.
"""
import importlib.util
import subprocess
import sys
from pathlib import Path

_spec = importlib.util.spec_from_file_location("datentor_laeufer", Path(__file__).resolve().parent.parent / "aussen_laeufer.py")
DT = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(DT)


class _MitUnterbefehl:
    """Stands in for the module subprocess in the loaded runner and adds the subcommand `person`."""
    TimeoutExpired = subprocess.TimeoutExpired

    @staticmethod
    def run(args, **kw):
        return subprocess.run(list(args) + ["person"], **kw)


DT.subprocess = _MitUnterbefehl

if __name__ == "__main__":
    sys.exit(DT.main(sys.argv))
