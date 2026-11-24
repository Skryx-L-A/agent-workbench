"""Tests fuer Enter mit Abstand (Entscheidung vom 22.11.2026): `Lauf.zug` sendet erst den Text, wartet, bis er im Raster steht,
wartet dann `ENTER_ABSTAND_S` und sendet erst dann Enter. Danach prueft der Lauf, ob die CLI abgesendet hat (der Text steht nicht mehr
im Eingabefeld). Sonst endet der Zug mit `<schritt>.nicht_abgesendet`, das Bild geht in die Diagnosedatei, und es gibt kein zweites Enter.

Gegenprobe (ohne Warten rot): Mit `ENTER_ABSTAND_S` 0 schluckt die nachgebildete CLI das Enter als Zeilenumbruch, und der Zug endet mit
`nicht_abgesendet`. Die Nachbildung des Rennens ist hergeleitet (Kandidat a): gemessen ist nur, dass die echte CLI 1.0.88 Text und Enter
direkt hintereinander einmal nicht absandte und mit 300 ms Abstand zweimal absandte (Messung in zwei Testlaeufen).

Nur Standardbibliothek, ohne Netz und ohne Prozess. Aufruf: python3 -I -B test_enter_abstand.py   (etwa 10 s: ueber die Tafel)
"""
import os
import shutil
import sys
import tempfile
import time
import unittest
from unittest import mock

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)  # `python3 -I` nimmt den Ordner des Skripts nicht auf den Suchpfad

from drehbuchlib import ablauf, bruecke as br, konfig, lauf, messen, probelauf, seite  # noqa: E402
from test_einstart import FalscheBruecke  # noqa: E402

LINIE = "─" * 40
FUSS = "  / commands · ? help"


class NachgebildeteCli(FalscheBruecke):
    """Eine Bruecke mit den Ereignissen `Text` und `Taste` und einem Raster wie die echte CLI: das Feld `❯` zwischen zwei Linien.

    `art`:
      - "cli": der Text erscheint nach `zeigen_s` im Feld; Enter sendet ab, wenn es mindestens `rennen_s` nach dem Erscheinen kommt,
        sonst wird es ein Zeilenumbruch im Feld (Kandidat a, nachgebildet). Abgesendet steht der Text als Zeile im Verlauf (mit `❯`
        davor, damit die Suche nach dem letzten Feld geprueft ist), das Feld ist leer.
      - "bleibt": Enter aendert nichts, der Text bleibt im Feld.
      - "nie": der Text erscheint nie.
      - "attrappe": wie `attrappen/copilot_attrappe.py` hinter der echten Bruecke: das Feld bleibt leer, das Echo steht unter der Fusszeile.
    """

    text_und_taste = True

    def __init__(self, art="cli", zeigen_s=0.1, rennen_s=0.25, kasten=False):
        super().__init__()
        self.art, self.zeigen_s, self.rennen_s, self.kasten = art, zeigen_s, rennen_s, kasten
        self.ereignisse = []  # (art, monotone Zeit)
        self.getippt, self.getippt_t, self.gesehen_t = "", None, None
        self.verlaufzeilen, self.umbruch = [], False

    def text(self, sid, text):
        self.ereignisse.append(("Text", time.monotonic()))
        self.getippt, self.getippt_t, self.umbruch = text, time.monotonic(), False
        return 202, {"ok": True}

    def taste(self, sid, taste="Enter"):
        jetzt = time.monotonic()
        self.ereignisse.append(("Taste", jetzt))
        if self.art == "cli" and self._zu_sehen(jetzt):
            if self.gesehen_t is not None and jetzt - self.gesehen_t >= self.rennen_s:
                self.verlaufzeilen.append("❯ " + self.getippt)
                self.getippt = ""
            else:
                self.umbruch = True
        return 202, {"ok": True}

    def eingabe(self, sid, text):  # der alte Weg: Text und Enter direkt hintereinander
        self.eingaben.append((sid, text))
        self.text(sid, text)
        return self.taste(sid)

    def _zu_sehen(self, jetzt):
        return self.art != "nie" and self.getippt_t is not None and jetzt - self.getippt_t >= self.zeigen_s

    def _feld(self, text):
        if not self.kasten:
            return ["❯ " + text]
        teile = [text[i:i + 30] for i in range(0, max(len(text), 1), 30)]  # umbrochen in einem Kasten mit Rand
        return ["│ " + ("❯ " if i == 0 else "  ") + t + " │" for i, t in enumerate(teile)]

    def raster(self, sid):
        jetzt = time.monotonic()
        zu_sehen = self._zu_sehen(jetzt) and self.getippt
        if zu_sehen and self.gesehen_t is None:
            self.gesehen_t = jetzt
        oben = ["Copilot CLI (nachgebildet)", ""] + self.verlaufzeilen + [""]
        if self.art == "attrappe":
            return {"zeilen": oben + [LINIE, "❯ ", LINIE, FUSS] + ([self.getippt] if zu_sehen else []), "cursor": [0, 0, True]}
        feld = self._feld(self.getippt if zu_sehen else "")
        if self.umbruch:
            feld = feld + [""]  # das geschluckte Enter ist eine leere Zeile im Feld; der Text steht weiter darin
        return {"zeilen": oben + [LINIE] + feld + [LINIE, FUSS], "cursor": [0, 0, True]}


def lauf_mit(arbeit, b):
    k = konfig.lade(os.path.join(HIER, "konfig.attrappe.json"), HIER, arbeit)
    for a in ("cloud", "lokal"):
        k["modell"][a]["zaehlen"] = False
    L = lauf.Lauf(k, b, {"cloud": None, "lokal": None}, None, arbeit)
    L.art_von["s-1"] = "cloud"
    return L


class EnterMitAbstand(unittest.TestCase):
    def setUp(self):
        self.arbeit = tempfile.mkdtemp(prefix="enter-abstand-")
        self.addCleanup(shutil.rmtree, self.arbeit, True)
        messen.diagnose_leeren()
        self.addCleanup(messen.diagnose_leeren)

    def test_enter_erst_nach_dem_abstand_und_abgesendet(self):
        b = NachgebildeteCli("cli")
        L = lauf_mit(self.arbeit, b)
        L.zug("s-1", "zaehlen", "")
        self.assertEqual([a for a, _ in b.ereignisse], ["Text", "Taste"], "genau ein Text und ein Enter")
        enter_t = b.ereignisse[1][1]
        self.assertIsNotNone(b.gesehen_t)
        self.assertGreaterEqual(enter_t - b.gesehen_t, lauf.ENTER_ABSTAND_S - 0.01, "Enter erst ENTER_ABSTAND_S nach dem sichtbaren Text")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": True})
        self.assertEqual((L.cloud_eingaben, b.eingaben), (1, []), "gezaehlt; der alte Weg `eingabe` lief nicht")
        self.assertIsNone(messen.diagnose_schreibe(os.path.join(self.arbeit, "e.json")), "ohne Halt keine Diagnosedatei")

    def test_fuenf_zuege_fuenf_abgesendet(self):
        b = NachgebildeteCli("cli")
        L = lauf_mit(self.arbeit, b)
        for _ in range(5):
            L.zug("s-1", "zaehlen", "")
            self.assertIs(L.sendung["abgesendet"], True)
        self.assertEqual(len(b.verlaufzeilen), 5)

    def test_langer_text_im_kasten_umbrochen(self):
        b = NachgebildeteCli("cli", kasten=True)
        L = lauf_mit(self.arbeit, b)
        L.zug("s-1", "buendel", "", vorlage="Bitte fuehre aus: " + "echo eins; " * 40 + "Ende T{n}")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": True})

    def test_gegenprobe_ohne_warten_rot(self):
        """Ohne Abstand schluckt die nachgebildete CLI das Enter: der Zug endet mit `nicht_abgesendet`, kein zweites Enter."""
        b = NachgebildeteCli("cli")
        L = lauf_mit(self.arbeit, b)
        with mock.patch.object(lauf, "ENTER_ABSTAND_S", 0.0):
            with self.assertRaises(lauf.NichtAbgesendet) as f:
                L.zug("s-1", "zaehlen", "")
        self.assertEqual([a for a, _ in b.ereignisse], ["Text", "Taste"], "kein zweites Enter")
        self.assertEqual(ablauf.ausnahme_code("S1", f.exception), "s1.nicht_abgesendet")
        self.assertEqual(L.cloud_eingaben, 1, "das Enter ging hinaus: es zaehlt")

    def test_gegenprobe_alter_weg_sendet_nicht_ab(self):
        """Der alte Weg (`eingabe`, Text und Enter direkt) laesst den Text im Feld der nachgebildeten CLI stehen."""
        b = NachgebildeteCli("cli")
        b.eingabe("s-1", "hallo T1")
        time.sleep(0.2)
        self.assertIs(lauf.im_eingabefeld(b.raster("s-1")["zeilen"], lauf.merkmal("hallo T1")), True)
        self.assertEqual(b.verlaufzeilen, [])

    def test_text_bleibt_im_feld(self):
        b = NachgebildeteCli("bleibt")
        L = lauf_mit(self.arbeit, b)
        with mock.patch.object(lauf, "ABGESENDET_S", 0.5):
            with self.assertRaises(lauf.NichtAbgesendet) as f:
                L.zug("s-1", "zaehlen", "")
        self.assertEqual(ablauf.ausnahme_code("S1", f.exception), "s1.nicht_abgesendet")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": False})
        self.assertEqual([a for a, _ in b.ereignisse], ["Text", "Taste"], "kein zweites Enter")
        pfad = os.path.join(self.arbeit, "e.json")
        self.assertEqual(messen.diagnose_schreibe(pfad), "angelegt")
        with open(pfad + ".diagnose", encoding="utf-8") as f:
            inhalt = f.read()
        self.assertIn("lauf.zug: Zug 1 nicht abgesendet: sichtbar=True abgesendet=False", inhalt)
        self.assertIn("lauf.raster: zeile ", inhalt, "das Bild steht in der Diagnosedatei")
        self.assertEqual(oct(os.stat(pfad + ".diagnose").st_mode & 0o777), "0o600")

    def test_text_nie_sichtbar_kein_enter(self):
        b = NachgebildeteCli("nie")
        L = lauf_mit(self.arbeit, b)
        with mock.patch.object(lauf, "EINGABE_SICHTBAR_S", 0.5):
            with self.assertRaises(lauf.NichtAbgesendet):
                L.zug("s-1", "zaehlen", "")
        self.assertEqual([a for a, _ in b.ereignisse], ["Text"], "ohne sichtbaren Text kein Enter")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": False, "abgesendet": None})
        self.assertEqual(L.cloud_eingaben, 0, "ohne Enter keine Eingabe an die Cloud gezaehlt")

    def test_ablehnung_des_textes_bleibt_ablehnung(self):
        class Ablehnend(NachgebildeteCli):
            def ablehnungen_seit(self, marke):
                return [("steuerzeichen_im_text", "")] if self.ereignisse else []
        b = Ablehnend("nie")
        L = lauf_mit(self.arbeit, b)
        t0 = time.monotonic()
        with self.assertRaises(lauf.Zeitueberschreitung) as f:
            L.zug("s-1", "zaehlen", "")
        self.assertLess(time.monotonic() - t0, 2.0, "die Ablehnung beendet das Warten auf den Text")
        self.assertEqual(str(f.exception), "Eingabe vom Dienst abgelehnt")
        self.assertEqual([a for a, _ in b.ereignisse], ["Text"])

    def test_attrappen_bild_text_unter_der_fusszeile(self):
        b = NachgebildeteCli("attrappe")
        L = lauf_mit(self.arbeit, b)
        L.zug("s-1", "zaehlen", "")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": True})

    def test_ohne_feld_nicht_feststellbar_kein_halt(self):
        class OhneFeld(NachgebildeteCli):
            def raster(self, sid):
                return {"zeilen": ["SIM-EINGABE >" + self.getippt]}
        b = OhneFeld("cli")
        L = lauf_mit(self.arbeit, b)
        t0 = time.monotonic()
        L.zug("s-1", "zaehlen", "")
        self.assertLess(time.monotonic() - t0, 2.0, "ohne Feld kein Warten bis ABGESENDET_S")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": None})
        self.assertIsNone(messen.diagnose_schreibe(os.path.join(self.arbeit, "e.json")), "nicht feststellbar: keine Diagnosedatei")

    def test_bruecke_ohne_ereignisse_alter_weg(self):
        b = FalscheBruecke()
        L = lauf_mit(self.arbeit, b)
        L.zug("s-1", "zaehlen", "")
        self.assertEqual((len(b.eingaben), L.sendung["weg"]), (1, "eingabe"))

    def test_aufzeichnung_reicht_den_neuen_weg_durch(self):
        """Im Probelauf haelt `Aufzeichnung` die Bruecke: `text_und_taste` kommt ueber `__getattr__` von der Bruecke."""
        b = NachgebildeteCli("cli")
        gesehen = []
        L = lauf_mit(self.arbeit, lauf.Aufzeichnung(b, gesehen))
        L.zug("s-1", "zaehlen", "")
        self.assertEqual(L.sendung, {"weg": "abstand", "sichtbar": True, "abgesendet": True})
        self.assertTrue(gesehen, "die Bilder des Wartens gehen in die Aufzeichnung")

    def test_seite_bleibt_beim_eigenen_weg(self):
        self.assertIs(br.Bruecke.text_und_taste, True)
        self.assertIs(seite.SeiteBruecke.text_und_taste, False, "die Seite sendet ueber ihr Formular")

    def test_feste_codes(self):
        self.assertEqual(ablauf.ausnahme_code("S1", lauf.NichtAbgesendet("x")), "s1.nicht_abgesendet")
        for s in probelauf.SCHRITTE:
            self.assertIn("%s.nicht_abgesendet" % s.lower(), probelauf.FEHLERCODES)


class Eingabefeld(unittest.TestCase):
    def test_letztes_feld_zaehlt(self):
        z = [LINIE, "❯ alt T1", "", LINIE, "❯ ", LINIE, FUSS]
        self.assertIs(lauf.im_eingabefeld(z, lauf.merkmal("alt T1")), False)
        self.assertIs(lauf.im_eingabefeld([LINIE, "❯ neu T2", LINIE, FUSS], lauf.merkmal("neu T2")), True)
        self.assertIsNone(lauf.im_eingabefeld(["SIM-EINGABE >"], "x"))

    def test_kastenrand_und_umbruch(self):
        z = ["╭" + LINIE + "╮", "│ ❯ Bitte zaehle bis │", "│   drei. FERTIG T4 │", "╰" + LINIE + "╯"]
        self.assertIs(lauf.im_eingabefeld(z, lauf.merkmal("Bitte zaehle bis drei. FERTIG T4")), True)

    def test_merkmal(self):
        self.assertEqual(lauf.merkmal("a b\nc"), "abc")
        self.assertEqual(len(lauf.merkmal("x" * 100)), lauf.MERKMAL_ZEICHEN)


class AttrappeDerBruecke(unittest.TestCase):
    """Die Attrappe der Bruecke zeigt den Text beim Ereignis `Text` (wie die echte CLI) und macht beim Enter aus `❯` ein `›`. Den ganzen Weg
    ueber die Klasse `Bruecke` prueft der Lauf `drehbuch.py --ziel attrappe`."""

    class Verbindung:
        def __init__(self):
            self.abos, self.abgelehnt = {"s-1": {}}, []

        def ablehnung(self, grund):
            self.abgelehnt.append(grund)

    def kern(self, bereit=True):
        sys.path.insert(0, os.path.join(HIER, "attrappen"))
        import bruecke_attrappe as ba
        kern = ba.Kern.__new__(ba.Kern)
        kern.zuege = []
        kern._zug = lambda s, text: kern.zuege.append(text)  # kein Agent: nur, was ankam
        s = ba.Sitzung("s-1", {"harness": "copilot"}, None, {}, time.time() - (1 if bereit else -60), (60, 8))
        kern.sitzungen = {"s-1": s}
        if bereit:
            s.schreibe("Sim-Harness copilot\n")
            s.schreibe("\n\u276f ")
            s.feld_setzen()
        return kern, s, self.Verbindung()

    def test_echo_vor_dem_enter_und_abgeschickte_zeile(self):
        kern, s, v = self.kern()
        kern.ereignis(v, {"art": "Text", "sitzung": "s-1", "text": "Bitte zaehle. FERTIG T1"})
        bild = s.term.bild()
        self.assertIs(lauf.im_eingabefeld(bild, lauf.merkmal("Bitte zaehle. FERTIG T1")), True, "der Text steht vor dem Enter im Feld")
        self.assertEqual(kern.zuege, [])
        kern.ereignis(v, {"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": []})
        bild = s.term.bild()
        self.assertEqual(kern.zuege, ["Bitte zaehle. FERTIG T1"], "der Agent bekommt den ganzen Text einmal")
        self.assertIn("\u203a Bitte zaehle. FERTIG T1", bild, "abgeschickte Zeile")
        self.assertIsNone(lauf.im_eingabefeld(bild, lauf.merkmal("Bitte zaehle. FERTIG T1")), "kein offenes Feld mehr")
        self.assertEqual(sum(z.count("FERTIG T1") for z in bild), 1, "der Text steht genau einmal im Bild")
        self.assertEqual(v.abgelehnt, [])

    def test_text_in_stuecken_und_eingabe_ueber_den_hilfsweg(self):
        kern, s, v = self.kern()
        kern.ereignis(v, {"art": "Text", "sitzung": "s-1", "text": "eins "})
        kern.ereignis(v, {"art": "Text", "sitzung": "s-1", "text": "zwei"})
        kern.ereignis(v, {"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": []})
        self.assertEqual(kern.zuege, ["eins zwei"])
        self.assertIn("\u203a eins zwei", s.term.bild())
        kern.eingabe(s, "drei")  # Hilfsweg der Attrappen-Seite: der ganze Text kommt mit dem Enter
        self.assertEqual(kern.zuege, ["eins zwei", "drei"])
        self.assertIn("drei", s.term.bild())

    def test_vor_der_bereitschaft_kein_echo_und_ablehnung(self):
        kern, s, v = self.kern(bereit=False)
        kern.ereignis(v, {"art": "Text", "sitzung": "s-1", "text": "zu frueh"})
        self.assertFalse(any("zu frueh" in z for z in s.term.bild()))
        kern.ereignis(v, {"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": []})
        self.assertEqual((v.abgelehnt, kern.zuege), (["harness_nicht_bereit"], []))


if __name__ == "__main__":
    unittest.main(verbosity=2)
