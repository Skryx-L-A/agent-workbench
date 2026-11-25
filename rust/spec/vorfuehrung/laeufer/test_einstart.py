"""Tests fuer den knappen Cloud-Lauf: ein Start, hoechstens zwei Eingaben an die Cloud-Sitzung, der Messbetrieb `--nur-cloud`, die wirksame
Markerliste und die Markerdatei fuer den Kern (mit den Nachtraegen 1 bis 5, 21.11.).

Nur Standardbibliothek. Aufruf: python3 -I -B test_einstart.py   (die Klasse `NurCloudLauf` startet die Attrappen und dauert etwa 15 s: ueber die Tafel)
"""
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, konfig, lauf, messen, probelauf, schritte  # noqa: E402
import test_dienste  # noqa: E402

KERN, BRUECKE = test_dienste.KERN, test_dienste.BRUECKE


class FalscheBruecke:
    """Nur die Aufrufe, die `Lauf.starten` und `Lauf.zug` brauchen. Schreibt jeden Aufruf mit; jedes Ende-Token steht schon im Verlauf."""

    def __init__(self):
        self.starts, self.eingaben = [], []

    def starten(self, start, ohne_token=False):
        self.starts.append((start, ohne_token))
        return 200, {"sitzung": "s-%d" % len(self.starts)}, 0.0

    def diagnose(self):
        return None

    def verlauf(self, sid, ab=0):
        if ab >= 10 ** 9:  # `Lauf.zug` fragt so nach dem Stand vor der Eingabe
            return 0, ""
        text = " ".join("FERTIG T%d" % n for n in range(1, 30))
        return len(text), text[ab:]

    def ablehnungen_marke(self):
        return 0

    def ablehnungen_seit(self, marke):
        return []

    def eingabe(self, sid, text):
        self.eingaben.append((sid, text))
        return 200, None

    def zumachen(self):
        pass


def neuer_lauf(arbeit, **probelauf_felder):
    k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
    k.setdefault("probelauf", {}).update(probelauf_felder)
    for a in ("cloud", "lokal"):
        k["modell"][a]["zaehlen"] = False  # kein Mitschnitt noetig
    b = FalscheBruecke()
    L = lauf.Lauf(k, b, {"cloud": None, "lokal": None}, None, arbeit)
    return L, b


class EinStart(unittest.TestCase):
    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="einstart-")

    def tearDown(self):
        shutil.rmtree(self.arbeit, ignore_errors=True)

    def test_zweiter_cloud_start_wird_abgewiesen_bevor_die_bruecke_ihn_sieht(self):
        L, b = neuer_lauf(self.arbeit, max_starts=1)
        code, _, _ = L.starten(L.start_dict("cloud"))
        self.assertEqual((code, L.starts, len(b.starts)), (200, 1, 1))
        with self.assertRaises(lauf.StartLimit) as e:
            L.starten(L.start_dict("cloud"))
        self.assertEqual(str(e.exception), "copilot.start-limit")
        self.assertEqual(len(b.starts), 1, "die Bruecke hat den zweiten Start nie gesehen")

    def test_ablehnungstest_geht_an_der_grenze_hinaus_und_zaehlt_nur_bei_annahme(self):
        L, b = neuer_lauf(self.arbeit, max_starts=1)
        L.starten(L.start_dict("cloud"))
        L.starten(dict(L.start_dict("cloud"), route="gibt-es-nicht"), ablehnung=True)
        self.assertEqual(len(b.starts), 2)
        self.assertEqual(L.starts, 2, "von der Bruecke angenommen: zaehlt als Start")

    def test_grenze_nur_in_der_konfiguration_des_probelaufs(self):
        with open(os.path.join(HIER, "konfig.probelauf.json"), encoding="utf-8") as f:
            pl = json.load(f)["probelauf"]
        self.assertEqual((pl["max_starts"], pl["cloud_eingaben_max"]), (1, 2))
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            self.assertNotIn("max_starts", json.load(f).get("probelauf", {}), "gegen die Attrappen bleibt die Abdeckung voll (Nachtrag 6)")


class S3Gegenprobe(unittest.TestCase):
    """Nachtrag 6: Mit `konfig.attrappe.json` (keine Grenze) startet S3 eine zweite Cloud-Sitzung und prueft die Zeile `kundendaten-an-cloud`
    (D11); mit `max_starts` 1 startet nur die erste, und D11 ist `offen` mit `s3.nur_attrappe`. Die Pruefung in der ersten Sitzung laeuft immer."""

    D11 = "Gegenprobe: Protokollzeile von k-nord mit Grund kundendaten-an-cloud"
    ERSTE = "Gegenprobe: eine Aenderung der Einstellungen nach dem Start oeffnet die laufende Sitzung nicht (weiter klasse-ueber-route)"

    def lauf_s3(self, max_starts):
        arbeit = messen.neuer_tempordner(prefix="dbl-")
        try:
            k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
            if max_starts is not None:
                k.setdefault("probelauf", {})["max_starts"] = max_starts
            ergebnisse, meldungen, info = ablauf.fuehre_aus(k, arbeit, [], ["S3"])
            s3 = [r for r in ergebnisse if r["schritt"] == "S3"][0]
            return s3, info, meldungen
        finally:
            messen.sicher_loeschen(arbeit)

    def test_ohne_grenze_zwei_cloud_sitzungen_und_d11_geprueft(self):
        s3, info, meldungen = self.lauf_s3(None)
        namen = {t["name"]: t["ok"] for t in s3["teile"]}
        self.assertEqual(info["starts"], 2, "zwei Cloud-Sitzungen")
        self.assertIs(namen.get(self.D11), True, "die Zeile kundendaten-an-cloud wird geprueft")
        self.assertIs(namen.get(self.ERSTE), True, "die Pruefung in der ersten Sitzung laeuft zusaetzlich")
        self.assertEqual(s3["ergebnis"], "gruen")
        self.assertEqual(meldungen, [])

    def test_mit_grenze_eins_eine_sitzung_und_d11_offen(self):
        s3, info, meldungen = self.lauf_s3(1)
        namen = {t["name"]: t["ok"] for t in s3["teile"]}
        self.assertEqual(info["starts"], 1, "eine Cloud-Sitzung")
        self.assertNotIn(self.D11, namen, "D11 ist nie gruen, wenn die zweite Sitzung fehlt")
        self.assertIs(namen.get(self.ERSTE), True)
        self.assertEqual((s3["ergebnis"], s3["code_offen"]), ("offen", "s3.nur_attrappe"))


class CloudEingaben(unittest.TestCase):
    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="eingaben-")

    def tearDown(self):
        shutil.rmtree(self.arbeit, ignore_errors=True)

    def test_dritte_eingabe_weist_der_laeufer_ab_bevor_sie_die_bruecke_erreicht(self):
        L, b = neuer_lauf(self.arbeit, cloud_eingaben_max=2)
        L.art_von["s-1"] = "cloud"
        L.zug("s-1", "zaehlen", "")
        L.zug("s-1", "befehl", "env")
        self.assertEqual((L.cloud_eingaben, len(b.eingaben)), (2, 2))
        with self.assertRaises(lauf.EingabeLimit) as e:
            L.zug("s-1", "befehl", "env")
        self.assertEqual(str(e.exception), "cloud.eingaben-limit")
        self.assertEqual(len(b.eingaben), 2, "die dritte Eingabe ging nie an die Bruecke")
        self.assertEqual(L.cloud_eingaben, 2)

    def test_lokale_eingaben_zaehlen_nicht_mit(self):
        L, b = neuer_lauf(self.arbeit, cloud_eingaben_max=1)
        L.art_von["s-9"] = "lokal"
        for _ in range(4):
            L.zug("s-9", "befehl", "env")
        self.assertEqual((L.cloud_eingaben, len(b.eingaben)), (0, 4))

    def test_ohne_grenze_gibt_es_keine_abweisung(self):
        L, b = neuer_lauf(self.arbeit)
        self.assertFalse(L.knapp)
        L.art_von["s-1"] = "cloud"
        for _ in range(5):
            L.zug("s-1", "befehl", "env")
        self.assertEqual(L.cloud_eingaben, 5)

    def test_knappe_schritte_laufen_nicht_und_sind_offen_mit_nur_attrappe(self):
        L, _ = neuer_lauf(self.arbeit, cloud_eingaben_max=2)
        for kennung, fn in (("S2", schritte.s2), ("S3", schritte.s3), ("S4", schritte.s4), ("E1", schritte.e1), ("E2", schritte.e2),
                            ("E3", schritte.e3)):
            r = fn(L)
            self.assertEqual(r.ergebnis(), "offen", kennung)
            self.assertEqual(r.code_fest, kennung.lower() + ".nur_attrappe")
            self.assertEqual(r.teile, [], "keine Teilpruefung, die gruen sein koennte")
        self.assertEqual(L.cloud_eingaben, 0, "keiner dieser Schritte sendet eine Eingabe")


class Buendel(unittest.TestCase):
    def test_zeilen_lesen_nur_zahlen_und_rc(self):
        class Z:
            def ausgabe(self):
                return ("⎿ s1_umgebung=12 RC=0\ns1_route_namen=0 RC=1\nrauschen\n e4c_token_env=0 RC=1\n"
                        "z=$(env); echo \"x=$z RC=$?\"\ne4c_token_env=0 RC=0\n")
        z = schritte.buendel_lesen(Z())
        self.assertEqual(z, {"s1_umgebung": (12, 0), "s1_route_namen": (0, 1), "e4c_token_env": (0, 0)})

    def test_buendel_traegt_alle_pruefungen(self):
        arbeit = tempfile.mkdtemp(prefix="buendel-")
        try:
            L, _ = neuer_lauf(arbeit, cloud_eingaben_max=2)
            cmd = schritte.cloud_buendel(L)
            for n in ("s1_umgebung", "s1_route_namen", "s1_route_wert") + tuple("e4c_" + t for t in schritte.TOKENPRUEFUNGEN):
                self.assertIn('echo "%s=$z RC=$?"' % n, cmd)
            self.assertNotIn("{", cmd.replace("{36}", "").replace("{http_code}", ""), "keine ungesetzte Vorlage")
        finally:
            shutil.rmtree(arbeit, ignore_errors=True)


class BuendelOhneTokenpraefix(unittest.TestCase):
    """Probe 2 der Aufnahme (21.11.): Der Text des Buendels traegt `gho_` nie woertlich, und die Pruefungen finden trotzdem."""

    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="buendel-praefix-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)
        self.L, _ = neuer_lauf(self.arbeit, cloud_eingaben_max=2)
        self.c = self.L.k["copilot"]

    def zaehle(self, zeilen):
        muster, ph = schritte.ohne_tokenpraefix(self.c["token_muster"]), schritte.ohne_tokenpraefix(self.c["platzhalter"])
        skript = "printf '%%s\\n' %s | grep -oE '%s' | grep -cvE '%s'" % (" ".join(zeilen), muster, ph)
        p = subprocess.run(["/bin/sh", "-c", skript], capture_output=True, text=True, timeout=10)
        return p.stdout.strip()

    def test_kein_tokenpraefix_im_text(self):
        cmd = schritte.cloud_buendel(self.L)
        self.assertNotIn("gho_", cmd.lower())
        self.assertIn("gh''o_", cmd, "Suchmuster und Platzhalter sind noch da, nur geteilt")

    def test_shell_setzt_jedes_geaenderte_wort_byte_fuer_byte_zum_alten_zusammen(self):
        # Entscheidung Punkt 1: je Vorlage beider Konfigurationen steht {muster} und {platzhalter} nur in einfachen Anfuehrungszeichen, und
        # bash setzt das geteilte Wort genau zum alten zusammen.
        for datei in ("konfig.attrappe.json", "konfig.probelauf.json"):
            with open(os.path.join(HIER, datei), encoding="utf-8") as f:
                befehle = json.load(f)["befehle"]
            for n in schritte.TOKENPRUEFUNGEN:
                with self.subTest(datei=datei, vorlage=n):
                    v = befehle[n]
                    for feld in ("{muster}", "{platzhalter}"):
                        self.assertEqual(v.count(feld), v.count("'" + feld + "'"), "nur in einfachen Anfuehrungszeichen")
        for wert in (self.c["token_muster"], self.c["platzhalter"]):
            p = subprocess.run(["bash", "-c", "printf '%%s' '%s'" % schritte.ohne_tokenpraefix(wert)], capture_output=True, timeout=10)
            self.assertEqual(p.stdout, wert.encode("utf-8"))

    def test_gegenprobe_gepflanztes_token_macht_e4_cloud_rot(self):
        # Entscheidung Punkt 2: Im Buendel-Weg (--nur-cloud, zwei Eingaben) findet das geaenderte Buendel ein Token in der Umgebung der
        # Cloud-Sitzung der Attrappe weiter (Fehler e4-cloud-token-lesbar), und E4-cloud wird rot.
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        konf = os.path.join(self.arbeit, "konfig.json")
        with open(konf, "w", encoding="utf-8") as f:
            json.dump(k, f)
        erg = os.path.join(self.arbeit, "ergebnis.json")
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig", konf,
                            "--fehler", "e4-cloud-token-lesbar", "--ergebnis", erg], capture_output=True, timeout=150)
        with open(erg, encoding="utf-8") as f:
            d = json.load(f)
        self.assertEqual(p.returncode, 1, p.stdout.decode("utf-8", "replace")[-400:])
        e4 = [s for s in d["schritte"] if s["schritt"] == "E4-cloud"][0]
        self.assertFalse(e4["bestanden"])
        code = e4["fehlercode"] or ""
        self.assertTrue(code.startswith("e4-cloud.") and ".offen" not in code and "nur_" not in code, "E4-cloud ist rot, nicht offen: " + code)
        self.assertEqual(d["cloud_eingaben"], 2, "derselbe Buendel-Weg wie im echten Lauf")

    def test_die_shell_findet_ein_token_und_laesst_den_platzhalter_aus(self):
        token = "gho_" + "Ab1" * 12  # erfunden, 36 Zeichen
        self.assertEqual(self.zaehle([token, self.c["platzhalter"]]), "1")
        # Gegenprobe: nur der Platzhalter ergibt 0, kein Token
        self.assertEqual(self.zaehle([self.c["platzhalter"]]), "0")


class Z:
    """Ein Zug, so weit `_markerliste` ihn liest."""

    def __init__(self, mw_liste=None, start_marker=None):
        self.mw_liste, self.start_marker = mw_liste, start_marker


class Markerliste(unittest.TestCase):
    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="markerliste-")
        self.L, _ = neuer_lauf(self.arbeit)
        self.soll = set(self.L.k["marker"].values())
        self.nk, self.np = self.L.k["marker"]["nk"], self.L.k["marker"]["np"]

    def tearDown(self):
        shutil.rmtree(self.arbeit, ignore_errors=True)

    def befund(self, z):
        s = schritte.Schritt("S1", "x")
        return schritte._markerliste(self.L, s, z, "s1"), s

    def test_liste_gleich_kein_befund(self):
        b, s = self.befund(Z(set(self.soll), "ok"))
        self.assertFalse(b)
        self.assertEqual((s.offen, s.code_offen), ([], None))
        self.assertFalse(self.befund(Z(None, None))[0], "nicht geprueft ist kein Befund")

    def test_abweichungen_ergeben_den_code(self):
        for name, liste in (("np fehlt", self.soll - {self.np}), ("Kundenmarker fehlt", self.soll - {self.nk}),
                            ("fremder Marker", self.soll | {"MX-9Z9"})):
            b, s = self.befund(Z(set(liste), "ok"))
            self.assertTrue(b, name)
            self.assertEqual(s.code_offen, "s1.markerliste_abweichend", name)
            self.assertEqual(s.ergebnis(), "offen", name)
            self.assertIn(probelauf.fehlercode(s.als_dict()), probelauf.FEHLERCODES)

    def test_startdatei_abweichend_oder_unlesbar(self):
        for wert in ("abweichend", "unlesbar"):
            b, s = self.befund(Z(set(self.soll), wert))
            self.assertTrue(b, wert)
            self.assertEqual(s.code_offen, "s1.markerliste_abweichend")

    def test_pruefe_start_marker_liest_liste_und_ordner(self):
        def schreibe(sid, inhalt):
            pfad = self.L.k["marker_start_datei"].replace("{sitzung}", sid)
            os.makedirs(os.path.dirname(pfad), exist_ok=True)
            with open(pfad, "w", encoding="utf-8") as f:
                f.write(inhalt if isinstance(inhalt, str) else json.dumps(inhalt))
        zo = self.L.k["zaehlordner"]
        liste = sorted(self.soll)
        schreibe("s-1", {"marker": {"liste": liste, "ordner": zo}})
        schreibe("s-2", {"marker": {"liste": liste[:-1], "ordner": zo}})
        schreibe("s-3", {"marker": {"liste": liste + ["MX-9Z9"], "ordner": zo}})
        schreibe("s-4", {"marker": {"liste": liste, "ordner": "/anderer"}})
        schreibe("s-5", "kein json")
        erg = {sid: self.L.pruefe_start_marker(sid) for sid in ("s-1", "s-2", "s-3", "s-4", "s-5", "s-6", "../x")}
        self.assertEqual(erg, {"s-1": "ok", "s-2": "abweichend", "s-3": "abweichend", "s-4": "abweichend", "s-5": "unlesbar",
                               "s-6": "unlesbar", "../x": None})

    def test_zaehldatei_mit_fremdem_marker_ist_kein_pruefmittelfehler(self):
        d = {"sitzung": "s-1", "anfragen": 1, "marker": {"MX-9Z9": 1}}
        lauf.pruefe_zaehldatei(d, "s-1", "modellweg", list(self.soll))  # wirft nicht: eigener Befund
        with self.assertRaises(lauf.ZaehlDateiFehler):
            lauf.pruefe_zaehldatei(d, "s-1", "suche", list(self.soll))

    def test_kern_datei_legt_den_zaehlordner_fest(self):
        k = {"ziel": "echt", "zaehlordner": "/var/lib/werkbank/z/1", "probelauf": {"kern_datei": os.path.join(self.arbeit, "kern.json")}}
        with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
            json.dump({"lauf": {"marker_ordner": "/var/lib/werkbank/z/1"}}, f)
        probelauf._lies_kern_zaehlordner(k)
        self.assertEqual(k["zaehlordner"], "/var/lib/werkbank/z/1")
        with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
            json.dump({"lauf": {"marker_ordner": "/var/lib/werkbank/z/2"}}, f)
        probelauf._lies_kern_zaehlordner(k)
        self.assertEqual(k["zaehlordner"], "/var/lib/werkbank/z/2", "der Wert aus kern.json gilt")
        for inhalt in ("{", json.dumps({"lauf": {}}), json.dumps([])):
            with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
                f.write(inhalt)
            k["zaehlordner"] = "/x"
            probelauf._lies_kern_zaehlordner(k)
            self.assertIsNone(k["zaehlordner"], "nicht lesbar: die Zaehlung bleibt offen")
        k["probelauf"]["kern_datei"] = os.path.join(self.arbeit, "gibt-es-nicht")
        k["zaehlordner"] = "/x"
        probelauf._lies_kern_zaehlordner(k)
        self.assertIsNone(k["zaehlordner"])

    def test_kern_datei_ohne_zaehlordner_in_der_konfiguration(self):
        # Regression: ohne `zaehlordner` verglich der Laeufer mit realpath("\0"), das ab Python 3.15 ValueError wirft
        k = {"ziel": "echt", "probelauf": {"kern_datei": os.path.join(self.arbeit, "kern.json")}}
        with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
            json.dump({"lauf": {"marker_ordner": "/var/lib/werkbank/z/1"}}, f)
        probelauf._lies_kern_zaehlordner(k)
        self.assertEqual(k["zaehlordner"], "/var/lib/werkbank/z/1")
        k["zaehlordner"] = None
        probelauf._lies_kern_zaehlordner(k)
        self.assertEqual(k["zaehlordner"], "/var/lib/werkbank/z/1")


class Form(unittest.TestCase):
    def datei(self, umfang="voll", **ueber):
        d = probelauf.baue_ergebnis(0, 1, {"paket": "1", "laeufer": "1", "copilot_cli": "1", "ubuntu": "26.04", "kernel": "7.0"}, 1, "pruefclient",
                                    None, [], umfang=umfang, eingaben=2)
        d.update(ueber)
        return d

    def test_felder_und_kennzeichnung(self):
        self.assertEqual(probelauf.pruefe_form(self.datei()), [])
        d = self.datei("nur_cloud")
        self.assertEqual((d["umfang"], d["kennzeichnung"], d["bestanden"], d["cloud_eingaben"]), ("nur_cloud", "Teilumfang", False, 2))
        self.assertEqual(probelauf.pruefe_form(d), [])
        self.assertIn("kennzeichnung", probelauf.pruefe_form(dict(d, kennzeichnung=None)))
        self.assertIn("umfang", probelauf.pruefe_form(dict(self.datei(), umfang="halb")))

    def test_nur_cloud_ist_nie_bestanden(self):
        schritte_ = [{"schritt": s, "ergebnis": "gruen", "dauer_ms": 1, "teile": [], "fehler": None, "code_fest": None, "code_rot": None,
                      "code_offen": None} for s in probelauf.SCHRITTE]
        d = probelauf.baue_ergebnis(0, 1, {"paket": "1", "laeufer": "1", "copilot_cli": "1", "ubuntu": "1", "kernel": "1"}, 1, "pruefclient", None,
                                    schritte_, umfang="nur_cloud")
        self.assertFalse(d["bestanden"], "auch wenn jeder Schritt gruen ist")
        self.assertIn("bestanden passt nicht zu den Schritten", probelauf.pruefe_form(dict(d, bestanden=True)))
        voll = probelauf.baue_ergebnis(0, 1, d["versionen"], 1, "pruefclient", None, schritte_, umfang="voll")
        self.assertTrue(voll["bestanden"])


class NurCloudLauf(unittest.TestCase):
    """Ganzer Probelauf gegen die Attrappen mit `--nur-cloud` und der Grenze von zwei Cloud-Eingaben."""

    @classmethod
    def setUpClass(cls):
        cls.ordner = tempfile.mkdtemp(prefix="nurcloud-")
        with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
            k = json.load(f)
        k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        cls.konfig = os.path.join(cls.ordner, "konfig.json")
        with open(cls.konfig, "w", encoding="utf-8") as f:
            json.dump(k, f)
        cls.ergebnis = os.path.join(cls.ordner, "ergebnis.json")
        p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig", cls.konfig,
                            "--ergebnis", cls.ergebnis], capture_output=True, timeout=150)
        cls.exit, cls.text = p.returncode, p.stdout.decode("utf-8", "replace")
        with open(cls.ergebnis, encoding="utf-8") as f:
            cls.d = json.load(f)
        cls.codes = {s["schritt"]: s["fehlercode"] for s in cls.d["schritte"]}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.ordner, ignore_errors=True)

    def test_exit_vier_ohne_roten_schritt_nie_null(self):
        self.assertEqual(self.exit, 4)
        self.assertFalse(self.d["bestanden"])

    def test_alle_cloud_teile_gruen_und_trotzdem_nie_bestanden(self):
        self.assertIsNone(self.codes["S1"], "S1 ist gruen")
        rot = [s for s, c in self.codes.items() if c and not c.endswith((".nur_cloud", ".nur_attrappe"))]
        self.assertEqual(rot, [], "kein Schritt rot: nur die ausgelassenen sind offen")
        self.assertNotEqual(self.exit, 0)

    def test_teilumfang_steht_in_der_datei_und_auf_der_konsole(self):
        self.assertEqual((self.d["umfang"], self.d["kennzeichnung"]), ("nur_cloud", "Teilumfang"))
        self.assertIn("Teilumfang", self.text)
        self.assertEqual(probelauf.pruefe_form(self.d), [])

    def test_lokale_schritte_nur_cloud_cloud_schritte_nur_attrappe(self):
        for s in ("S5", "E4-lokal", "E5", "E6"):
            self.assertEqual(self.codes[s], s.lower() + ".nur_cloud")
        for s in ("S2", "S3", "S4", "E1", "E2", "E3"):
            self.assertEqual(self.codes[s], s.lower() + ".nur_attrappe")
        self.assertEqual(len(self.codes), 12, "keiner fehlt")
        for s in self.d["schritte"]:
            if s["schritt"] != "S1":
                self.assertFalse(s["bestanden"], s["schritt"] + " ist nie gruen")

    def test_ein_start_und_zwei_eingaben(self):
        self.assertEqual((self.d["copilot_starts"], self.d["cloud_eingaben"]), (1, 2))

    def test_ohne_gateway_keine_pruefinstanz(self):
        self.assertIsNone(self.d["voraussetzung"])

    def test_offene_gruende_in_datei_und_konsole_gleich(self):
        """Testlauf vom 21.11.2026: je Schritt die offenen Gruende als feste Codes, in der Datei (`offen`) und auf der Konsole
        (`  <schritt> offen: <code>`) gleich, in derselben Reihenfolge; dazu je Schritt mit Teilen die Zahl gruener und roter Teile."""
        konsole = {}
        for z in self.text.splitlines():
            m = re.match(r"^  (\S+) offen: (\S+)$", z)
            if m:
                konsole.setdefault(m.group(1), []).append(m.group(2))
        datei = {s["schritt"]: s["offen"] for s in self.d["schritte"] if s["offen"]}
        self.assertEqual(konsole, datei)
        self.assertIn("e4-cloud.nur_attrappe", datei["E4-cloud"])
        self.assertEqual(datei.get("S1"), None, "S1 ist gruen und hat keinen offenen Grund")
        for s, codes in datei.items():
            for c in codes:
                self.assertRegex(c, probelauf.KENNUNG, s)
        self.assertRegex(self.text, r"(?m)^  E4-cloud teile: [1-9][0-9]* gruen, 0 rot$")
        self.assertRegex(self.text, r"(?m)^  S1 teile: [1-9][0-9]* gruen, 0 rot$")
        for z in self.text.splitlines():
            if " offen: " in z:
                self.assertRegex(z, r"^  \S+ offen: \S+$", "kein Text nach der Kennung")

    def test_form_lehnt_falsche_offen_liste_ab(self):
        d = json.loads(json.dumps(self.d))
        self.assertEqual(probelauf.pruefe_form(d), [])
        for falsch in ("e4-cloud.offen", ["Text mit Leerzeichen"], [1], ["a"] * 41):
            with self.subTest(falsch=str(falsch)[:30]):
                d2 = json.loads(json.dumps(self.d))
                d2["schritte"][9]["offen"] = falsch
                self.assertIn("E4-cloud.offen", probelauf.pruefe_form(d2))
        d3 = json.loads(json.dumps(self.d))
        d3["schritte"][0]["offen"] = ["copilot.ziel_url"]
        self.assertIn("S1.offen", probelauf.pruefe_form(d3), "ein bestandener Schritt hat keinen offenen Grund")
        d4 = json.loads(json.dumps(self.d))
        del d4["schritte"][0]["offen"]
        self.assertTrue(any(m.startswith("schritt S1") for m in probelauf.pruefe_form(d4)))


class GegenprobeOhneDiagnose(unittest.TestCase):
    """Nachtrag 8: Gegenprobe von S1 ohne Diagnose der Bruecke (echt): Sitzungsliste und direkte Kinder des Kerns (`/proc/[0-9]*/stat`)."""

    def setUp(self):
        self.ordner = tempfile.mkdtemp(prefix="gegenprobe-")
        self.proc = os.path.join(self.ordner, "proc")
        os.mkdir(self.proc)
        self.addCleanup(shutil.rmtree, self.ordner, True)

    def prozess(self, pid, ppid, name="agent"):
        os.makedirs(os.path.join(self.proc, str(pid)), exist_ok=True)
        with open(os.path.join(self.proc, str(pid), "stat"), "w") as f:
            f.write("%d (%s) S %d 1 1 0 -1 4194304 100 0 0 0 1 1 0 0 20 0 1 0 12345 1000000 100\n" % (pid, name, ppid))

    def stub_systemctl(self, mainpid):
        pfad = os.path.join(self.ordner, "systemctl")
        with open(pfad, "w") as f:
            f.write("#!/bin/sh\n[ \"$1 $2 $3 $4 $5\" = \"--user show -p MainPID --value\" ] || exit 2\n"
                    "[ \"$6\" = werkbank-kern ] || exit 3\necho %s\n" % mainpid)
        os.chmod(pfad, 0o755)
        return mock.patch.dict(os.environ, {"PATH": self.ordner + os.pathsep + os.environ.get("PATH", "")})

    # --- Messfunktionen -------------------------------------------------------------------------------------------------------
    def test_kinder_zaehlt_nur_direkte_kinder_und_liest_nur_stat(self):
        self.prozess(100, 1)
        self.prozess(101, 100)
        self.prozess(102, 100, name="a b) c (d")  # Name mit Leerzeichen und Klammern: PPID zaehlt nach der letzten `)`
        self.prozess(103, 101)  # Enkel
        self.prozess(104, 999)
        os.makedirs(os.path.join(self.proc, "self"))  # kein Prozess
        for n in ("environ", "cmdline"):
            with open(os.path.join(self.proc, "101", n), "w") as f:
                f.write("GEHEIM=1")
        geoeffnet = []
        echt = open

        def spion(datei, *a, **kw):
            geoeffnet.append(str(datei))
            return echt(datei, *a, **kw)
        with mock.patch("builtins.open", spion):
            n = messen.kinder_zahl(self.proc, 100)
        self.assertEqual(n, 2)
        self.assertTrue(geoeffnet and all(os.path.basename(x) == "stat" for x in geoeffnet), geoeffnet)

    def test_kinder_ohne_proc_oder_pid_ist_none(self):
        self.assertIsNone(messen.kinder_zahl(os.path.join(self.ordner, "gibt-es-nicht"), 100), "keine Wurzel")
        self.assertIsNone(messen.kinder_zahl(self.proc, 100), "kein Prozess gelesen")
        self.assertIsNone(messen.kinder_zahl(self.proc, None))
        self.assertIsNone(messen.kinder_zahl(self.proc, 0))
        self.prozess(100, 1)
        self.assertEqual(messen.kinder_zahl(self.proc, 100), 0, "ein lesbarer Prozess ohne Kinder: 0, nicht None")

    def test_main_pid_aus_systemctl(self):
        with self.stub_systemctl("4242"):
            self.assertEqual(messen.main_pid("werkbank-kern"), 4242)
            self.assertIsNone(messen.main_pid("-x"), "ein Name mit `-` am Anfang wird nie uebergeben")
            self.assertIsNone(messen.main_pid(None))
            self.assertIsNone(messen.main_pid("anderer-dienst"), "systemctl scheitert")
        with self.stub_systemctl("0"):
            self.assertIsNone(messen.main_pid("werkbank-kern"), "MainPID 0: die Einheit laeuft nicht")
        with self.stub_systemctl("kein-zahl"):
            self.assertIsNone(messen.main_pid("werkbank-kern"))

    # --- Auswertung -----------------------------------------------------------------------------------------------------------
    def auswerten(self, vorher, nachher):
        L, _ = neuer_lauf(self.ordner)
        s = schritte.Schritt("S1", "x")
        with mock.patch.object(schritte, "_kinder_fenster", return_value=nachher):
            schritte._gegenprobe_ohne_diagnose(L, s, vorher)
        return s, L

    def teile(self, s):
        return {n: ok for _, n, ok in s.teile}

    def test_liste_gleich_kinder_gleich_ist_gruen(self):
        s, L = self.auswerten(({"s-1"}, 3), ({"s-1"}, 3))
        self.assertEqual(s.ergebnis(), "gruen")
        self.assertEqual(sorted(self.teile(s).values()), [True, True], "beide Teile halten")
        self.assertEqual(L.gegenprobe, {"weg": "sitzungen_und_kinder", "neue_sitzungen": 0, "neue_kinder": 0})
        self.assertEqual((s.gegenwerte["gegenprobe_weg"], s.gegenwerte["neue_sitzungen"], s.gegenwerte["neue_kinder"]),
                         ("sitzungen_und_kinder", 0, 0))

    def test_neue_sitzung_in_der_liste_ist_rot(self):
        s, L = self.auswerten(({"s-1"}, 3), ({"s-1", "s-2"}, 3))
        self.assertEqual(s.ergebnis(), "rot")
        self.assertEqual(L.gegenprobe["neue_sitzungen"], 1)
        self.assertEqual(L.gegenprobe["neue_kinder"], 0)

    def test_neues_kind_des_kerns_ist_rot(self):
        s, L = self.auswerten(({"s-1"}, 3), ({"s-1"}, 4))
        self.assertEqual(s.ergebnis(), "rot")
        self.assertEqual((L.gegenprobe["neue_sitzungen"], L.gegenprobe["neue_kinder"]), (0, 1))

    def test_negative_kinderzahl_ist_rot_und_die_form_bleibt_gueltig(self):
        s, L = self.auswerten(({"s-1"}, 3), ({"s-1"}, 2))  # ein Kind des Kerns endete im Fenster
        self.assertEqual(s.ergebnis(), "rot")
        self.assertEqual(L.gegenprobe["neue_kinder"], -1)
        d = Form().datei()
        d["s1_gegenprobe"] = dict(L.gegenprobe)
        self.assertEqual(probelauf.pruefe_form(d), [], "das Rot verschwindet nicht hinter einem Formfehler")
        d["s1_gegenprobe"] = {"weg": "sitzungen_und_kinder", "neue_sitzungen": -1, "neue_kinder": 0}
        self.assertEqual(probelauf.pruefe_form(d), ["s1_gegenprobe.neue_sitzungen"], "neue_sitzungen bleibt ab 0")
        d["s1_gegenprobe"] = {"weg": "sitzungen_und_kinder", "neue_sitzungen": 0, "neue_kinder": -100001}
        self.assertEqual(probelauf.pruefe_form(d), ["s1_gegenprobe.neue_kinder"])

    def test_nicht_messbar_ist_offen_nie_gruen(self):
        for name, vorher, nachher, kinder_teil in (("MainPID 0 oder /proc nicht lesbar", ({"s-1"}, None), ({"s-1"}, None), False),
                                                   ("Kinder nur nachher", ({"s-1"}, 3), ({"s-1"}, None), False),
                                                   ("Liste nicht lesbar", (None, 3), (None, 3), True)):
            s, L = self.auswerten(vorher, nachher)
            self.assertEqual(s.ergebnis(), "offen", name)
            self.assertEqual(len(s.offen), 1, name)
            self.assertEqual(len(s.teile), 1, name + ": nur der messbare Teil steht da")
            if kinder_teil:
                self.assertIn("Keine neuen direkten Kinder des Kerns fuer die abgelehnten Starts", self.teile(s))
            else:
                self.assertIn("Keine neue Sitzung in der Sitzungsliste fuer die abgelehnten Starts", self.teile(s))

    def test_nicht_messbar_aber_der_messbare_teil_rot_ist_rot(self):
        s, _ = self.auswerten(({"s-1"}, None), ({"s-1", "s-2"}, None))
        self.assertEqual(s.ergebnis(), "rot")

    # --- Fenster: echte Messfunktionen mit Stub und Attrappe von /proc ----------------------------------------------------------------
    def test_fenster_mit_stub_und_proc_attrappe(self):
        class B:
            def sitzungs_ids(self):
                return {"s-1"}
        L, _ = neuer_lauf(self.ordner)
        L.b = B()
        L.k["kern_einheit"] = "werkbank-kern"
        L.k["bruecke"]["proc_wurzel"] = self.proc
        self.prozess(4242, 1, "werkbank-kern")
        self.prozess(4300, 4242)
        self.prozess(4301, 4242)
        with self.stub_systemctl("4242"):
            self.assertEqual(schritte._kinder_fenster(L), ({"s-1"}, 2))
        with self.stub_systemctl("0"):
            self.assertEqual(schritte._kinder_fenster(L), ({"s-1"}, None))
        L.k["bruecke"]["proc_wurzel"] = os.path.join(self.ordner, "gibt-es-nicht")
        with self.stub_systemctl("4242"):
            self.assertEqual(schritte._kinder_fenster(L), ({"s-1"}, None))
        L.k.pop("kern_einheit")
        self.assertEqual(schritte._kinder_fenster(L), ({"s-1"}, None), "ohne Einheit keine Kinderzahl")

    def lauf_s1(self, aendere):
        arbeit = messen.neuer_tempordner(prefix="dbl-")
        try:
            k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
            aendere(k)
            ergebnisse, meldungen, info = ablauf.fuehre_aus(k, arbeit, [], ["S1"])
            return [r for r in ergebnisse if r["schritt"] == "S1"][0], info
        finally:
            messen.sicher_loeschen(arbeit)

    def test_mit_diagnose_laeuft_der_alte_weg_unveraendert(self):
        s1, info = self.lauf_s1(lambda k: None)
        namen = {t["name"]: t["ok"] for t in s1["teile"]}
        self.assertIs(namen.get("Weder Startprogramm noch Harness fuer die abgelehnten Starts gelaufen"), True)
        self.assertNotIn("Keine neue Sitzung in der Sitzungsliste fuer die abgelehnten Starts", namen)
        self.assertEqual(s1["ergebnis"], "gruen")
        self.assertEqual(info["gegenprobe"], {"weg": "diagnose", "neue_sitzungen": None, "neue_kinder": None})

    def test_ohne_diagnose_gegen_die_attrappe_ist_s1_gruen_mit_stub_und_proc(self):
        self.prozess(4242, 1, "werkbank-kern")
        self.prozess(4300, 4242)

        def aendere(k):
            k["bruecke"]["diagnose_pfad"] = None
            k["bruecke"]["proc_wurzel"] = self.proc
            k["kern_einheit"] = "werkbank-kern"
        with self.stub_systemctl("4242"):
            s1, info = self.lauf_s1(aendere)
        namen = {t["name"]: t["ok"] for t in s1["teile"]}
        self.assertIs(namen.get("Keine neue Sitzung in der Sitzungsliste fuer die abgelehnten Starts"), True)
        self.assertIs(namen.get("Keine neuen direkten Kinder des Kerns fuer die abgelehnten Starts"), True)
        self.assertNotIn("Weder Startprogramm noch Harness fuer die abgelehnten Starts gelaufen", namen)
        self.assertEqual(s1["ergebnis"], "gruen", "S1 kann ohne Diagnose gruen werden")
        self.assertEqual(info["gegenprobe"], {"weg": "sitzungen_und_kinder", "neue_sitzungen": 0, "neue_kinder": 0})

    def test_ohne_diagnose_und_ohne_kern_einheit_ist_s1_offen(self):
        def aendere(k):
            k["bruecke"]["diagnose_pfad"] = None
        s1, info = self.lauf_s1(aendere)
        self.assertEqual(s1["ergebnis"], "offen")
        self.assertEqual((info["gegenprobe"]["weg"], info["gegenprobe"]["neue_sitzungen"], info["gegenprobe"]["neue_kinder"]),
                         ("sitzungen_und_kinder", 0, None))

    def test_ergebnisfeld_ohne_pfade_und_pids(self):
        d = Form().datei()
        d["s1_gegenprobe"] = {"weg": "sitzungen_und_kinder", "neue_sitzungen": 0, "neue_kinder": 0}
        self.assertEqual(probelauf.pruefe_form(d), [])
        for schlecht in ({"weg": "/proc", "neue_sitzungen": 0, "neue_kinder": 0}, {"weg": None, "neue_sitzungen": "4242", "neue_kinder": 0},
                         {"weg": None, "neue_sitzungen": 0, "neue_kinder": 0, "pid": 4242}, {"weg": None}):
            self.assertNotEqual(probelauf.pruefe_form(dict(d, s1_gegenprobe=schlecht)), [], schlecht)


class NurCloudRot(unittest.TestCase):
    """`--nur-cloud` mit einem roten Schritt (Fehler `s1-route-umgebung` der Attrappe): Exit 1, nie 0, nie bestanden."""

    def test_roter_schritt_ergibt_exit_eins(self):
        ordner = tempfile.mkdtemp(prefix="nurcloud-rot-")
        try:
            with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
                k = json.load(f)
            k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
            konf = os.path.join(ordner, "konfig.json")
            with open(konf, "w", encoding="utf-8") as f:
                json.dump(k, f)
            erg = os.path.join(ordner, "ergebnis.json")
            p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--nur-cloud", "--konfig", konf,
                                "--fehler", "s1-route-umgebung", "--ergebnis", erg], capture_output=True, timeout=150)
            with open(erg, encoding="utf-8") as f:
                d = json.load(f)
            self.assertEqual(p.returncode, 1)
            self.assertFalse(d["bestanden"])
            self.assertEqual((d["umfang"], d["kennzeichnung"]), ("nur_cloud", "Teilumfang"))
            s1 = [s for s in d["schritte"] if s["schritt"] == "S1"][0]
            self.assertFalse(s1["bestanden"])
            self.assertNotIn("nur_", s1["fehlercode"] or "", "S1 ist rot, nicht ausgelassen")
        finally:
            shutil.rmtree(ordner, ignore_errors=True)


class VoraussetzungenNurCloud(unittest.TestCase):
    def test_gateway_voraussetzungen_entfallen_nur_mit_dem_schalter(self):
        with open(os.path.join(HIER, "konfig.probelauf.json"), encoding="utf-8") as f:
            ein = json.load(f)["probelauf"]["voraussetzungen"]
        codes = {v["code"] for v in ein}
        self.assertIn("modellweg.schluessel-fehlt", codes)
        self.assertIn("gateway.nicht-erreichbar", codes)
        # Der Code liest `umfang`: ohne Schalter bleibt jede Voraussetzung, mit ihm entfallen die des Gateways.
        self.assertEqual([v["art"] for v in ein if v["art"] == "pruefinstanz"], ["pruefinstanz"])


class ArbeitsordnerOrte(unittest.TestCase):
    """Arbeitsordner im echten Lauf unter `$ARBEIT` im Heim, nie an einem geschuetzten Ort (Befund eines Testlaufs, Entscheidung vom 21.11.2026)."""

    def setUp(self):
        # Ein festes Heim, das nicht angelegt wird: Die Pruefung liest nur Pfade. Ein Temp-Heim laege auf dem Linux-Baurechner unter /tmp, und dann
        # waere /tmp ein Ordner ueber dem Heim (Grund `heim` statt `system`).
        self.heim = "/srv/wb-test-heim/wb"
        self.arbeit = os.path.join(self.heim, "werkbank-probelauf-test")

    def probelauf_konfig(self):
        return konfig.lade(os.path.join(HIER, "konfig.probelauf.json"), HIER, self.arbeit, "wb")

    def test_ort_hinter_einem_verweis_gilt_wie_der_ort(self):
        """Ein Arbeitsordner, der ueber einen Verweis in einem Systemordner landet, ergibt denselben Grund wie der Systemordner selbst
        (unter macOS sind `/etc` und `/tmp` Verweise auf `/private/...`; unter Linux gilt dasselbe fuer jeden Verweis)."""
        basis = tempfile.mkdtemp(prefix="orte-")
        self.addCleanup(shutil.rmtree, basis, True)
        verweis = os.path.join(basis, "nach-etc")
        os.symlink("/etc", verweis)
        k = self.probelauf_konfig()
        k["start"]["cloud"]["arbeitsordner"] = os.path.join(verweis, "o1")
        self.assertEqual(probelauf.arbeitsordner_befund(k, heim=self.heim), "system")

    def test_var_tmp_ist_geschuetzt_ein_ordner_darunter_nicht(self):
        """`/var/tmp` ist wie `/tmp` ein gemeinsamer Temp-Ordner (Nachtrag 25.11.2026, verbote.rs). Ein Ordner darunter bleibt erlaubt; ein
        Verweis, der auf `/var/tmp` zeigt, ergibt denselben Grund. Nichts wird angelegt: die Pruefung liest nur Pfade."""
        k = self.probelauf_konfig()
        k["start"]["cloud"]["arbeitsordner"] = "/var/tmp"
        self.assertEqual(probelauf.arbeitsordner_befund(k, heim=self.heim), "system")
        k["start"]["cloud"]["arbeitsordner"] = "/var/tmp/wb-test-x/o1"
        self.assertIsNone(probelauf.arbeitsordner_befund(k, heim=self.heim), "ein Ordner unter /var/tmp ist erlaubt")
        basis = tempfile.mkdtemp(prefix="orte-")
        self.addCleanup(shutil.rmtree, basis, True)
        verweis = os.path.join(basis, "nach-var-tmp")
        os.symlink("/var/tmp", verweis)
        k["start"]["cloud"]["arbeitsordner"] = verweis
        self.assertEqual(probelauf.arbeitsordner_befund(k, heim=self.heim), "system", "ueber einen Verweis gilt derselbe Grund")

    def test_probelauf_konfig_legt_die_arbeitsordner_unter_arbeit(self):
        k = self.probelauf_konfig()
        for art in ("cloud", "lokal"):
            p = k["start"][art]["arbeitsordner"]
            self.assertTrue(p.startswith(self.arbeit + os.sep), art)
            self.assertFalse(p.startswith("/var/lib/werkbank"), "nie unter dem geschuetzten Zustandsordner")
        self.assertIsNone(probelauf.arbeitsordner_befund(k, heim=self.heim))

    def test_geschuetzte_orte_ergeben_den_festen_grund(self):
        k = self.probelauf_konfig()
        faelle = {
            "/var/lib/werkbank/wb/ordner/a/o1": "geschuetzt",          # Zustandsordner (Code 5 des Einschlusses)
            "/var/lib/werkbank/z/1000/o1": "geschuetzt",               # Zaehlordner
            "/etc/werkbank/o1": "system",
            self.heim: "heim",
            os.path.dirname(self.heim): "heim",
            os.path.join(self.heim, ".config", "o1"): "geheimnisteil",
            os.path.join(self.heim, ".ssh"): "geheimnisteil",
            os.path.join(self.heim, ".copilot", "o1"): "geschuetzt",
            "/tmp": "system",
            "/var/tmp": "system",
            "/usr/local/o1": "system",
            "/opt/o1": "system",
        }
        for pfad, grund in faelle.items():
            with self.subTest(pfad):
                k2 = json.loads(json.dumps(k))
                k2["start"]["cloud"]["arbeitsordner"] = pfad
                self.assertEqual(probelauf.arbeitsordner_befund(k2, heim=self.heim), grund)
        k2 = json.loads(json.dumps(k))
        k2["start"]["lokal"]["arbeitsordner"] = os.path.join(k2["vault"], "o1")
        self.assertEqual(probelauf.arbeitsordner_befund(k2, heim=self.heim), "geschuetzt", "auch der lokale Ordner zaehlt")

    def test_voraussetzung_endet_vor_dem_ersten_start_mit_festem_code(self):
        k = self.probelauf_konfig()
        k["start"]["cloud"]["arbeitsordner"] = "/var/lib/werkbank/wb/ordner/b/o2"
        hinweise = []
        with mock.patch.object(probelauf, "_offen", return_value=False), \
                mock.patch.object(os.path, "expanduser", return_value=self.heim):
            self.assertEqual(probelauf.voraussetzungen(k, hinweise), "probelauf.arbeitsordner_geschuetzt")
        self.assertEqual(hinweise, ["arbeitsordner: geschuetzt"])
        self.assertIn("probelauf.arbeitsordner_geschuetzt", probelauf.VORAUSSETZUNGEN)

    def test_arbeit_basis_nur_im_echten_lauf_im_heim(self):
        with mock.patch.object(os.path, "expanduser", return_value=self.heim):
            self.assertEqual(probelauf.arbeit_basis(os.path.join(HIER, "konfig.probelauf.json")), self.heim)
            self.assertIsNone(probelauf.arbeit_basis(os.path.join(HIER, "konfig.attrappe.json")), "Tests legen nichts im Heim an")
            self.assertIsNone(probelauf.arbeit_basis(os.path.join(self.heim, "gibt-es-nicht.json")))


class NurCloudExit(unittest.TestCase):
    """Exitcodes von `probelauf.lauf` mit --nur-cloud ohne Attrappen: Voraussetzung und Meldungen beim Aufraeumen (Befund B4)."""

    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="nurcloud-exit-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)
        self.k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, self.arbeit)
        self.k["umfang"] = "nur_cloud"

    @staticmethod
    def gruen():
        return [{"schritt": s, "ergebnis": "gruen", "dauer_ms": 1, "teile": [], "offen": [], "fehler": None, "code_fest": None,
                 "code_rot": None, "code_offen": None, "messwerte": {}} for s in probelauf.SCHRITTE]

    def lauf(self, voraus=None, meldungen=()):
        info = {"starts": 1, "browser": "pruefclient", "cloud_eingaben": 2}
        vers = {n: "1.0" for n in ("paket", "laeufer", "copilot_cli", "ubuntu", "kernel")}
        with mock.patch.object(probelauf, "versionen", return_value=vers), \
                mock.patch.object(probelauf, "voraussetzungen", return_value=voraus), \
                mock.patch.object(ablauf, "fuehre_aus", return_value=(self.gruen(), list(meldungen), info)):
            return probelauf.lauf(self.k, self.arbeit, os.path.join(self.arbeit, "e.json"), HIER)

    def test_ohne_meldung_exit_4(self):
        self.assertEqual(self.lauf()[0], probelauf.EXIT_TEILUMFANG)

    def test_fehlende_voraussetzung_bleibt_exit_3(self):
        code, d, _, _ = self.lauf(voraus="bruecke.sitzungen_laufen")
        self.assertEqual(code, 3)
        self.assertEqual((d["voraussetzung"], d["bestanden"], d["kennzeichnung"]), ("bruecke.sitzungen_laufen", False, "Teilumfang"))

    def test_meldung_beim_aufraeumen_ist_exit_1_wie_im_vollen_lauf(self):
        meld = ["Dienst werkbank-kern nicht gestoppt"]
        self.k["umfang"] = "voll"
        self.assertEqual(self.lauf(meldungen=meld)[0], 1, "voll: probelauf.py, Exit 1 bei einer Meldung")
        self.k["umfang"] = "nur_cloud"
        self.assertEqual(self.lauf(meldungen=meld)[0], 1, "Teilumfang darf eine Meldung nicht hinter Exit 4 verbergen")


class MarkerDatei(unittest.TestCase):
    """Markerdatei fuer den Kern: Ordner 0700, Datei 0600, genau `{"liste": [...]}`; Stub fuer systemctl."""

    def setUp(self):
        self.s = None

    def tearDown(self):
        if self.s is not None:
            self.s.loesche()

    def stub(self, **kw):
        self.s = test_dienste.Stub(**kw)
        self.pfad = os.path.join(self.s.xdg, "werkbank-probelauf", "marker.json")
        return self.s

    def k(self):
        k = self.s.k()
        k["marker"] = {"nk": "MK-7Q2", "ni": "MI-4F9", "ns": "MS-3P8", "np": "MP-2C5"}
        k["probelauf"] = {"marker_datei": self.pfad}
        return k

    def umgebung(self):
        return mock.patch.dict(os.environ, {"PATH": self.s.ordner + os.pathsep + os.environ.get("PATH", ""), "XDG_RUNTIME_DIR": self.s.xdg})

    def fuehre(self, k, im_lauf=None, voraus=None, sitzungen=0):
        """`probelauf.lauf` mit einem Stand-in fuer `fuehre_aus`, der den Zustand mitten im Lauf liest. `sitzungen`: Antwort der Zaehlung
        laufender Sitzungen (die Stub-Dateien der Bruecke gehoeren keiner echten Bruecke); `"echt"` laesst `_sitzungen` selbst lesen."""
        arbeit = tempfile.mkdtemp(prefix="marker-arbeit-")
        mitte = {}

        def mitte_(*a, **kw):
            p = self.pfad
            mitte["da"] = os.path.exists(p)
            if mitte["da"]:
                mitte["datei_modus"] = stat.S_IMODE(os.stat(p).st_mode)
                mitte["ordner_modus"] = stat.S_IMODE(os.stat(os.path.dirname(p)).st_mode)
                with open(p, "rb") as f:
                    mitte["inhalt"] = f.read()
            if im_lauf:
                im_lauf()
            return [], [], {"starts": 0, "browser": "pruefclient"}
        try:
            with self.umgebung(), mock.patch.object(probelauf, "voraussetzungen", return_value=voraus), \
                    mock.patch.object(ablauf, "fuehre_aus", side_effect=mitte_):
                if sitzungen == "echt":
                    return probelauf.lauf(k, arbeit, os.path.join(arbeit, "e.json"), HIER), mitte
                with mock.patch.object(probelauf.ProbeDienste, "_sitzungen", return_value=sitzungen):
                    return probelauf.lauf(k, arbeit, os.path.join(arbeit, "e.json"), HIER), mitte
        finally:
            shutil.rmtree(arbeit, ignore_errors=True)

    def test_normaler_lauf(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        (code, d, _, meld), mitte = self.fuehre(self.k())
        self.assertIsNone(d["voraussetzung"])
        self.assertTrue(mitte["da"])
        self.assertEqual((mitte["datei_modus"], mitte["ordner_modus"]), (0o600, 0o700))
        self.assertEqual(json.loads(mitte["inhalt"]), {"liste": ["MK-7Q2", "MI-4F9", "MS-3P8", "MP-2C5"]})
        self.assertEqual(list(json.loads(mitte["inhalt"])), ["liste"], "kein Feld ordner")
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "Datei und Ordner am Ende weg")
        self.assertEqual(meld, [])
        self.assertEqual(self.s.aufrufe_mit("start"), [KERN, BRUECKE])
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE, KERN], "vorher lief nichts: am Ende ist alles gestoppt")
        self.assertEqual(self.s.aufrufe_mit("restart"), [])

    def test_vorher_lief_alles_wird_neu_gestartet_und_ohne_datei_wiederhergestellt(self):
        self.stub(laeuft=[KERN, BRUECKE], dateien=True)
        (code, d, _, meld), mitte = self.fuehre(self.k())
        self.assertTrue(mitte["da"])
        self.assertEqual(self.s.aufrufe_mit("restart"), [KERN, BRUECKE, BRUECKE, KERN], "erst mit Datei, dann ohne")
        self.assertEqual(self.s.aufrufe_mit("stop"), [])
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)))
        self.assertEqual(meld, [])

    def test_laufende_sitzung_endet_ohne_neustart(self):
        self.stub(laeuft=[KERN, BRUECKE], dateien=True)
        (code, d, _, meld), mitte = self.fuehre(self.k(), sitzungen=1)
        self.assertEqual(code, 3)
        self.assertEqual(d["voraussetzung"], "bruecke.sitzungen_laufen")
        self.assertNotIn("da", mitte, "der Lauf selbst lief nicht")
        self.assertEqual(self.s.aufrufe_mit("restart") + self.s.aufrufe_mit("start") + self.s.aufrufe_mit("stop"), [], "kein Neustart")
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "keine Datei angelegt")

    def test_kern_aktiv_und_bruecke_stumm_ist_sitzungen_unbekannt(self):
        # Der Kern laeuft, die Bruecke nicht (keine Dateien): die Zahl laesst sich nicht lesen. Nichts wird neu gestartet, keine Datei angelegt.
        self.stub(laeuft=[KERN], dateien=False)
        (code, d, _, meld), mitte = self.fuehre(self.k(), sitzungen="echt")
        self.assertEqual((code, d["voraussetzung"]), (3, "bruecke.sitzungen_unbekannt"))
        self.assertNotIn("da", mitte, "der Lauf selbst lief nicht")
        self.assertEqual(self.s.aufrufe_mit("restart") + self.s.aufrufe_mit("start") + self.s.aufrufe_mit("stop"), [], "kein Neustart")
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "keine Markerdatei angelegt")

    def test_kern_aktiv_bruecke_antwortet_nicht_ist_sitzungen_unbekannt(self):
        # Dateien der Bruecke da, aber unter dem Port hoert niemand (Stub-Dateien: 127.0.0.1:4711)
        self.stub(laeuft=[KERN, BRUECKE], dateien=True)
        (code, d, _, meld), mitte = self.fuehre(self.k(), sitzungen="echt")
        self.assertEqual(d["voraussetzung"], "bruecke.sitzungen_unbekannt")
        self.assertEqual(self.s.aufrufe_mit("restart") + self.s.aufrufe_mit("start") + self.s.aufrufe_mit("stop"), [])
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)))

    def test_nichts_aktiv_ist_die_zahl_sicher_null_und_der_start_normal(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)  # nichts laeuft, die Dateien der Bruecke liegen schon da: kein Lesen noetig
        (code, d, _, meld), mitte = self.fuehre(self.k(), sitzungen="echt")
        self.assertIsNone(d["voraussetzung"])
        self.assertTrue(mitte["da"])
        self.assertEqual(self.s.aufrufe_mit("start"), [KERN, BRUECKE])

    def test_nur_bruecke_aktiv_und_stumm_ist_sitzungen_unbekannt(self):
        self.stub(laeuft=[BRUECKE], dateien=False)
        (code, d, _, meld), mitte = self.fuehre(self.k(), sitzungen="echt")
        self.assertEqual(d["voraussetzung"], "bruecke.sitzungen_unbekannt")

    def kern_lauf(self, wert, laeuft=(), weg_mit_uid=True):
        """Konfiguration mit `kern.json`: `wert` ist `lauf.marker_datei` (None: Feld fehlt, "kaputt": keine JSON-Datei). Gibt (k, Wurzel,
        Pfad der festen Form)."""
        wurzel = tempfile.mkdtemp(prefix="marker-lauf-")
        self.addCleanup(shutil.rmtree, wurzel, True)
        if weg_mit_uid:
            os.mkdir(os.path.join(wurzel, str(os.getuid())), 0o700)
        fest = os.path.join(wurzel, str(os.getuid()), "werkbank-probelauf", "marker.json")
        kern = os.path.join(wurzel, "kern.json")
        with open(kern, "w", encoding="utf-8") as f:
            if wert == "kaputt":
                f.write("{")
            else:
                lauf_ = {"marker_ordner": "/var/lib/werkbank/z/%d" % os.getuid()}
                if wert is not None:
                    lauf_["marker_datei"] = wert
                json.dump({"lauf": lauf_}, f)
        k = self.k()
        k["probelauf"].update(kern_datei=kern, lauf_wurzel=wurzel)
        return k, wurzel, fest

    def test_kern_json_legt_den_ort_der_markerdatei_fest(self):
        # Entscheidung vom 21.11.2026: Der Kern liest genau `lauf.marker_datei` aus kern.json. Der Laeufer legt die Datei dort an, nicht unter
        # $XDG_RUNTIME_DIR, auch wenn beide verschieden sind.
        self.stub(dateien_bei=BRUECKE, dateien=True)
        xdg_ordner = os.path.dirname(self.pfad)
        k, _, fest = self.kern_lauf(None)
        with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
            json.dump({"lauf": {"marker_datei": fest, "marker_ordner": "/var/lib/werkbank/z/%d" % os.getuid()}}, f)
        self.pfad = fest
        im = {}
        (code, d, _, meld), mitte = self.fuehre(k, im_lauf=lambda: im.update(xdg=os.path.exists(xdg_ordner)))
        self.assertIsNone(d["voraussetzung"])
        self.assertTrue(mitte["da"], "die Datei liegt am Ort aus kern.json")
        self.assertEqual((mitte["datei_modus"], mitte["ordner_modus"]), (0o600, 0o700))
        self.assertEqual(json.loads(mitte["inhalt"]), {"liste": ["MK-7Q2", "MI-4F9", "MS-3P8", "MP-2C5"]})
        self.assertFalse(im["xdg"], "unter $XDG_RUNTIME_DIR liegt keine Datei")
        self.assertFalse(os.path.exists(os.path.dirname(fest)), "Datei und Ordner am Ende weg")
        self.assertTrue(os.path.isdir(os.path.dirname(os.path.dirname(fest))), "der Laufzeitordner des Nutzers bleibt")
        self.assertEqual(meld, [])
        self.assertEqual(self.s.aufrufe_mit("start"), [KERN, BRUECKE])

    def test_ohne_gueltige_marker_datei_in_kern_json_startet_nichts(self):
        # fail closed: kein Neustart, keine Datei, fester Code; auch wenn Kern und Bruecke laufen
        self.stub(laeuft=[KERN, BRUECKE], dateien=True)
        uid = str(os.getuid())
        faelle = {
            "Feld fehlt": lambda w, f: None,
            "keine JSON-Datei": lambda w, f: "kaputt",
            "relativer Pfad": lambda w, f: os.path.join(uid, "werkbank-probelauf", "marker.json"),
            "anderer Dateiname": lambda w, f: os.path.join(os.path.dirname(f), "andere.json"),
            "anderer Ordnername": lambda w, f: os.path.join(w, uid, "anderer-ordner", "marker.json"),
            "fremde uid": lambda w, f: os.path.join(w, str(os.getuid() + 1), "werkbank-probelauf", "marker.json"),
            "Pfad mit ..": lambda w, f: os.path.join(w, uid, "x", "..", "werkbank-probelauf", "marker.json"),
            "Ordner direkt im Laufzeitordner": lambda w, f: os.path.join(w, uid, "marker.json"),
            "kein Text": lambda w, f: 17,
        }
        for name, wert_von in faelle.items():
            with self.subTest(name):
                k, wurzel, fest = self.kern_lauf(None)
                wert = wert_von(wurzel, fest)
                with open(k["probelauf"]["kern_datei"], "w", encoding="utf-8") as f:
                    if wert == "kaputt":
                        f.write("{")
                    else:
                        lauf_ = {"marker_ordner": "/var/lib/werkbank/z/%s" % uid}
                        if wert is not None:
                            lauf_["marker_datei"] = wert
                        json.dump({"lauf": lauf_}, f)
                self.pfad = fest
                (code, d, _, meld), mitte = self.fuehre(k)
                self.assertEqual((code, d["voraussetzung"]), (3, "probelauf.marker_datei_fehlt"))
                self.assertNotIn("da", mitte, "der Lauf selbst lief nicht")
                self.assertFalse(os.path.exists(os.path.dirname(fest)), "keine Datei am Ort der festen Form")
                self.assertEqual(sorted(os.listdir(os.path.join(wurzel, uid))), [], "nichts im Laufzeitordner angelegt")
        self.assertEqual(self.s.aufrufe_mit("restart") + self.s.aufrufe_mit("start") + self.s.aufrufe_mit("stop"), [], "kein Dienst angefasst")

    def test_nicht_lesbare_kern_json_startet_nichts(self):
        self.stub(laeuft=[KERN, BRUECKE], dateien=True)
        k, wurzel, fest = self.kern_lauf(None)
        k["probelauf"]["kern_datei"] = os.path.join(wurzel, "gibt-es-nicht.json")
        self.pfad = fest
        (code, d, _, meld), mitte = self.fuehre(k)
        self.assertEqual((code, d["voraussetzung"]), (3, "probelauf.marker_datei_fehlt"))
        self.assertEqual(self.s.aufrufe_mit("restart") + self.s.aufrufe_mit("start") + self.s.aufrufe_mit("stop"), [])

    def test_zweites_signal_waehrend_des_aufraeumens_bricht_nicht_ab(self):
        self.stub(laeuft=[KERN], dateien_bei=BRUECKE, dateien=True)
        echt = probelauf.marker_loeschen
        gesendet, im_lauf_gesendet = [], []

        def wrap(pfad):
            if im_lauf_gesendet and not gesendet:  # beim Aufraeumen kommt ein zweites Signal
                gesendet.append(1)
                os.kill(os.getpid(), signal.SIGTERM)
            return echt(pfad)

        def erstes_signal():
            im_lauf_gesendet.append(1)
            os.kill(os.getpid(), signal.SIGTERM)
        alt = signal.getsignal(signal.SIGTERM)
        with mock.patch.object(probelauf, "marker_loeschen", new=wrap):
            with self.assertRaises(SystemExit) as e:
                self.fuehre(self.k(), im_lauf=erstes_signal)
        self.assertEqual(e.exception.code, 128 + signal.SIGTERM, "der Exitcode des ersten Signals bleibt")
        self.assertEqual(gesendet, [1], "das zweite Signal kam beim Aufraeumen")
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "Datei und Ordner sind trotzdem weg")
        self.assertEqual(self.s.aufrufe_mit("restart"), [KERN, KERN], "Zustand von vorher: der Kern lief, jetzt ohne Datei neu gestartet")
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE], "die Bruecke lief nicht: gestoppt")
        self.assertIs(signal.getsignal(signal.SIGTERM), alt, "die alten Handler sind zurueck")

    def test_reste_werden_vorher_geraeumt(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        ordner = os.path.dirname(self.pfad)
        os.makedirs(ordner, mode=0o755)
        for n in ("marker.json", ".marker.json.neu"):
            with open(os.path.join(ordner, n), "w") as f:
                f.write('{"liste": ["ALT"], "ordner": "/x"}')
        (code, d, _, meld), mitte = self.fuehre(self.k())
        self.assertEqual(json.loads(mitte["inhalt"]), {"liste": ["MK-7Q2", "MI-4F9", "MS-3P8", "MP-2C5"]})
        self.assertEqual(mitte["ordner_modus"], 0o700)
        self.assertFalse(os.path.exists(ordner))

    def test_fremder_inhalt_im_ordner_wird_nicht_angefasst(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        ordner = os.path.dirname(self.pfad)
        os.makedirs(ordner, mode=0o700)
        fremd = os.path.join(ordner, "fremd.txt")
        with open(fremd, "w") as f:
            f.write("x")
        (code, d, _, meld), mitte = self.fuehre(self.k())
        self.assertEqual(d["voraussetzung"], "bruecke.dienst-startet-nicht")
        self.assertTrue(os.path.exists(fremd), "nur die zwei Namen werden geloescht")
        self.assertEqual(self.s.aufrufe_mit("start") + self.s.aufrufe_mit("restart"), [])

    def test_ordner_als_link_wird_nicht_verfolgt(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        ziel = tempfile.mkdtemp(prefix="marker-ziel-")
        try:
            with open(os.path.join(ziel, "marker.json"), "w") as f:
                f.write("x")
            os.symlink(ziel, os.path.dirname(self.pfad))
            (code, d, _, meld), mitte = self.fuehre(self.k())
            self.assertEqual(d["voraussetzung"], "bruecke.dienst-startet-nicht")
            self.assertTrue(os.path.exists(os.path.join(ziel, "marker.json")), "dem Link wird nicht gefolgt")
        finally:
            os.unlink(os.path.dirname(self.pfad))
            shutil.rmtree(ziel, ignore_errors=True)

    def test_fehler_mitten_im_lauf(self):
        self.stub(laeuft=[KERN], dateien_bei=BRUECKE, dateien=True)

        def boese():
            raise RuntimeError("Absturz im Lauf")
        with self.assertRaises(RuntimeError):
            self.fuehre(self.k(), im_lauf=boese)
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "Datei und Ordner weg")
        self.assertEqual(self.s.aufrufe_mit("restart"), [KERN, KERN], "der Kern lief vorher: neu gestartet, jetzt ohne Datei")
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE], "die Bruecke lief vorher nicht: gestoppt")

    def test_sigterm_mitten_im_lauf(self):
        self.stub(laeuft=[KERN], dateien_bei=BRUECKE, dateien=True)
        alt = signal.getsignal(signal.SIGTERM)

        def sigterm():
            os.kill(os.getpid(), signal.SIGTERM)
        with self.assertRaises(SystemExit) as e:
            self.fuehre(self.k(), im_lauf=sigterm)
        self.assertEqual(e.exception.code, 128 + signal.SIGTERM)
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)))
        self.assertEqual(self.s.aufrufe_mit("restart"), [KERN, KERN])
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE])
        self.assertIs(signal.getsignal(signal.SIGTERM), alt, "der alte Handler ist zurueck")

    def test_sigint_mitten_im_lauf(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        with self.assertRaises(SystemExit):
            self.fuehre(self.k(), im_lauf=lambda: os.kill(os.getpid(), signal.SIGINT))
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)))
        self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE, KERN])

    def test_sighup_mitten_im_lauf(self):
        """Befund B8: ein ssh-Abbruch (SIGHUP) raeumt auf wie SIGTERM: Markerdatei weg, Dienste und Kern wie vorher."""
        self.stub(laeuft=[KERN], dateien_bei=BRUECKE, dateien=True)

        class NichtAbgefangen(Exception):
            pass

        def ersatz(_n, _f):  # haelt der Laeufer SIGHUP nicht, endet nicht der Testprozess, sondern der Test wird rot
            raise NichtAbgefangen()
        alt = signal.signal(signal.SIGHUP, ersatz)
        try:
            with self.assertRaises(SystemExit) as e:
                self.fuehre(self.k(), im_lauf=lambda: os.kill(os.getpid(), signal.SIGHUP))
            self.assertEqual(e.exception.code, 128 + signal.SIGHUP)
            self.assertFalse(os.path.exists(os.path.dirname(self.pfad)), "Datei und Ordner weg")
            self.assertEqual(self.s.aufrufe_mit("restart"), [KERN, KERN], "der Kern lief vorher: neu gestartet, jetzt ohne Datei")
            self.assertEqual(self.s.aufrufe_mit("stop"), [BRUECKE], "die Bruecke lief vorher nicht: gestoppt")
            self.assertIs(signal.getsignal(signal.SIGHUP), ersatz, "der alte Handler ist zurueck")
        finally:
            signal.signal(signal.SIGHUP, alt)

    def test_sighup_endet_wie_sigterm(self):
        alt = signal.getsignal(signal.SIGHUP)
        s = probelauf._SignaleAlsEnde().__enter__()
        try:
            self.assertIs(signal.getsignal(signal.SIGHUP), probelauf._ende_durch_signal, "ssh-Abbruch: Aufraeumen ueber SystemExit")
        finally:
            s.__exit__()
        self.assertIs(signal.getsignal(signal.SIGHUP), alt)

    def test_erstes_signal_ignoriert_weitere_sofort(self):
        """Der Wrapper reicht SIGTERM und SIGHUP weiter, ein ssh-Abbruch schickt SIGHUP zusaetzlich ueber die Gruppe: Ein zweites
        Signal vor `ignorieren` darf kein zweites SystemExit werfen."""
        alt = {n: signal.getsignal(n) for n in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
        s = probelauf._SignaleAlsEnde().__enter__()
        try:
            with self.assertRaises(SystemExit) as e:
                probelauf._ende_durch_signal(signal.SIGHUP, None)
            self.assertEqual(e.exception.code, 129)
            for n in alt:
                self.assertEqual(signal.getsignal(n), signal.SIG_IGN, n)
        finally:
            s.__exit__()
        self.assertEqual({n: signal.getsignal(n) for n in alt}, alt, "die alten Handler sind zurueck")

    def test_start_scheitert_datei_trotzdem_weg(self):
        self.stub(start_scheitert=[KERN], dateien=True)
        (code, d, _, meld), mitte = self.fuehre(self.k())
        self.assertEqual(d["voraussetzung"], "bruecke.dienst-startet-nicht")
        self.assertFalse(os.path.exists(os.path.dirname(self.pfad)))

    def test_ohne_marker_datei_in_der_konfiguration_bleibt_der_alte_weg(self):
        self.stub(dateien_bei=BRUECKE, dateien=True)
        k = self.s.k()
        (code, d, _, meld), mitte = self.fuehre(k)
        self.assertFalse(mitte["da"])
        self.assertEqual(self.s.aufrufe_mit("restart"), [])


if __name__ == "__main__":
    unittest.main()
