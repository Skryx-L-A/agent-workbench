"""Zug 2 des knappen Cloud-Laufs als Shell-Eingabe der CLI, CA-Vorpruefung vor Zug 2 und fruehe Erkennung einer Antwort ohne Merkmal.

1. Form (Kriterium i): `prompts.buendel` beginnt in beiden Konfigurationen mit `!`. Die CLI fuehrt eine solche Eingabe selbst in ihrer Shell
   aus und fragt das Modell nicht; das Modell bekommt weder das Buendel noch einen Auftrag. `konfig.lade` weist eine andere Form ab (auch
   die alte Prosa-Vorlage). Gegen die Attrappen zaehlt der Weiterreicher im Zug des Buendels 0 Anfragen.
2. Dieselben Pruefungen (Kriterien ii und iii): das Buendel ist unveraendert (11 Zeilen, dieselben Befehle), es laeuft in der Shell der
   Cloud-Sitzung; S1 und E4-cloud haben ihre Teile wie vorher; dazu kommt ein Teil (Kundenmarker 0 am Weiterreicher im Zug des Buendels).
3. Fruehe Erkennung: Antwortet die CLI ohne `FERTIG T<n>` (Fehler der Attrappe `shell-eingabe-an-modell`: eine CLI ohne Shell-Eingabe gibt
   den Text an das Modell, das ohne Merkmal antwortet), endet Zug 2 nach `grenzen.still_s` mit `s1.antwort_ohne_merkmal` statt nach der
   Frist mit `s1.zug_frist`. Die Diagnosedatei traegt `verlauf_fertig=<n> raster_fertig=<n>`.
4. CA-Vorpruefung direkt nach Zug 1: Im Fall 3 ist `ca` gemessen, und E4-cloud meldet `['e4-cloud.offen']` statt `ca`.

Die Gegenprobe mit dem Laeufer vor dieser Aenderung: dieselben Tests werden rot.
Nur Standardbibliothek. Aufruf: python3 -I -B test_zug2_shell.py   (startet die Attrappen, ueber die Tafel)
"""
import json
import os
import sys
import time
import unittest

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, konfig, lauf, probelauf, schritte  # noqa: E402
from test_positivkontrollen import Werkstatt, lade_konfig, schreibe  # noqa: E402

ALTE_VORLAGE = ("Kennung {np}, Zug T{n}. Führe in der Shell genau einmal dieses Skript aus, ohne es zu ändern: `{arg}` Gib danach die "
                "vollständige Ausgabe unverändert in einem Codeblock wieder, ohne Kommentar. Schreibe zuletzt eine eigene Zeile aus dem Wort "
                "FERTIG, einem Leerzeichen und T{n}.")
# Zaehlzeile der Diagnose bei einem Zug ohne Merkmal: nur Zahlen (Zeiten in Sekunden seit Beginn des Laufs, Zaehler oder `-`)
ZUGZEILE = (r"(?m)^lauf\.zug: zug=%d beginn_s=\d+\.\d{3} enter_s=\d+\.\d{3} ende_s=\d+\.\d{3} still_s=\d+\.\d "
            r"weiterreicher_anfragen=(\d+|-) weiterreicher_antworten=(\d+|-) verlauf_zeilen=\d+$")
# Woerter eines Auftrags an das Modell; keins davon darf im Text von Zug 2 stehen
AUFTRAG = ("Führe", "Fuehre", "Gib ", "Schreibe", "Kennung", "Skript", "ausführen", "execute", "run ")


class Form(unittest.TestCase):
    def test_vorlage_ist_shell_eingabe_in_beiden_konfigurationen(self):
        for name in ("konfig.probelauf.json", "konfig.attrappe.json"):
            v = lade_konfig(name)["prompts"]["buendel"]
            self.assertTrue(v.startswith(konfig.SHELL_EINGABE), name)
            text = v.format(n=2, arg="z=$(true); echo \"x=$z RC=$?\"", np="MP-2C5")
            for w in AUFTRAG:
                self.assertNotIn(w, text, "%s: kein Auftrag an das Modell im Text von Zug 2" % name)
            self.assertNotIn("MP-2C5", text, "kein Marker: Zug 2 ist keine Modellanfrage")
            self.assertIsNone(lauf.TOKEN(2).search(text), "der Text traegt das Merkmal nicht woertlich, erst die Shell schreibt es")
            self.assertTrue(text.endswith('; echo "FERTIG T$((2))"'))

    def test_konfig_weist_andere_form_ab(self):
        for vorlage in (ALTE_VORLAGE, "#T{n} {np} BUENDEL {arg}", " !{arg}", 7):
            k = lade_konfig()
            k["prompts"]["buendel"] = vorlage
            pfad = os.path.join(self.tmp, "k.json")
            schreibe(pfad, k)
            with self.assertRaises(konfig.KonfigFehler):
                konfig.lade(pfad, HIER, self.tmp)

    def test_buendel_unveraendert_elf_zeilen(self):
        k = konfig.lade(os.path.join(HIER, "konfig.probelauf.json"), HIER, self.tmp)

        class L_:
            pass
        L_.k, L_.cloud = k, k["routen"]["cloud"]
        b = schritte.cloud_buendel(L_)
        self.assertEqual(b.count('echo "'), 11)
        for befehl in ("gh auth token", "secret-tool lookup", "/proc/[0-9]*/environ", "grep -rIhoE"):
            self.assertIn(befehl, b, "die Pruefung bleibt im Buendel: sie laeuft in der Shell der Sitzung, nur nicht als Auftrag an das Modell")

    def test_neuer_code_in_der_festen_liste(self):
        self.assertIn("s1.antwort_ohne_merkmal", probelauf.FEHLERCODES)
        self.assertEqual(ablauf.ausnahme_code("S1", lauf.AntwortOhneMerkmal("x")), "s1.antwort_ohne_merkmal")
        self.assertEqual(ablauf.ausnahme_code("S1", lauf.Zeitueberschreitung("x")), "s1.zug_frist")

    def setUp(self):
        import tempfile
        self.tmp = tempfile.mkdtemp(prefix="zug2-")

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)


class Stillstand(unittest.TestCase):
    """`lauf._Stillstand`: wahr erst nach `still_s` gleichem Bild, ohne den Text im Feld und ohne Arbeitsanzeige."""

    def neu(self, muster=None):
        s = lauf._Stillstand(0.2, lauf.merkmal("!z=$(true); echo \"FERTIG T$((2))\""), muster)
        return s

    def test_still_feld_leer(self):
        s = self.neu()
        bild = ["antwort", "❯ ", "─" * 10]
        self.assertFalse(s.pruefe(bild), "das erste Bild setzt nur den Anfang")
        time.sleep(0.25)
        self.assertTrue(s.pruefe(bild))

    def test_text_noch_im_feld(self):
        s = self.neu()
        bild = ["❯ !z=$(true); echo \"FERTIG T$((2))\"", "─" * 10]
        s.pruefe(bild)
        time.sleep(0.25)
        self.assertFalse(s.pruefe(bild), "der Text steht noch im Feld: nicht abgesendet, kein Fall der fruehen Erkennung")

    def test_ohne_feld_nicht_feststellbar(self):
        s = self.neu()
        bild = ["antwort ohne Feld"]
        s.pruefe(bild)
        time.sleep(0.25)
        self.assertFalse(s.pruefe(bild))

    def test_arbeitsanzeige(self):
        s = self.neu("(?i)esc to|thinking|running")
        bild = ["Thinking (Esc to cancel)", "❯ ", "─" * 10]
        s.pruefe(bild)
        time.sleep(0.25)
        self.assertFalse(s.pruefe(bild))

    def test_bild_aendert_sich(self):
        s = self.neu()
        s.pruefe(["a", "❯ "])
        time.sleep(0.25)
        self.assertFalse(s.pruefe(["b", "❯ "]), "ein neues Bild setzt den Anfang neu")

    def test_aus_ohne_still_s(self):
        self.assertFalse(lauf._Stillstand(None, "x", None).aktiv)


class Zug2Knapp(Werkstatt):
    def konfig_knapp(self, name):
        k = lade_konfig()
        k.setdefault("probelauf", {})["cloud_eingaben_max"] = 2
        return schreibe(os.path.join(self.ordner, "konfig-knapp-%s.json" % name), k)

    def test_gruen_ohne_modellanfrage_im_zug_des_buendels(self):
        _, s, d, text = self.knapp("gruen")
        self.assertEqual(d["cloud_eingaben"], 2)
        self.assertTrue(s["S1"]["bestanden"], s["S1"])
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.nur_attrappe", "wie vorher: im knappen Lauf gegen die Attrappen")
        self.assertTrue(any(z.startswith("  E4-cloud teile: ") and z.endswith(", 0 rot") for z in text.splitlines()), "kein roter Teil")
        self.assertEqual(d["ca"], {"cli_nimmt_an": True, "ausgang": True})
        self.assertEqual(probelauf.pruefe_form(d), [])

    def test_bericht_teile_und_messwerte(self):
        _, b = self.bericht("bericht", konfig_pfad=self.konfig_knapp("bericht"), nur="S1,E4-cloud")
        s1, e4 = b["S1"], b["E4-cloud"]
        self.assertEqual(s1["ergebnis"], "gruen")
        namen = [t["name"] for t in e4["teile"]]
        self.assertIn("Kein Treffer des Tokens ausser dem Platzhalter: Umgebung, Argumente, Prozesse, Dateien, gh, secret-tool", namen)
        self.assertIn("Am Weiterreicher der Cloud-Route: Marker der Kunden 0 (Zaehldatei des Weiterreichers)", namen)
        neu = "Am Weiterreicher im Zug des Buendels: Marker der Kunden 0"
        # letzter eigener Teil von E4-cloud (davor bleiben Reihenfolge und Nummern wie vorher); danach haengt `suche_nachtrag` die zwei Teile
        # der Schluesselsuche an
        self.assertEqual(namen.index(neu), len(namen) - 3, namen)
        self.assertTrue(namen[namen.index(neu) - 1].startswith("Ausserhalb: das Ziel lehnt ohne Token ab"))
        self.assertTrue(namen[-1].startswith("Gegenprobe: ein Verweis auf die Schluesseldatei"))
        self.assertEqual(e4["messwerte"]["buendel_anfragen_weiterreicher"], 0, "Zug 2 ging nicht an das Modell")
        self.assertGreaterEqual(e4["messwerte"]["kontrolle_platzhalter"], 1)
        self.assertGreaterEqual(e4["messwerte"]["kontrolle_proc"], 1)
        self.assertTrue(all(v == 0 for v in e4["messwerte"]["treffer"].values()))
        self.assertEqual(len(e4["messwerte"]["treffer"]), 6)

    def test_token_lesbar_bleibt_rot(self):
        """Gegenprobe: liegt das echte Token in der Umgebung, findet die Shell-Eingabe es wie vorher das Werkzeug des Modells."""
        _, s, d, _ = self.knapp("token", fehler="e4-cloud-token-lesbar")
        self.assertFalse(s["E4-cloud"]["bestanden"])
        self.assertTrue(s["E4-cloud"]["fehlercode"].startswith("e4-cloud.teil"), s["E4-cloud"]["fehlercode"])

    def test_antwort_ohne_merkmal_frueh_und_e4_cloud_offen(self):
        t0 = time.monotonic()
        rc, s, d, text = self.knapp("ohne-merkmal", fehler="shell-eingabe-an-modell")
        dauer = time.monotonic() - t0
        self.assertNotEqual(rc, 0)
        self.assertEqual(s["S1"]["fehlercode"], "s1.antwort_ohne_merkmal")
        self.assertLess(s["S1"]["dauer_ms"], 20000, "vor der Frist von Zug 2 (baum_s 30 s)")
        self.assertEqual(s["E4-cloud"]["offen"], ["e4-cloud.offen"])
        self.assertEqual(s["E4-cloud"]["fehlercode"], "e4-cloud.offen")
        self.assertEqual(d["ca"], {"cli_nimmt_an": True, "ausgang": True}, "die CA-Vorpruefung lief nach Zug 1, vor Zug 2")
        self.assertEqual(probelauf.pruefe_form(d), [])
        erg = os.path.join(self.ordner, "ergebnis-ohne-merkmal.json.diagnose")
        with open(erg, encoding="utf-8") as f:
            diag = f.read()
        self.assertIn("verlauf_fertig=0 raster_fertig=0", diag)
        self.assertRegex(diag, ZUGZEILE % 2)
        self.assertLess(dauer, 120)

    def test_ablehnung_429_im_zug_1(self):
        """Der Modellweg lehnt ab (wie `mw.ausgelastet`), die CLI zeigt einen Fehler ohne Merkmal: Zug 1 endet frueh, die CA-Vorpruefung lief
        nicht (null), E4-cloud meldet das fehlende Buendel. Zug 2 ist als Shell-Eingabe keine Modellanfrage und kann so nicht abgelehnt werden."""
        rc, s, d, _ = self.knapp("429", fehler="modell-429")
        self.assertNotEqual(rc, 0)
        self.assertEqual(d["cloud_eingaben"], 1)
        self.assertEqual(s["S1"]["fehlercode"], "s1.antwort_ohne_merkmal")
        self.assertLess(s["S1"]["dauer_ms"], 10000, "vor der Frist von Zug 1 (turn_s 10 s)")
        self.assertEqual(s["E4-cloud"]["offen"], ["e4-cloud.offen"])
        self.assertEqual(d["ca"], {"cli_nimmt_an": None, "ausgang": None})
        with open(os.path.join(self.ordner, "ergebnis-429.json.diagnose"), encoding="utf-8") as f:
            diag = f.read()
        self.assertRegex(diag, ZUGZEILE % 1)
        self.assertRegex(diag, r"weiterreicher_anfragen=0 ")


if __name__ == "__main__":
    unittest.main()
