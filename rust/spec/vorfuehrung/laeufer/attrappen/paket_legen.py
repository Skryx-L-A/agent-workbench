#!/usr/bin/env python3
"""Legt die Paketform der Copilot-CLI-Attrappe in einen Zielordner (Form: `paketform-copilot-1.0.88`, Abschnitte Aufbau und Attrappe).

Aufruf: python3 -I -B paket_legen.py <prefix>     (fuer den Lauf auf der Testmaschine und den lokalen Lauf ist <prefix> `<heim>/.npm-global`)

Unter <prefix> entsteht, Fassung `0.0.88` in beiden Paketen:
  lib/node_modules/@github/copilot/package.json                                         0644
  lib/node_modules/@github/copilot/npm-loader.js                                        0755  Python-Loader, Shebang `#!/usr/bin/env python3`
  lib/node_modules/@github/copilot/node_modules/@github/copilot-linux-x64/package.json  0644
  lib/node_modules/@github/copilot/node_modules/@github/copilot-linux-x64/copilot       0755  gewoehnliche Datei: die Attrappe `copilot_attrappe.py`
Ordner 0755. Das Werkzeug legt keine Verweise an; `<prefix>/bin/copilot` (Verweis auf `npm-loader.js`) legt der Aufrufer an, wenn er ihn braucht.
Es schreibt nie durch einen Verweis: Ist eines der Ziele oder einer der Ordner ein Verweis, bricht es mit Exit 2 ab, bevor es etwas schreibt.
Nur Standardbibliothek, Python 3. Nicht ausgeliefert, wie alle Attrappen.
"""
from __future__ import annotations

import json
import os
import sys

HIER = os.path.dirname(os.path.abspath(__file__))
FASSUNG = "0.0.88"
PLATTFORM = "linux-x64"
HAUPT = os.path.join("lib", "node_modules", "@github", "copilot")
PLATTFORMPAKET = os.path.join(HAUPT, "node_modules", "@github", "copilot-" + PLATTFORM)

HAUPT_PACKAGE = {"name": "@github/copilot", "version": FASSUNG, "bin": {"copilot": "npm-loader.js"}, "type": "module",
                 "optionalDependencies": {"@github/copilot-" + PLATTFORM: FASSUNG}}
PLATTFORM_PACKAGE = {"name": "@github/copilot-" + PLATTFORM, "version": FASSUNG, "type": "module", "os": ["linux"], "cpu": ["x64"],
                     "libc": ["glibc"], "bin": {"copilot-" + PLATTFORM: "copilot"}, "exports": {".": "./copilot"}}

# Der Loader sucht das Plattformpaket wie npm-loader.js der echten CLI: Ort (1) neben sich im Ordner node_modules, dann Ort (2) im Ordner darueber.
# Er startet das Programm per exec und reicht Argumente, Umgebung und Terminal unveraendert durch.
LOADER = '''#!/usr/bin/env python3
import os
import sys

HIER = os.path.dirname(os.path.realpath(__file__))
for ort in (os.path.join(HIER, "node_modules", "@github", "copilot-%(plattform)s"), os.path.join(os.path.dirname(HIER), "copilot-%(plattform)s")):
    programm = os.path.join(ort, "copilot")
    if os.path.isfile(programm):
        try:
            os.execv(programm, [programm] + sys.argv[1:])
        except OSError:
            sys.stderr.write("Failed to start the platform package\\n")
            sys.exit(1)
sys.stderr.write("Failed to find the platform package @github/copilot-%(plattform)s\\n")
sys.exit(1)
''' % {"plattform": PLATTFORM}


def _json(d):
    return (json.dumps(d, indent=2) + "\n").encode("utf-8")


def dateien():
    """(relativer Pfad, Inhalt als Bytes, Modus) der vier Dateien."""
    with open(os.path.join(HIER, "copilot_attrappe.py"), "rb") as f:
        programm = f.read()
    return [(os.path.join(HAUPT, "package.json"), _json(HAUPT_PACKAGE), 0o644),
            (os.path.join(HAUPT, "npm-loader.js"), LOADER.encode("utf-8"), 0o755),
            (os.path.join(PLATTFORMPAKET, "package.json"), _json(PLATTFORM_PACKAGE), 0o644),
            (os.path.join(PLATTFORMPAKET, "copilot"), programm, 0o755)]


def lege(prefix):
    """Legt die vier Dateien unter `prefix` an. Wirft OSError, wenn ein Ordner oder Ziel ein Verweis ist, bevor es etwas schreibt."""
    prefix = os.path.abspath(prefix)
    liste = dateien()
    for rel, _, _ in liste:
        teile = rel.split(os.sep)
        for i in range(len(teile)):
            if os.path.islink(os.path.join(prefix, *teile[:i + 1])):
                raise OSError("Verweis im Weg: " + os.path.join(prefix, *teile[:i + 1]))
    for rel, inhalt, modus in liste:
        ziel = os.path.join(prefix, rel)
        ordner = os.path.dirname(ziel)
        os.makedirs(ordner, exist_ok=True)
        os.chmod(ordner, 0o755)
        fd = os.open(ziel, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, modus)
        with os.fdopen(fd, "wb") as f:
            f.write(inhalt)
        os.chmod(ziel, modus)
    return [os.path.join(prefix, rel) for rel, _, _ in liste]


def main(argv):
    if len(argv) != 2 or argv[1] in ("-h", "--help"):
        sys.stderr.write("Aufruf: paket_legen.py <prefix>\n")
        return 2
    try:
        for pfad in lege(argv[1]):
            print(pfad)
    except OSError as e:
        sys.stderr.write("paket_legen: %s\n" % e)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
