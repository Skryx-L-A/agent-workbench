"""Prueft die Attrappe der Suche (`attrappen/modellweg.py suche`) gegen die Faelle in `rust/spec/modellweg/faelle/suche.json`
(Vertrag 8, Abschnitt 9a) und die Mutation `suche-folgt-verknuepfung`. Nur Standardbibliothek.

Faelle mit Zielliste laufen gegen die Attrappe des Halters (`attrappen/halter.py`): Ist der Halter laut Fall erreichbar, startet der
Test sie mit den Antworten des Falls an einem eigenen Socket und gibt der Suche `--attrappe-halter <socket>` (Naht aus Abschnitt 5.1);
sonst zeigt die Naht auf einen Socket, den es nicht gibt. Dazu die Faelle dieses Laeufers zur Zielliste in der Form vom 25.11.2026
(Klasse `ZiellisteHalter`): weggefallene Felder, Halter fehlt, eigene Suche des Halters mit Gegenprobe des Verweises.
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
HALTER = os.path.join(HIER, "attrappen", "halter.py")
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


class Halter:
    """Die Attrappe des Halters an einem eigenen Socket (Ordner 0700 unter einem eigenen `mkdtemp`). `konf` ist ihre Konfiguration;
    `None` startet nichts, und der Socket bleibt aus (Halter fehlt)."""

    def __init__(self, konf, fehler=None, ablage=None):
        self.ordner = os.path.realpath(tempfile.mkdtemp(prefix="sh-"))
        os.chmod(self.ordner, 0o700)
        self.sock = os.path.join(self.ordner, "halter.sock")
        self.proc = None
        if konf is None:
            return
        kp = os.path.join(self.ordner, "konfig.json")
        with open(kp, "w", encoding="utf-8") as f:
            json.dump(konf, f)
        cmd = [sys.executable, "-I", "-B", HALTER, "--socket", self.sock, "--konfig", kp] + (["--fehler", fehler] if fehler else []) \
            + (["--ablage", ablage] if ablage else [])
        self.proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if self.proc.stdout.readline() != b'{"bereit":true}\n':
            self.ende()
            raise RuntimeError("Attrappe des Halters nicht bereit")

    def ende(self):
        if self.proc is not None and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
        if self.proc is not None:
            self.proc.stdout.close()
        shutil.rmtree(self.ordner, ignore_errors=True)


def halter_fuer(fall):
    """Halter nach dem Feld `halter` des Falls: erreichbar mit den Antworten des Falls, sonst keiner."""
    h = fall.get("halter")
    return Halter({"antworten": h["antworten"]} if h and h.get("erreichbar") else None)


def starte_fall(fall, vorn=()):
    wurzel = tempfile.mkdtemp(prefix="sf-")
    wurzel = os.path.realpath(wurzel)
    unlesbar = []
    halter = None
    try:
        unlesbar = baue(fall, wurzel)
        if fall.get("zielliste"):
            halter = halter_fuer(fall)
            vorn = ["--attrappe-halter", halter.sock] + list(vorn)
        p = lauf(fall, wurzel, vorn)
        return p.returncode, p.stdout.decode("utf-8", "replace"), p.stderr.decode("utf-8", "replace")
    finally:
        if halter is not None:
            halter.ende()
        for u in sorted(unlesbar, key=len):
            try:
                os.chmod(u, 0o700)
            except OSError:
                pass
        shutil.rmtree(wurzel, ignore_errors=True)


class Faelle(unittest.TestCase):
    def test_alle_faelle(self):
        faelle = lade()
        self.assertGreaterEqual(len(faelle), 46)
        self.assertGreaterEqual(sum(1 for f in faelle if f.get("halter")), 8, "die Faelle des Halterwegs fehlen")
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


ZIEL_A = {"name": "api-a", "host": "api.beispiel.test", "port": 443, "halter_ziel": "api.beispiel.test",
          "regeln": [{"methode": "*", "pfad": "/*"}], "upgrade": "durchreichen"}
ZIEL_T = {"name": "telemetrie", "host": "telemetry.beispiel.test", "port": 443, "halter_ziel": None,
          "regeln": [{"methode": "POST", "pfad": "/telemetry"}], "upgrade": "ablehnen"}
SUCHE_LEER = {"dateien": 1, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": 0}
FEHLT = (78, "", "mw.schluessel_fehlt: Schluessel nicht lesbar\n")
UNGUELTIG = (78, "", "mw.ziele_ungueltig: Zielliste ungueltig\n")
HALTER_NAME = (78, "", "mw.ziel_halter_name: Halterziel nicht der Host\n")


class ZiellisteHalter(unittest.TestCase):
    """Zielliste in der Form vom 25.11.2026 (Vertrag 8, Abschnitt 12 und 9a S2 Punkte 3, 3a, 3b), jede Zusage mit Gegenprobe."""

    def setUp(self):
        self.wurzel = os.path.realpath(tempfile.mkdtemp(prefix="sz-"))
        self.addCleanup(shutil.rmtree, self.wurzel, True)
        self.arbeit = os.path.join(self.wurzel, "arbeit")
        os.mkdir(self.arbeit)
        with open(os.path.join(self.arbeit, "notiz.txt"), "w") as f:
            f.write("nichts\n")
        self.zl = os.path.join(self.wurzel, "zielliste.json")

    def liste(self, *ziele):
        with open(self.zl, "w", encoding="utf-8") as f:
            json.dump({"fassung": 1, "ca_zulassung": "zugelassen", "ziele": list(ziele)}, f)

    def suche(self, halter, ins=None, vorn=()):
        argv = ["--attrappe-halter", halter.sock] + list(vorn) + ["suche", "--zielliste", self.zl]
        for i in ins or [self.arbeit]:
            argv += ["--in", i]
        p = subprocess.run([sys.executable, "-I", "-B", ATTRAPPE] + argv, capture_output=True, timeout=30,
                           env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")})
        return p.returncode, p.stdout.decode(), p.stderr.decode()

    def halter(self, konf, fehler=None):
        h = Halter(konf, fehler, (konf or {}).get("ablage"))
        self.addCleanup(h.ende)
        return h

    def test_weggefallene_felder_abgewiesen(self):
        """(a) `schluessel_datei`, `kopf` und `wert_vorsatz` machen die Liste ungueltig, auch als `null` neben `halter_ziel`.
        Gegenprobe: dieselbe Liste ohne das Feld ist gueltig und gruen."""
        h = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER}})
        self.liste(ZIEL_A, ZIEL_T)
        self.assertEqual(self.suche(h), (0, '{"dateien":1,"nicht_lesbar":0,"treffer":0,"uebersprungen":0}\n', ""))
        for feld in ("schluessel_datei", "kopf", "wert_vorsatz"):
            for wert in ("/srv/x.schluessel" if feld == "schluessel_datei" else "x", None):
                with self.subTest(feld=feld, wert=wert, ziel="gebunden"):
                    self.liste(dict(ZIEL_A, **{feld: wert}), ZIEL_T)
                    self.assertEqual(self.suche(h), UNGUELTIG)
                with self.subTest(feld=feld, wert=wert, ziel="ungebunden"):
                    self.liste(ZIEL_A, dict(ZIEL_T, **{feld: wert}))
                    self.assertEqual(self.suche(h), UNGUELTIG)

    def test_alte_form_ohne_halter_ziel_abgewiesen(self):
        """Die Form bis 22.11.2026 (Schluesseldatei, Kopf und Vorsatz statt `halter_ziel`) ist ungueltig."""
        h = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER}})
        alt = {k: v for k, v in ZIEL_A.items() if k != "halter_ziel"}
        alt.update({"schluessel_datei": os.path.join(self.wurzel, "cloud.schluessel"), "kopf": "authorization", "wert_vorsatz": "Bearer "})
        self.liste(alt, ZIEL_T)
        self.assertEqual(self.suche(h), UNGUELTIG)

    def test_halter_ziel_ist_genau_der_host(self):
        """Ein `halter_ziel`, das nicht genau der `host` ist, endet mit Exit 78 `mw.ziel_halter_name` (Vertrag 8, Abschnitt 3a, Pflicht 1; Fall
        `zielliste_halter_name`), nicht mit `mw.ziele_ungueltig`. Gegenprobe: `halter_ziel` gleich dem `host` laeuft."""
        h = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER, "anderes.beispiel.test": SUCHE_LEER}})
        self.liste(dict(ZIEL_A, halter_ziel="anderes.beispiel.test"), ZIEL_T)
        self.assertEqual(self.suche(h), HALTER_NAME)
        self.liste(ZIEL_A, ZIEL_T)  # Gegenprobe
        self.assertEqual(self.suche(h)[0], 0)

    def test_stufe_12_je_ziel_in_der_reihenfolge_und_vor_den_spaeten_stufen(self):
        """Stufe 12 (Abschnitt 12): das erste Ziel mit einem Fehler entscheidet. Ein weggefallenes Feld ist `mw.ziele_ungueltig`, ein fremdes
        Halterziel `mw.ziel_halter_name`. Die Stufe 13 (`upgrade`) und die Stufe 15 (Doppel) kommen danach, die Stufen bis 11 davor."""
        h = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER}})
        fremd = dict(ZIEL_A, name="api-b", host="b.beispiel.test", halter_ziel="x.beispiel.test")
        weg = dict(ZIEL_A, kopf="x")
        folgen = [
            ("weg_dann_name", [weg, fremd], UNGUELTIG),
            ("name_dann_weg", [fremd, weg], HALTER_NAME),
            ("name_vor_upgrade", [dict(fremd, upgrade="falsch"), ZIEL_T], HALTER_NAME),
            ("name_vor_ungebundenem_upgrade", [fremd, dict(ZIEL_T, upgrade="durchreichen")], HALTER_NAME),
            ("name_vor_doppelt", [ZIEL_A, dict(fremd, name="api-c", host="api.beispiel.test", port=444, halter_ziel="x.beispiel.test")], HALTER_NAME),
            ("regelform_vor_name", [dict(fremd, regeln=[{"methode": "DELETE", "pfad": "/*"}])], UNGUELTIG),
            ("upgrade_ohne_name", [dict(ZIEL_A, upgrade="falsch"), ZIEL_T], UNGUELTIG),
        ]
        for name, ziele, erwartet in folgen:
            with self.subTest(fall=name):
                self.liste(*ziele)
                self.assertEqual(self.suche(h), erwartet)

    def test_halter_fehlt_ergibt_78(self):
        """(c) Ohne Halter: Exit 78, `mw.schluessel_fehlt`, nie gruen. Gegenprobe: mit Halter am selben Pfad Exit 0."""
        self.liste(ZIEL_A, ZIEL_T)
        ohne = self.halter(None)
        self.assertEqual(self.suche(ohne), FEHLT)
        mit = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER}})
        self.assertEqual(self.suche(mit)[0], 0)

    def test_ohne_naht_der_feste_pfad(self):
        """Ohne `--attrappe-halter` nimmt die Suche den festen Pfad `/run/user/<uid>/wb-halter/halter.sock`; liegt dort kein Halter
        dieser Attrappe (hier: kein solcher Ordner), endet sie mit 78."""
        if os.path.exists("/run/user/%d/wb-halter/halter.sock" % os.getuid()):
            self.skipTest("an diesem Rechner laeuft ein Halter am festen Pfad")
        self.liste(ZIEL_A, ZIEL_T)
        p = subprocess.run([sys.executable, "-I", "-B", ATTRAPPE, "suche", "--zielliste", self.zl, "--in", self.arbeit],
                           capture_output=True, timeout=30, env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")})
        self.assertEqual((p.returncode, p.stdout.decode(), p.stderr.decode()), FEHLT)

    def test_halter_unecht_ergibt_78(self):
        """Socket-Ordner nicht 0700: die Gegenseite gilt als unecht. Gegenprobe: mit 0700 Exit 0."""
        self.liste(ZIEL_A, ZIEL_T)
        h = self.halter({"antworten": {"api.beispiel.test": SUCHE_LEER}})
        os.chmod(h.ordner, 0o755)
        self.assertEqual(self.suche(h), FEHLT)
        os.chmod(h.ordner, 0o700)
        self.assertEqual(self.suche(h)[0], 0)

    def ablage(self, wert="wert-im-halter-7f3a"):
        ablage = os.path.join(self.wurzel, "halter", "geheim")
        os.makedirs(ablage, mode=0o700)
        fd = os.open(os.path.join(ablage, "anmeldung.copilot"), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as f:
            f.write(wert)
        return ablage, {"ablage": ablage, "ziele": {"api.beispiel.test": {"schluessel": "anmeldung.copilot"}}}  # `ablage` geht als Option

    def test_weiterreicher_sucht_nicht_selbst(self):
        """S2 Punkt 3a: Die Zeile ist die Summe der Antworten des Halters; die Suche zaehlt nichts selbst. Die Antwort des Halters nennt
        Zahlen, die die eigenen Wurzeln nicht ergeben koennen (eine Datei, aber 7 Dateien gemeldet)."""
        self.liste(ZIEL_A, ZIEL_T)
        h = self.halter({"antworten": {"api.beispiel.test": {"dateien": 7, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": 2}}})
        self.assertEqual(self.suche(h), (0, '{"dateien":7,"nicht_lesbar":0,"treffer":0,"uebersprungen":2}\n', ""))

    def test_halter_sucht_den_wert_aus_der_ablage(self):
        """Die Attrappe des Halters sucht ihr Geheimnis selbst: ein Fund ist rot; Gegenprobe ohne Fund gruen."""
        _, konf = self.ablage()
        self.liste(ZIEL_A, ZIEL_T)
        h = self.halter(konf)
        self.assertEqual(self.suche(h)[0], 0)
        with open(os.path.join(self.arbeit, "leck.txt"), "w") as f:
            f.write("vorn wert-im-halter-7f3a hinten\n")
        self.assertEqual(self.suche(h), (1, '{"dateien":2,"nicht_lesbar":0,"treffer":1,"uebersprungen":0}\n', ""))

    def test_verweis_auf_die_ablage_zaehlt_nicht(self):
        """(b) S2a: Ein Verweis auf die Datei des Geheimnisses in der Ablage des Halters zaehlt nicht als Fund (0 Treffer, 0 Dateien,
        1 uebersprungen). Gegenprobe: mit `suche-folgt-verknuepfung` im Halter ist derselbe Verweis ein Fund (Exit 1)."""
        ablage, konf = self.ablage()
        self.liste(ZIEL_A, ZIEL_T)
        vo = os.path.join(self.wurzel, "verweis")
        os.mkdir(vo, 0o700)
        os.symlink(os.path.join(ablage, "anmeldung.copilot"), os.path.join(vo, "verweis"))
        h = self.halter(konf)
        self.assertEqual(self.suche(h, [vo]), (0, '{"dateien":0,"nicht_lesbar":0,"treffer":0,"uebersprungen":1}\n', ""))
        m = self.halter(konf, "suche-folgt-verknuepfung")
        self.assertEqual(self.suche(m, [vo])[0], 1)

    def test_kein_geheimnis_im_halter(self):
        """Ein Ziel ohne Datei in der Ablage: `halter.kein_schluessel`, die Suche endet mit 78."""
        ablage, konf = self.ablage()
        os.unlink(os.path.join(ablage, "anmeldung.copilot"))
        self.liste(ZIEL_A, ZIEL_T)
        self.assertEqual(self.suche(self.halter(konf)), FEHLT)

    def test_mutation_der_suche_wirkt_im_halter(self):
        """`suche-unlesbar` im Halter: Exit 3. Dieselbe Mutation an der Suche des Weiterreichers aendert auf der Cloud-Route nichts,
        denn er sucht dort nicht selbst."""
        _, konf = self.ablage()
        self.liste(ZIEL_A, ZIEL_T)
        self.assertEqual(self.suche(self.halter(konf, "suche-unlesbar"))[0], 3)
        self.assertEqual(self.suche(self.halter(konf), vorn=["--fehler", "suche-unlesbar"])[0], 0)


if __name__ == "__main__":
    if sys.argv[1:] == ["--liste"]:
        for f in lade():
            print(f["name"])
        sys.exit(0)
    unittest.main()
