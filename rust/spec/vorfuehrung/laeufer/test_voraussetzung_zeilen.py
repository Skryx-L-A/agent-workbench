"""Tests der Voraussetzung `befehl_zeilen` (Auswertung von `werkbank-pruefen` nach Zeilen). Nur Standardbibliothek.
Aufruf: python3 -I -B test_voraussetzung_zeilen.py. Eine kleine Attrappe gibt nur feste Zeilen und einen festen Exit aus."""
import os
import sys
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)
from drehbuchlib import probelauf  # noqa: E402

ATTRAPPE = "import sys; sys.stdout.write(sys.argv[1].replace('|', chr(10))); sys.exit(int(sys.argv[2]))"


def befund(ausgabe, exitcode, **mehr):
    """Wahr, wenn die Voraussetzung nicht erfuellt ist (der Code der Voraussetzung wuerde gemeldet)."""
    v = {"code": "copilot.nicht-angemeldet", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
         "befehl": [sys.executable, "-I", "-B", "-c", ATTRAPPE, ausgabe, str(exitcode)]}
    v.update(mehr)
    return probelauf._zeilen_befund(v)


class Zeilen(unittest.TestCase):
    def test_nur_ok_exit0_erfuellt(self):
        self.assertFalse(befund("pruef.ok: Alle 8 Pruefungen bestanden.|", 0))

    def test_copilot_fehlt_exit1_nicht_angemeldet(self):
        self.assertTrue(befund("pruef.copilot_fehlt: Anmeldung fehlt.|", 1))

    def test_fremder_befund_exit1_erfuellt(self):
        self.assertFalse(befund("pruef.vault_fehlt: Vault fehlt.|", 1))

    def test_leere_ausgabe_exit0_nicht_angemeldet(self):
        self.assertTrue(befund("", 0))

    def test_exitcode_zaehlt_nicht(self):
        self.assertFalse(befund("pruef.ok: Alle 8 Pruefungen bestanden.|", 3))

    def test_zeile_ohne_praefix_pruef_gilt_nicht(self):
        self.assertTrue(befund("irgendetwas|", 0))

    def test_jede_form_des_praefixes(self):
        for n in ("fehlt", "art", "eigentuemer", "modus", "leer", "form", "nicht_pruefbar", "rest"):
            self.assertTrue(befund("pruef.copilot_%s: x|" % n, 1), n)

    def test_befehl_nicht_startbar(self):
        self.assertTrue(probelauf._zeilen_befund({"code": "c", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
                                                  "befehl": ["/nicht/vorhanden/werkbank-pruefen"]}))

    def test_frist(self):
        self.assertTrue(befund("pruef.ok: x|", 0, befehl=[sys.executable, "-I", "-B", "-c", "import time; time.sleep(5)"], frist_s=0.5))


HINWEIS_UBUNTU = "pruef.hinweis_system_bwrap_ubuntu: Das System-bwrap ist durch das Ubuntu-Profil frei; die Firma kann dieses Profil entfernen."
PRAEFIX = {"bwrap_praefix_ubuntu": "pruef.hinweis_system_bwrap_ubuntu", "bwrap_praefix_gesperrt": None}


def lage(ausgabe, exitcode=0, **mehr):
    l = {}
    v = {"code": "copilot.nicht-angemeldet", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
         "befehl": [sys.executable, "-I", "-B", "-c", ATTRAPPE, ausgabe, str(exitcode)]}
    v.update(PRAEFIX)
    v.update(mehr)
    probelauf._zeilen_befund(v, l)
    return l["system_bwrap"]


class SystemBwrap(unittest.TestCase):
    """Lage des System-bwrap (Nachtrag 20.11., 12): aus derselben Ausgabe, kein zweiter Aufruf, nur der Wert bleibt."""

    def test_mit_hinweiszeile_ubuntu(self):
        self.assertEqual(lage("pruef.ok: Alle 9 Pruefungen bestanden.|" + HINWEIS_UBUNTU + "|"), "ubuntu")

    def test_ohne_hinweiszeile_unbekannt(self):
        self.assertEqual(lage("pruef.ok: Alle 8 Pruefungen bestanden.|"), "unbekannt")

    def test_praefix_gesperrt_null_meldet_nie_gesperrt(self):
        self.assertEqual(lage("pruef.hinweis_system_bwrap_gesperrt: x|pruef.ok: y|"), "unbekannt")

    def test_praefix_gesperrt_gesetzt(self):
        self.assertEqual(lage("pruef.hinweis_system_bwrap_gesperrt: x|pruef.ok: y|", bwrap_praefix_gesperrt="pruef.hinweis_system_bwrap_gesperrt"), "gesperrt")

    def test_beide_zeilen_unbekannt(self):
        self.assertEqual(lage(HINWEIS_UBUNTU + "|pruef.hinweis_system_bwrap_gesperrt: x|", bwrap_praefix_gesperrt="pruef.hinweis_system_bwrap_gesperrt"), "unbekannt")

    def test_befehl_nicht_startbar_unbekannt(self):
        l = {}
        probelauf._zeilen_befund({"code": "c", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_", "befehl": ["/nicht/vorhanden"], **PRAEFIX}, l)
        self.assertEqual(l["system_bwrap"], "unbekannt")

    def test_form_kennt_das_feld(self):
        d = {"fassung": 2, "art": "probelauf-ergebnis", "beginn": "2026-11-20T10:00:00Z", "ende": "2026-11-20T10:01:00Z",
             "versionen": {"paket": "1", "laeufer": "1", "copilot_cli": "1", "ubuntu": "26.04", "kernel": "7"}, "copilot_starts": 0, "cloud_eingaben": 0, "umfang": "voll", "kennzeichnung": None, "s1_gegenprobe": {"weg": None, "neue_sitzungen": None, "neue_kinder": None},
             "browser": "pruefclient", "ca": {"cli_nimmt_an": None, "ausgang": None}, "system_bwrap": "ubuntu", "ubuntu_profil": "geladen", "voraussetzung": "konfiguration.fehlt",
             "schritte": [{"schritt": s, "bestanden": False, "fehlercode": s.lower() + ".nicht-gelaufen", "dauer_ms": 0, "offen": []} for s in probelauf.SCHRITTE],
             "bestanden": False}
        self.assertEqual(probelauf.pruefe_form(d), [])
        for falsch in ("Ubuntu", "ja", None, True, ""):
            d["system_bwrap"] = falsch
            self.assertIn("system_bwrap", probelauf.pruefe_form(d), repr(falsch))
        del d["system_bwrap"]
        self.assertNotEqual(probelauf.pruefe_form(d), [])


PROFIL_GELADEN = "pruef.hinweis_ubuntu_profil_geladen"
PROFIL_ENTLADEN = "pruef.hinweis_ubuntu_profil_entladen"
ZEILE_GELADEN = PROFIL_GELADEN + ": Das Ubuntu-Profil bwrap-userns-restrict ist geladen."
ZEILE_ENTLADEN = PROFIL_ENTLADEN + ": Das Ubuntu-Profil bwrap-userns-restrict ist nicht geladen."
# dieselben Praefixe wie in konfig.probelauf.json (Entscheidung vom 20.11.2026): `gesperrt` hängt am Praefix der entladenen Zeile
PRAEFIX_PROFIL = dict(PRAEFIX, bwrap_praefix_gesperrt=PROFIL_ENTLADEN, profil_praefix_geladen=PROFIL_GELADEN,
                      profil_praefix_entladen=PROFIL_ENTLADEN)


def lage_alle(ausgabe, exitcode=0, **mehr):
    """Alle Lagefelder aus einer Ausgabe (system_bwrap und ubuntu_profil), kein zweiter Aufruf."""
    l = {}
    v = {"code": "copilot.nicht-angemeldet", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
         "befehl": [sys.executable, "-I", "-B", "-c", ATTRAPPE, ausgabe, str(exitcode)]}
    v.update(PRAEFIX_PROFIL)
    v.update(mehr)
    probelauf._zeilen_befund(v, l)
    return l


class UbuntuProfil(unittest.TestCase):
    """Lage des Ubuntu-Profils (Entscheidung vom 20.11.2026): aus denselben Zeilen wie `system_bwrap`, nur der Wert bleibt."""

    def test_geladen_mit_ubuntu_hinweis(self):
        l = lage_alle("pruef.ok: x|" + HINWEIS_UBUNTU + "|" + ZEILE_GELADEN + "|")
        self.assertEqual((l["ubuntu_profil"], l["system_bwrap"]), ("geladen", "ubuntu"))

    def test_entladen_ohne_ubuntu_hinweis(self):
        l = lage_alle("pruef.ok: x|" + ZEILE_ENTLADEN + "|")
        self.assertEqual((l["ubuntu_profil"], l["system_bwrap"]), ("entladen", "gesperrt"))

    def test_keine_hinweiszeile(self):
        l = lage_alle("pruef.ok: x|")
        self.assertEqual((l["ubuntu_profil"], l["system_bwrap"]), ("unbekannt", "unbekannt"))

    def test_beide_profilzeilen(self):
        self.assertEqual(lage_alle("pruef.ok: x|" + ZEILE_GELADEN + "|" + ZEILE_ENTLADEN + "|")["ubuntu_profil"], "unbekannt")

    def test_praefix_null_meldet_nie(self):
        self.assertEqual(lage_alle("pruef.ok: x|" + ZEILE_GELADEN + "|", profil_praefix_geladen=None)["ubuntu_profil"], "unbekannt")

    def test_befehl_nicht_startbar(self):
        l = {}
        probelauf._zeilen_befund({"code": "c", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
                                  "befehl": ["/nicht/vorhanden"], **PRAEFIX_PROFIL}, l)
        self.assertEqual(l["ubuntu_profil"], "unbekannt")

    def test_form_prueft_das_feld(self):
        d = _form_datei()
        self.assertEqual(probelauf.pruefe_form(d), [])
        for falsch in ("Geladen", "ubuntu", "ja", None, True, ""):
            d["ubuntu_profil"] = falsch
            self.assertIn("ubuntu_profil", probelauf.pruefe_form(d), repr(falsch))
        del d["ubuntu_profil"]
        self.assertTrue(any("ubuntu_profil" in m for m in probelauf.pruefe_form(d)))


def _form_datei():
    """Eine gueltige Ergebnisdatei (Form), mit dem Lagefeld `ubuntu_profil`."""
    return {"fassung": 2, "art": "probelauf-ergebnis", "beginn": "2026-11-20T10:00:00Z", "ende": "2026-11-20T10:01:00Z",
            "versionen": {"paket": "1", "laeufer": "1", "copilot_cli": "1", "ubuntu": "26.04", "kernel": "7"}, "copilot_starts": 0, "cloud_eingaben": 0, "umfang": "voll", "kennzeichnung": None, "s1_gegenprobe": {"weg": None, "neue_sitzungen": None, "neue_kinder": None},
            "browser": "pruefclient", "ca": {"cli_nimmt_an": None, "ausgang": None}, "system_bwrap": "ubuntu",
            "ubuntu_profil": "geladen", "voraussetzung": "konfiguration.fehlt",
            "schritte": [{"schritt": s, "bestanden": False, "fehlercode": s.lower() + ".nicht-gelaufen", "dauer_ms": 0, "offen": []} for s in probelauf.SCHRITTE],
            "bestanden": False}


class KonfigProbelauf(unittest.TestCase):
    """Die ausgelieferte Konfiguration traegt die Praefixe des Profils (Entscheidung vom 20.11.2026)."""

    def test_voraussetzung_traegt_profil(self):
        import json
        with open(os.path.join(HIER, "konfig.probelauf.json"), encoding="utf-8") as f:
            k = json.load(f)
        v = [x for x in k["probelauf"]["voraussetzungen"] if x["code"] == "copilot.nicht-angemeldet"][0]
        self.assertEqual({n: v.get(n) for n in ("bwrap_praefix_ubuntu", "bwrap_praefix_gesperrt", "profil_praefix_geladen",
                                                "profil_praefix_entladen")},
                         {"bwrap_praefix_ubuntu": PROFIL_GELADEN, "bwrap_praefix_gesperrt": PROFIL_ENTLADEN,
                          "profil_praefix_geladen": PROFIL_GELADEN, "profil_praefix_entladen": PROFIL_ENTLADEN})

    def test_als_nutzer_ohne_ubuntu_hinweis(self):
        """Als Nutzer ohne sudo fehlt `pruef.hinweis_system_bwrap_ubuntu`; die Profilzeile kommt aus derselben Probe des
        System-bwrap (Entscheidung vom 20.11.2026). Mit den Praefixen der Konfiguration ergibt sie `ubuntu` bzw. `gesperrt`."""
        import json
        with open(os.path.join(HIER, "konfig.probelauf.json"), encoding="utf-8") as f:
            k = json.load(f)
        v = [x for x in k["probelauf"]["voraussetzungen"] if x["code"] == "copilot.nicht-angemeldet"][0]
        nutzer_geladen = [PROFIL_GELADEN + ": x", "pruef.apparmor_nicht_pruefbar: y", "pruef.rot: z"]
        nutzer_entladen = [PROFIL_ENTLADEN + ": x", "pruef.apparmor_nicht_pruefbar: y", "pruef.rot: z"]
        self.assertEqual(probelauf.bwrap_lage(nutzer_geladen, v), "ubuntu")
        self.assertEqual(probelauf.bwrap_lage(nutzer_entladen, v), "gesperrt")
        self.assertEqual(probelauf.bwrap_lage(["pruef.apparmor_nicht_pruefbar: y", "pruef.rot: z"], v), "unbekannt")


class ExitcodeUnveraendert(unittest.TestCase):
    """Der Exitcode des Probelaufs ist mit und ohne Hinweiszeile gleich; nur das Feld `system_bwrap` unterscheidet sich."""

    def lauf(self, ausgabe, praefix=None):
        import json
        import subprocess
        import tempfile
        with tempfile.TemporaryDirectory(prefix="sb-") as t:
            with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
                k = json.load(f)
            v = {"code": "copilot.nicht-angemeldet", "art": "befehl_zeilen", "verboten_praefix": "pruef.copilot_",
                 "befehl": [sys.executable, "-I", "-B", "-c", ATTRAPPE, ausgabe, "0"]}
            v.update(PRAEFIX if praefix is None else praefix)
            k.setdefault("probelauf", {})["voraussetzungen"] = [v]
            kp, ep = os.path.join(t, "k.json"), os.path.join(t, "e.json")
            with open(kp, "w", encoding="utf-8") as f:
                json.dump(k, f)
            p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--konfig", kp, "--nur", "S1",
                                "--ergebnis", ep], capture_output=True, timeout=120)
            with open(ep, encoding="utf-8") as f:
                return p.returncode, json.load(f)

    def test_exit_gleich(self):
        e1, d1 = self.lauf("pruef.ok: x|" + HINWEIS_UBUNTU + "|")
        e2, d2 = self.lauf("pruef.ok: x|")
        self.assertEqual((d1["system_bwrap"], d2["system_bwrap"]), ("ubuntu", "unbekannt"))
        self.assertEqual(e1, e2)
        self.assertEqual(d1["bestanden"], d2["bestanden"])
        self.assertEqual([s["fehlercode"] for s in d1["schritte"]], [s["fehlercode"] for s in d2["schritte"]])

    def test_exit_gleich_bei_geladen_und_entladen(self):
        e1, d1 = self.lauf("pruef.ok: x|" + ZEILE_GELADEN + "|", PRAEFIX_PROFIL)
        e2, d2 = self.lauf("pruef.ok: x|" + ZEILE_ENTLADEN + "|", PRAEFIX_PROFIL)
        self.assertEqual((d1["ubuntu_profil"], d2["ubuntu_profil"]), ("geladen", "entladen"))
        self.assertEqual((d1["system_bwrap"], d2["system_bwrap"]), ("unbekannt", "gesperrt"))
        self.assertEqual(e1, e2)
        self.assertEqual(d1["bestanden"], d2["bestanden"])
        self.assertEqual([s["fehlercode"] for s in d1["schritte"]], [s["fehlercode"] for s in d2["schritte"]])


if __name__ == "__main__":
    unittest.main()
