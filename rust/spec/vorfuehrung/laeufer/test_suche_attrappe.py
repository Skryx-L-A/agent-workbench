"""Prueft die Attrappe der Suche (`attrappen/modellweg.py suche`) gegen die Faelle in `rust/spec/modellweg/faelle/suche.json`
(Vertrag 8, Abschnitt 9a) und die Mutation `suche-folgt-verknuepfung`. Nur Standardbibliothek.
Aufruf: python3 -I -B test_suche_attrappe.py [--liste]

Jeder Fall baut seinen Dateibaum unter einem Wegwerf-Ordner; die festen Pfade `/srv/beispiel/lauf` und `/srv/beispiel` der Faelle
werden durch diesen Ordner ersetzt. Der Fall gilt, wenn Exit, stdout und stderr genau stimmen. Nicht nachbildbare Eigenschaften
(`eigen: false`, ein fremder Eigentuemer) kommen in den Faellen nicht vor; ein Fall mit `lesbar: false` braucht eine Kennung ohne root.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
ATTRAPPE = os.path.join(HIER, "attrappen", "modellweg.py")
FAELLE = os.path.join(os.path.dirname(HIER), "..", "modellweg", "faelle", "suche.json")
WURZEL_FALL = "/srv/beispiel"


def lade():
    with open(os.path.realpath(FAELLE), encoding="utf-8") as f:
        return json.load(f)["faelle"]


def ersetze(text, wurzel):
    return text.replace(WURZEL_FALL, wurzel) if isinstance(text, str) else text


def baue(fall, wurzel):
    """Legt den Dateibaum des Falls unter `wurzel` an (Ordner zuerst, dann Dateien und Verknuepfungen; Unlesbares zuletzt)."""
    spaeter = []
    baum = [b for b in fall["baum"] if b["pfad"].startswith(WURZEL_FALL + "/")]  # `/srv` selbst legt der Fall nicht an
    for b in baum:
        p = ersetze(b["pfad"], wurzel)
        if b["art"] == "ordner":
            os.makedirs(p, exist_ok=True)
    for b in baum:
        p = ersetze(b["pfad"], wurzel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        if b["art"] == "datei":
            with open(p, "wb") as f:
                f.write(b.get("inhalt", "").encode("latin-1"))
            os.chmod(p, int(b.get("modus", "0644"), 8))
        elif b["art"] == "verknuepfung":
            os.symlink(ersetze(b["ziel"], wurzel), p)
        elif b["art"] == "fifo":
            os.mkfifo(p)
        if b.get("lesbar") is False:
            spaeter.append(p)
    if fall.get("zielliste"):
        p = ersetze(fall["zielliste"]["pfad"], wurzel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8") as f:
            f.write(ersetze(fall["zielliste"]["inhalt"], wurzel))
    for p in sorted(spaeter, key=len, reverse=True):
        os.chmod(p, 0)
    return spaeter


def lauf(fall, wurzel, vorn=()):
    argv = [ersetze(a, wurzel) for a in fall["argv"]]
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
    env.update({k: ersetze(v, wurzel) for k, v in fall.get("umgebung", {}).items()})
    return subprocess.run([sys.executable, "-I", "-B", ATTRAPPE] + list(vorn) + argv, capture_output=True, env=env, timeout=30)


def starte_fall(fall, vorn=()):
    wurzel = tempfile.mkdtemp(prefix="sf-")
    wurzel = os.path.realpath(wurzel)
    unlesbar = []
    try:
        unlesbar = baue(fall, wurzel)
        p = lauf(fall, wurzel, vorn)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace")
    finally:
        for u in sorted(unlesbar, key=len):
            try:
                os.chmod(u, 0o700)
            except OSError:
                pass
        shutil.rmtree(wurzel, ignore_errors=True)


class Faelle(unittest.TestCase):
    def test_alle_faelle(self):
        faelle = lade()
        self.assertGreaterEqual(len(faelle), 43)
        falsch = []
        for fall in faelle:
            exit_, aus, err = starte_fall(fall)
            e = fall["erwartet"]
            if (exit_, aus, err) != (e["exit"], e["stdout"], e["stderr"]):
                falsch.append((fall["name"], (exit_, aus, err), e))
        self.assertEqual(falsch, [], "Faelle, die die Attrappe nicht besteht")


class Mutationen(unittest.TestCase):
    def test_verknuepfung_wird_gefolgt(self):
        """Mit `suche-folgt-verknuepfung` zaehlt der Verweis auf die Schluesseldatei als Fund (M-S1)."""
        fall = next(f for f in lade() if f["name"] == "nur_verknuepfung")
        exit_, aus, _ = starte_fall(fall)
        self.assertEqual(exit_, 0)
        exit_, aus, _ = starte_fall(fall, ["--fehler", "suche-folgt-verknuepfung"])
        self.assertEqual(exit_, 1, aus)

    def test_unlesbar(self):
        fall = next(f for f in lade() if f["name"] == "kein_fund")
        exit_, aus, _ = starte_fall(fall, ["--fehler", "suche-unlesbar"])
        self.assertEqual(exit_, 3, aus)


if __name__ == "__main__":
    if sys.argv[1:] == ["--liste"]:
        for f in lade():
            print(f["name"])
        sys.exit(0)
    unittest.main()
