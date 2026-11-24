"""Positivkontrollen des Laeufers (Entscheidung vom 21.11.2026): Zwei Pruefungen duerfen nicht gruen werden, wenn ihr
Messweg blind ist.

1. E4-cloud: Dieselbe Umgebungssuche, die das Token zaehlt, zaehlt den PLATZHALTER der Konfiguration (`copilot.platzhalter`), der in der
   Umgebung der CLI steht. Erwartet >= 1. Bei 0 (oder keiner Zahl) ist die Suche blind: fester Code `e4-cloud.kontrolle_blind`, nie gruen.
2. S1: Waehrend die Cloud-Sitzung laeuft, hat der Kern mindestens ein direktes Kind (Gegenprobe ohne Diagnose). Erwartet >= 1. Bei 0 ist der
   Messweg blind: fester Code `s1.kontrolle_blind`, nie gruen.

Je Kontrolle: sieht und blind, im vollen und im knappen Lauf (E4-cloud) und im Lauf mit und ohne Diagnose (S1). Im blinden Fall laesst der Test
den Messweg absichtlich nichts sehen (Fehler der Attrappen `e4-cloud-env-blind`, Prozessliste ohne Kind) und erwartet den festen Code.
3. E4-cloud, zweiter Weg (Entscheidung vom 21.11.2026): `token_proc` liest die Startumgebung der Prozesse der Sitzung (Fehler nach
   /dev/null). Dieselbe Suche zaehlt den PLATZHALTER (`kontrolle_proc`, Messwert im Bericht). Erwartet >= 1, sonst fester Code
   `e4-cloud.kontrolle_blind`. Im knappen Lauf ist sie eine weitere Zeile im einen Buendel (jetzt 11 Zeilen), keine Eingabe.
4. E4-lokal: Die Umgebungssuche zaehlt den NAMEN `PI_OFFLINE` (nie einen Wert), den das Programm im Harness `pi` immer setzt
   (Vertrag 6, `rust/spec/VERTRAEGE.md`). Erwartet >= 1 (`kontrolle_lokal_env`), sonst fester Code `e4-lokal.kontrolle_blind`.
Die Gegenprobe mit dem alten Laeufer: dieselben Tests muessen dort rot werden.

Nur Standardbibliothek. Aufruf: python3 -I -B test_positivkontrollen.py   (startet die Attrappen, dauert einige Minuten: ueber die Tafel)
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import konfig, messen, probelauf, schritte  # noqa: E402
import test_einstart  # noqa: E402


def lade_konfig(name="konfig.attrappe.json"):
    with open(os.path.join(HIER, name), encoding="utf-8") as f:
        return json.load(f)


def schreibe(pfad, k):
    with open(pfad, "w", encoding="utf-8") as f:
        json.dump(k, f)
    return pfad


def kern_proc(wurzel, mit_kind):
    """Prozessliste des Kerns (4242), mit oder ohne direktes Kind (4300); nur `stat`, wie der Laeufer liest."""
    for pid, ppid, name in ((4242, 1, "werkbank-kern"), (4300, 4242, "agent")) if mit_kind else ((4242, 1, "werkbank-kern"),):
        os.makedirs(os.path.join(wurzel, str(pid)), exist_ok=True)
        with open(os.path.join(wurzel, str(pid), "stat"), "w") as f:
            f.write("%d (%s) S %d 1 1 0 -1 4194304 100 0 0 0 1 1 0 0 20 0 1 0 12345 1000000 100\n" % (pid, name, ppid))


class Werkstatt(unittest.TestCase):
    """Ein Wegwerf-Ordner je Klasse, Aufruf von drehbuch.py gegen die Attrappen (voll und `probelauf --nur-cloud`)."""

    @classmethod
    def setUpClass(cls):
        cls.ordner = tempfile.mkdtemp(prefix="poskontr-")
        cls.stub = os.path.join(cls.ordner, "bin")
        os.makedirs(cls.stub)
        pfad = os.path.join(cls.stub, "systemctl")
        with open(pfad, "w") as f:
            f.write("#!/bin/sh\n[ \"$1 $2 $3 $4 $5\" = \"--user show -p MainPID --value\" ] || exit 2\n"
                    "[ \"$6\" = werkbank-kern ] || exit 3\necho 4242\n")
        os.chmod(pfad, 0o755)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.ordner, ignore_errors=True)

    def konfig_ohne_diagnose(self, name, mit_kind, knapp=False):
        """Konfiguration ohne Diagnose der Bruecke: der Laeufer misst Sitzungsliste und Kinder des Kerns aus der Prozessliste."""
        k = lade_konfig()
        k["bruecke"]["diagnose_pfad"] = None
        k["bruecke"]["proc_wurzel"] = os.path.join(self.ordner, "proc-" + name)
        k["kern_einheit"] = "werkbank-kern"
        if knapp:
            k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        kern_proc(k["bruecke"]["proc_wurzel"], mit_kind)
        return schreibe(os.path.join(self.ordner, "konfig-%s.json" % name), k)

    def umgebung(self):
        return dict(os.environ, PATH=self.stub + os.pathsep + os.environ.get("PATH", ""))

    def bericht(self, name, konfig_pfad=None, fehler="", nur="", vault=""):
        """Voller Lauf gegen die Attrappen: der JSON-Bericht (Schritt -> Eintrag)."""
        js = os.path.join(self.ordner, "bericht-%s.json" % name)
        cmd = [sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "attrappe", "--json", js]
        for opt, wert in (("--konfig", konfig_pfad), ("--fehler", fehler), ("--nur", nur), ("--vault", vault)):
            if wert:
                cmd += [opt, wert]
        p = subprocess.run(cmd, capture_output=True, timeout=300, env=self.umgebung())
        with open(js, encoding="utf-8") as f:
            b = json.load(f)
        return p.returncode, {s["schritt"]: s for s in b["schritte"]}

    def knapp(self, name, konfig_pfad=None, fehler="", nur_cloud=True):
        """`probelauf --nur-cloud` (zwei Cloud-Eingaben; ohne `nur_cloud` auch die lokalen Schritte): (Exit, Ergebnisdatei, Konsole)."""
        if konfig_pfad is None:
            k = lade_konfig()
            k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
            konfig_pfad = schreibe(os.path.join(self.ordner, "konfig-knapp-%s.json" % name), k)
        erg = os.path.join(self.ordner, "ergebnis-%s.json" % name)
        cmd = [sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf"] + (["--nur-cloud"] if nur_cloud else []) \
            + ["--konfig", konfig_pfad, "--ergebnis", erg]
        if fehler:
            cmd += ["--fehler", fehler]
        p = subprocess.run(cmd, capture_output=True, timeout=300, env=self.umgebung())
        with open(erg, encoding="utf-8") as f:
            d = json.load(f)
        return p.returncode, {s["schritt"]: s for s in d["schritte"]}, d, p.stdout.decode("utf-8", "replace")


class E4CloudKontrolle(Werkstatt):
    def test_voll_sieht_gruen_und_zaehlt_den_platzhalter(self):
        _, b = self.bericht("e4-voll-sieht", nur="E4-cloud")
        e = b["E4-cloud"]
        self.assertEqual(e["ergebnis"], "gruen")
        self.assertGreaterEqual(e["messwerte"]["kontrolle_platzhalter"], 1)
        self.assertIsNone(e["code_offen"])

    def test_voll_blind_fester_code_nie_gruen(self):
        _, b = self.bericht("e4-voll-blind", fehler="e4-cloud-env-blind", nur="E4-cloud")
        e = b["E4-cloud"]
        self.assertEqual(e["ergebnis"], "offen")
        self.assertEqual(e["code_offen"], "e4-cloud.kontrolle_blind")
        self.assertEqual(e["messwerte"]["kontrolle_platzhalter"], 0)
        self.assertNotIn("Kein Treffer des Tokens ausser dem Platzhalter: Umgebung, Argumente, Prozesse, Dateien, gh, secret-tool",
                         [t["name"] for t in e["teile"]], "ein blinder Weg behauptet keinen Teil als gruen")
        self.assertEqual(b["S1"]["ergebnis"], "gruen", "nur der Schritt mit dem blinden Messweg wird nicht gruen")

    def test_voll_token_statt_platzhalter_bleibt_rot(self):
        # In der Umgebung steht das echte Token statt des Platzhalters: die Kontrolle sieht 0, und der Treffer des Tokens macht den Schritt rot.
        _, b = self.bericht("e4-voll-token", fehler="e4-cloud-token-lesbar", nur="E4-cloud")
        self.assertEqual(b["E4-cloud"]["ergebnis"], "rot")

    def test_knapp_sieht_nichts_aendert_sich(self):
        _, s, d, text = self.knapp("e4-knapp-sieht")
        self.assertEqual(d["cloud_eingaben"], 2, "die Kontrolle ist eine Zeile im Buendel, keine dritte Eingabe")
        # gegen die Attrappen steht `ziel_url`, also ist der Code `e4-cloud.nur_attrappe` (im echten Lauf `e4-cloud.offen` mit drei Gruenden)
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.nur_attrappe")
        self.assertNotIn("e4-cloud.kontrolle_blind", s["E4-cloud"]["offen"], "die Kontrolle sieht, also kein Code von ihr")
        self.assertTrue(any(z.startswith("  E4-cloud teile: ") and z.endswith(", 0 rot") for z in text.splitlines()), "kein roter Teil")

    def test_knapp_blind_fester_code_nie_gruen(self):
        rc, s, d, text = self.knapp("e4-knapp-blind", fehler="e4-cloud-env-blind")
        self.assertEqual(d["cloud_eingaben"], 2)
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.kontrolle_blind")
        self.assertFalse(s["E4-cloud"]["bestanden"])
        self.assertIn("e4-cloud.kontrolle_blind", s["E4-cloud"]["offen"])
        self.assertEqual(probelauf.pruefe_form(d), [], "die Datei bleibt Fassung 2 und geschlossen")
        self.assertNotEqual(rc, 0)
        self.assertIsNone(s["S1"]["fehlercode"], "nur der Schritt mit dem blinden Messweg bekommt den Code")


class E4CloudKontrolleProc(Werkstatt):
    """`kontrolle_proc`: der Weg von `token_proc` zaehlt den Platzhalter in den Prozessen der Sitzung."""
    TEIL = "Kein Treffer des Tokens ausser dem Platzhalter: Umgebung, Argumente, Prozesse, Dateien, gh, secret-tool"

    def test_voll_sieht_gruen_und_zaehlt_den_platzhalter_in_den_prozessen(self):
        _, b = self.bericht("proc-voll-sieht", nur="E4-cloud")
        e = b["E4-cloud"]
        self.assertEqual(e["ergebnis"], "gruen")
        self.assertGreaterEqual(e["messwerte"]["kontrolle_proc"], 1)
        self.assertIsNone(e["code_offen"])

    def test_voll_blind_fester_code_nie_gruen(self):
        _, b = self.bericht("proc-voll-blind", fehler="e4-cloud-proc-blind", nur="E4-cloud")
        e = b["E4-cloud"]
        self.assertEqual(e["ergebnis"], "offen")
        self.assertEqual(e["code_offen"], "e4-cloud.kontrolle_blind")
        self.assertEqual(e["messwerte"]["kontrolle_proc"], 0)
        self.assertGreaterEqual(e["messwerte"]["kontrolle_platzhalter"], 1, "die Umgebungssuche sieht, nur der Weg ueber die Prozesse ist blind")
        self.assertTrue(any("token_proc" in o for o in e["offen"]), "der Text nennt token_proc")
        self.assertNotIn(self.TEIL, [t["name"] for t in e["teile"]], "ein blinder Weg behauptet keinen Teil als gruen")

    def test_voll_blind_und_treffer_des_tokens_bleibt_rot(self):
        _, b = self.bericht("proc-voll-token", fehler="e4-cloud-proc-blind,e4-cloud-token-lesbar", nur="E4-cloud")
        self.assertEqual(b["E4-cloud"]["ergebnis"], "rot")

    def test_knapp_sieht_nichts_aendert_sich(self):
        _, s, d, text = self.knapp("proc-knapp-sieht")
        self.assertEqual(d["cloud_eingaben"], 2, "die Kontrolle ist eine Zeile im Buendel, keine dritte Eingabe")
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.nur_attrappe")
        self.assertNotIn("e4-cloud.kontrolle_blind", s["E4-cloud"]["offen"])
        self.assertTrue(any(z.startswith("  E4-cloud teile: ") and z.endswith(", 0 rot") for z in text.splitlines()), "kein roter Teil")
        self.assertEqual(d["fassung"], 2)
        self.assertEqual(probelauf.pruefe_form(d), [])

    def test_knapp_blind_fester_code_nie_gruen(self):
        rc, s, d, _ = self.knapp("proc-knapp-blind", fehler="e4-cloud-proc-blind")
        self.assertEqual(d["cloud_eingaben"], 2)
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.kontrolle_blind")
        self.assertFalse(s["E4-cloud"]["bestanden"])
        self.assertIn("e4-cloud.kontrolle_blind", s["E4-cloud"]["offen"])
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.assertNotEqual(rc, 0)
        self.assertIsNone(s["S1"]["fehlercode"], "die Zeile steht im Buendel von S1, aber nur E4-cloud bekommt den Code")


class E4LokalKontrolle(Werkstatt):
    """`kontrolle_lokal_env`: der Name `PI_OFFLINE` steht in der Umgebung der lokalen Sitzung (Harness pi)."""
    TEIL = "Schluessel nicht in Umgebung, Argumenten und Dateien des Einschlusses"

    def test_voll_sieht_gruen_ohne_neuen_teil(self):
        _, b = self.bericht("lokal-voll-sieht", nur="E4-lokal")
        e = b["E4-lokal"]
        self.assertGreaterEqual(e["messwerte"]["kontrolle_lokal_env"], 1)
        self.assertIsNone(e["code_offen"])
        self.assertIn(self.TEIL, [t["name"] for t in e["teile"]])
        self.assertEqual([t["name"] for t in e["teile"] if not t["ok"]], [], "kein roter Teil, kein Teil dazu")

    def test_voll_blind_fester_code_nie_gruen(self):
        _, b = self.bericht("lokal-voll-blind", fehler="e4-lokal-env-blind", nur="E4-lokal")
        e = b["E4-lokal"]
        self.assertEqual(e["ergebnis"], "offen")
        self.assertEqual(e["code_offen"], "e4-lokal.kontrolle_blind")
        self.assertEqual(e["messwerte"]["kontrolle_lokal_env"], 0)
        self.assertNotIn(self.TEIL, [t["name"] for t in e["teile"]], "ein blinder Weg behauptet keinen Teil als gruen")

    def test_voll_blind_und_schluessel_in_umgebung_bleibt_rot(self):
        _, b = self.bericht("lokal-voll-treffer", fehler="e4-lokal-env-blind,e4-schluessel-in-umgebung", nur="E4-lokal")
        self.assertEqual(b["E4-lokal"]["ergebnis"], "rot")

    def test_knapp_cloud_lauf_mit_lokalen_schritten(self):
        # `cloud_eingaben_max` 2 und die lokalen Schritte laufen voll (wie im Lauf der lokalen Route): sieht und blind
        k = lade_konfig()
        k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        pfad = schreibe(os.path.join(self.ordner, "konfig-knapp-lokal.json"), k)
        _, s, d, _ = self.knapp("lokal-knapp-sieht", pfad, nur_cloud=False)
        self.assertNotIn("e4-lokal.kontrolle_blind", s["E4-lokal"]["offen"])
        self.assertIsNone(s["E4-lokal"]["fehlercode"])
        self.assertEqual(d["cloud_eingaben"], 2)
        _, s, d, _ = self.knapp("lokal-knapp-blind", pfad, fehler="e4-lokal-env-blind", nur_cloud=False)
        self.assertEqual(s["E4-lokal"]["fehlercode"], "e4-lokal.kontrolle_blind")
        self.assertFalse(s["E4-lokal"]["bestanden"])
        self.assertEqual(probelauf.pruefe_form(d), [])


class S1Kontrolle(Werkstatt):
    def test_voll_ohne_diagnose_kern_mit_kind_gruen(self):
        k = self.konfig_ohne_diagnose("s1-voll-mit", True)
        _, b = self.bericht("s1-voll-mit", k, nur="S1")
        s1 = b["S1"]
        self.assertEqual(s1["ergebnis"], "gruen")
        self.assertEqual(s1["gegenprobe_messwerte"]["kontrolle_kinder"], 1)
        self.assertEqual(s1["gegenprobe_messwerte"]["gegenprobe_weg"], "sitzungen_und_kinder")

    def test_voll_ohne_diagnose_kern_ohne_kind_fester_code(self):
        k = self.konfig_ohne_diagnose("s1-voll-ohne", False)
        _, b = self.bericht("s1-voll-ohne", k, fehler="s1-kern-ohne-kind", nur="S1")
        s1 = b["S1"]
        self.assertEqual(s1["ergebnis"], "offen")
        self.assertEqual(s1["code_offen"], "s1.kontrolle_blind")
        self.assertEqual(s1["gegenprobe_messwerte"]["kontrolle_kinder"], 0)
        self.assertNotIn("Keine neuen direkten Kinder des Kerns fuer die abgelehnten Starts", [t["name"] for t in s1["teile"]])

    def test_knapp_ohne_diagnose_mit_und_ohne_kind(self):
        _, s, d, _ = self.knapp("s1-knapp-mit", self.konfig_ohne_diagnose("s1-knapp-mit", True, knapp=True))
        self.assertIsNone(s["S1"]["fehlercode"])
        self.assertTrue(s["S1"]["bestanden"])
        self.assertEqual(d["s1_gegenprobe"]["weg"], "sitzungen_und_kinder")
        _, s, d, _ = self.knapp("s1-knapp-ohne", self.konfig_ohne_diagnose("s1-knapp-ohne", False, knapp=True), fehler="s1-kern-ohne-kind")
        self.assertEqual(s["S1"]["fehlercode"], "s1.kontrolle_blind")
        self.assertFalse(s["S1"]["bestanden"])
        self.assertEqual(d["cloud_eingaben"], 2)
        self.assertEqual(probelauf.pruefe_form(d), [])


class S1KontrolleAuswertung(unittest.TestCase):
    """Die Auswertung allein, ohne Attrappen: `_gegenprobe_ohne_diagnose` mit festen Fenstern."""

    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="poskontr-aus-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)

    def auswerten(self, vorher, nachher):
        L, _ = test_einstart.neuer_lauf(self.arbeit)
        s = schritte.Schritt("S1", "x")
        with mock.patch.object(schritte, "_kinder_fenster", return_value=nachher):
            schritte._gegenprobe_ohne_diagnose(L, s, vorher)
        return s

    def test_kind_gesehen_gruen(self):
        s = self.auswerten(({"s-1"}, 1), ({"s-1"}, 1))
        self.assertEqual(s.ergebnis(), "gruen")
        self.assertIsNone(s.code_offen)
        self.assertEqual(s.gegenwerte["kontrolle_kinder"], 1)

    def test_kein_kind_gesehen_blind_nie_gruen(self):
        s = self.auswerten(({"s-1"}, 0), ({"s-1"}, 0))
        self.assertEqual(s.ergebnis(), "offen")
        self.assertEqual(s.code_offen, "s1.kontrolle_blind")
        self.assertEqual(probelauf.fehlercode(s.als_dict()), "s1.kontrolle_blind")
        self.assertIn("s1.kontrolle_blind", probelauf.FEHLERCODES)
        self.assertEqual(probelauf.offen_codes(s.als_dict()), ["s1.kontrolle_blind"])
        self.assertEqual([n for _, n, _ in s.teile], ["Keine neue Sitzung in der Sitzungsliste fuer die abgelehnten Starts"],
                         "der Kinder-Teil entfaellt, er sagt bei einem blinden Weg nichts")

    def test_blind_und_neues_kind_bleibt_rot(self):
        s = self.auswerten(({"s-1"}, 0), ({"s-1"}, 1))
        self.assertEqual(s.ergebnis(), "rot")

    def test_nicht_messbar_bleibt_der_alte_offen_grund(self):
        s = self.auswerten(({"s-1"}, None), ({"s-1"}, None))
        self.assertEqual(s.ergebnis(), "offen")
        self.assertIsNone(s.code_offen)

    def test_messweg_ueber_die_prozessliste_zaehlt_kind_und_keins(self):
        proc = os.path.join(self.arbeit, "proc")
        kern_proc(proc, True)
        self.assertEqual(messen.kinder_zahl(proc, 4242), 1)
        shutil.rmtree(proc)
        kern_proc(proc, False)
        self.assertEqual(messen.kinder_zahl(proc, 4242), 0, "der blinde Fall: lesbar, aber kein Kind")


class E4CloudKontrolleBuendel(unittest.TestCase):
    """Die Zeile der Kontrolle im Buendel: eine Zeile, kein Tokenpraefix, die Shell findet den Platzhalter und sonst nichts."""

    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="poskontr-buendel-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)
        self.L, _ = test_einstart.neuer_lauf(self.arbeit, cloud_eingaben_max=2)
        self.ph = self.L.k["copilot"]["platzhalter"]

    def test_zeile_im_buendel_ohne_tokenpraefix(self):
        cmd = schritte.cloud_buendel(self.L)
        self.assertIn('echo "%s=$z RC=$?"' % schritte.BUENDEL_KONTROLLE, cmd)
        self.assertNotIn("gho_", cmd.lower())

    def test_vorlage_in_beiden_konfigurationen_mit_platzhalter_in_einfachen_anfuehrungszeichen(self):
        for datei in ("konfig.attrappe.json", "konfig.probelauf.json"):
            v = lade_konfig(datei)["befehle"][schritte.KONTROLLE_ENV]
            self.assertEqual(v.count("{platzhalter}"), 1, datei)
            self.assertIn("'{platzhalter}'", v, datei)
            self.assertNotIn("{muster}", v, "die Kontrolle sucht nur den Platzhalter, nie ein Token")

    def test_shell_zaehlt_den_platzhalter_in_einer_umgebung_und_null_ohne(self):
        vorlage = lade_konfig()["befehle"][schritte.KONTROLLE_ENV]
        cmd = schritte._fmt(vorlage, platzhalter=schritte.ohne_tokenpraefix(self.ph))
        mit = subprocess.run(["/bin/sh", "-c", cmd], capture_output=True, text=True, timeout=10,
                             env={"PATH": "/usr/bin:/bin", "COPILOT_GITHUB_TOKEN": self.ph})
        ohne = subprocess.run(["/bin/sh", "-c", cmd], capture_output=True, text=True, timeout=10, env={"PATH": "/usr/bin:/bin"})
        self.assertEqual(mit.stdout.strip(), "1")
        self.assertEqual(ohne.stdout.strip(), "0", "ohne Platzhalter in der Umgebung zaehlt der Weg 0: das ist der blinde Fall")

    def test_codes_stehen_in_der_festen_liste(self):
        self.assertIn("e4-cloud.kontrolle_blind", probelauf.FEHLERCODES)
        self.assertIn("s1.kontrolle_blind", probelauf.FEHLERCODES)
        self.assertIn("e4-lokal.kontrolle_blind", probelauf.FEHLERCODES)

    def test_buendel_traegt_die_proc_zeile_ohne_tokenpraefix_und_erwartet_elf_zeilen(self):
        cmd = schritte.cloud_buendel(self.L)
        self.assertIn('echo "%s=$z RC=$?"' % schritte.BUENDEL_KONTROLLE_PROC, cmd)
        self.assertNotIn("gho_", cmd.lower())
        self.assertEqual(cmd.count('echo "'), 5 + len(schritte.TOKENPRUEFUNGEN), "elf Zeilen: drei von S1, sechs Tokenpruefungen, zwei Kontrollen")

    def test_proc_vorlage_in_beiden_konfigurationen(self):
        for datei in ("konfig.attrappe.json", "konfig.probelauf.json"):
            b = lade_konfig(datei)["befehle"]
            v = b[schritte.KONTROLLE_PROC]
            self.assertIn("2>/dev/null", v, datei)
            self.assertIn("'{platzhalter}'", v, datei)
            self.assertNotIn("{muster}", v, "die Kontrolle sucht nur den Platzhalter, nie ein Token")
            self.assertNotIn("cmdline", v, "wie token_proc die Startumgebung, aber ohne Argumente: die Kontrolle prueft den Weg der Umgebung")
            self.assertEqual(b["kontrolle_lokal_env"], "env | grep -c '^PI_OFFLINE='", "nur der Name, nie ein Wert")

    def test_shell_zaehlt_den_platzhalter_in_den_prozessen_und_null_ohne(self):
        # Mit echtem /proc der eigenen Sitzung: ein Kindprozess mit dem Platzhalter in der Startumgebung wird gezaehlt, ohne ihn nicht.
        vorlage = lade_konfig()["befehle"][schritte.KONTROLLE_PROC]
        cmd = schritte._fmt(vorlage, platzhalter="gh''o_" + "0" * 36)
        if not os.path.isdir("/proc/self"):
            self.skipTest("kein /proc (nur auf dem Linux-Baurechner aussagekraeftig)")
        kind = subprocess.Popen(["/bin/sh", "-c", "sleep 20"], env={"PATH": "/usr/bin:/bin", "COPILOT_GITHUB_TOKEN": self.ph})
        try:
            out = subprocess.run(["/bin/sh", "-c", cmd], capture_output=True, text=True, timeout=10)
            self.assertGreaterEqual(int(out.stdout.strip() or 0), 1)
        finally:
            kind.kill()
            kind.wait()


class S2FehlerzeileUmbruch(Werkstatt):
    """Zweiter Weg, Nachtrag 4 (21.11.): S2 darf nicht vom Pfad des Baums abhaengen. Die erste Fehlerzeile der Datei-Probe wird im
    120-spaltigen Terminal umbrochen; ein Umbruch auf ein Leerzeichen liess `logische_zeilen` die Teile als zwei Zeilen im Verlauf stehen."""

    def test_vergleich_ohne_leerraum_und_zeilenwechsel(self):
        zeile = "cat: /x/y/MK-7Q[2]-liefertermin-presslinie.md: No such file or directory"
        f = schritte.fehlerzeile_im_verlauf
        self.assertTrue(f(zeile, "  \u23bf " + zeile + "\n    RC=1"), "unverbrochen")
        self.assertTrue(f(zeile, "  \u23bf cat: /x/y/MK-7Q[2]-liefertermin-presslinie.md: No\nsuch file or directory\n    RC=1"),
                        "Umbruch auf ein Leerzeichen: das Leerzeichen fehlt, ein Zeilenwechsel steht dort")
        self.assertTrue(f(zeile, "  \u23bf cat: /x/y/MK-7Q[2]-liefertermin-pre\nsslinie.md: No such file or directory"), "Umbruch im Wort")
        self.assertFalse(f(zeile, "  \u23bf cat: /x/y/MK-7Q[2]-liefertermin-presslinie.md: No such file\n    RC=1"), "der Rest der Zeile fehlt")
        self.assertFalse(f("", "irgendein Text"))

    def test_s2_gruen_bei_jeder_pfadlaenge_der_fehlerzeile(self):
        # Die Fehlerzeile `cat: <Vault>/<rel>: No such file or directory` ist 138 bis 143 Zeichen lang: der Umbruch faellt auf ein Leerzeichen,
        # ins Wort oder auf die Spaltengrenze. Mit dem alten Vergleich war S2 bei 138 und 141 rot (gemessen); jetzt ist er bei jeder Laenge gruen.
        k = lade_konfig()
        rel = k["notiz_nk"]["rel"]
        quelle = os.path.realpath(os.path.join(HIER, "..", "..", "such-socket", "vorfuehr-vault"))
        for laenge in range(138, 144):
            basis = tempfile.mkdtemp(prefix="s2l-", dir=self.ordner)
            fest = len(basis) + len("/") + len("/") + len(rel)  # <basis>/<pad>/<rel>
            pad = laenge - 5 - len(": No such file or directory") - fest
            self.assertGreater(pad, 0)
            vault = os.path.join(basis, "v" * pad)
            shutil.copytree(quelle, vault)
            _, b = self.bericht("s2-%d" % laenge, nur="S2", vault=vault)
            teile = {t["name"]: t["ok"] for t in b["S2"]["teile"]}
            self.assertTrue(teile.get("Fehlertext steht im Terminal"), "Zeilenlaenge %d: %s" % (laenge, teile))
            self.assertEqual(b["S2"]["ergebnis"], "gruen", "Zeilenlaenge %d" % laenge)
            shutil.rmtree(basis, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
