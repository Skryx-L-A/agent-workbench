"""Prueft den Strom-Client des Laeufers (`drehbuchlib/bruecke.py`) gegen Rahmen, die nach den Vertraegen gebaut sind. Nur Standardbibliothek.
Aufruf: python3 -I -B test_strom.py

- Rahmen auf der Leitung (Vertrag dienst, Abschnitt 5, DI3): `[n: u32][art: u8][nutzlast]`, in beliebigen Stuecken, kaputte Laengen.
- Rasterrahmen (Vertrag rasterauszug, RA9 bis RA11): Beispielrahmen als Hex (Handgebaute Rahmen des Crates `rasterauszug`, Faelle
  `ra10_dek_gueltig` und `ra10_dek_schub`), mit und ohne Raw-Deflate (ein Kontext je Verbindung, Sync-Flush ohne die vier Endbytes), kaputte
  Rahmen ergeben einen `StromFehler` und lassen das Bild unveraendert.
- Verlauf aus den Rasterrahmen: ein Token `FERTIG T<n>` ueber zwei Rahmen, ueber zwei Zeilen und ueber eine umgebrochene Zeile geht nicht verloren
  und zaehlt genau einmal; ein Schub fuehrt nicht zu doppelten Zeilen.
- Der ganze Weg: ein Strom ueber einen Prueftisch-Server (nur Standardbibliothek), mit Kopf, `X-Deflate`, Rahmen byteweise geliefert,
  `Abo`, Text und Enter ueber `POST /eingabe`, Ablehnung im Strom, kaputter Rahmen.
"""
import json
import os
import re
import sys
import threading
import time
import unittest
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HIER = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HIER)
from drehbuchlib import bruecke as br  # noqa: E402
from drehbuchlib import lauf  # noqa: E402

# Beispielrahmen des Crates `rasterauszug` (Fall ra10_dek_gueltig): Vollbild 20x6 Sitzung 1, Schub mit Cursor und invers, eine Zeile wird leer
HEX_VOLL = "050104051406020200000000010300020005616c70686101012102010004626574610501010567616d6d61"
HEX_SCHUB = "05010b0600060101010101010000010501000564656c7461"
HEX_LEER = "0501000900010000"
# Fall ra10_dek_schub: Vollbild 10x8, danach Schub (1, 6, 2) mit einer Zeile
HEX_S_VOLL = ("050104010a08010000000800010006616c706861300101000562657461310201000667616d6d61320301000664656c74613304010008657073696c6f6e34"
              "050100057a65746135060100046574613607010006746865746137")
HEX_S_SCHUB = "050101020106020100000001050100036e6575"


def vu(n):
    aus = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            aus.append(b | 0x80)
        else:
            aus.append(b)
            return bytes(aus)


def rahmen(nr, stand, groesse=None, schub=None, cursor=None, zeilen=None, invers=False):
    """Ein Rasterrahmen nach der Form A-B des Vertrags rasterauszug (alle Laeufe mit Stil 0). `zeilen` ist {Index: Text}."""
    zeilen = zeilen or {}
    flags = (1 if schub else 0) | (2 if cursor else 0) | (4 if groesse else 0) | (8 if invers else 0)
    a = bytearray([5, nr, flags]) + vu(stand)
    if groesse:
        a += vu(groesse[0]) + vu(groesse[1])
    if schub:
        a += b"".join(vu(x) for x in schub)
    if cursor:
        a += vu(cursor[0]) + vu(cursor[1]) + vu(1 if cursor[2] else 0)
    a += (vu(1) + vu(0) + vu(0) + vu(0)) if any(zeilen.values()) else vu(0)
    a += vu(len(zeilen))
    for i in sorted(zeilen):
        t = zeilen[i].encode("utf-8")
        a += vu(i) + (vu(1) + vu(0) + vu(len(t)) + t if t else vu(0))
    return bytes(a)


def auf_leitung(art, nutz):
    return (1 + len(nutz)).to_bytes(4, "big") + bytes([art]) + nutz


class Packer:
    """Der Absender einer Verbindung mit Deflate: ein Kontext, jeder Rahmen mit Sync-Flush ohne `00 00 ff ff` (RA11)."""

    def __init__(self):
        self.c = zlib.compressobj(6, zlib.DEFLATED, -15)

    def pack(self, roh):
        d = self.c.compress(roh) + self.c.flush(zlib.Z_SYNC_FLUSH)
        assert d.endswith(b"\x00\x00\xff\xff")
        return d[:-4]


def anwenden(leser, hexfolge):
    return [leser.nimm(bytes.fromhex(h)) for h in hexfolge]


class Rahmen(unittest.TestCase):
    def test_der_test_encoder_gleicht_dem_crate(self):
        # Der Encoder dieses Tests gibt die Rahmen des Crates Byte fuer Byte wieder (Fall ra10_dek_schub)
        self.assertEqual(rahmen(1, 1, groesse=(10, 8), zeilen=dict(enumerate(["alpha0", "beta1", "gamma2", "delta3", "epsilon4", "zeta5", "eta6", "theta7"]))).hex(),
                         HEX_S_VOLL)
        self.assertEqual(rahmen(1, 2, schub=(1, 6, 2), zeilen={5: "neu"}).hex(), HEX_S_SCHUB)

    def test_rahmenleser_stueckweise_und_mehrere_in_einem(self):
        a, b = auf_leitung(6, b'{"art":"X"}'), auf_leitung(5, b"\x05\x01\x00")
        for stueck in (1, 2, 3, 7, len(a + b)):
            l, erg = br.Rahmenleser(), []
            for i in range(0, len(a + b), stueck):
                erg += l.nimm((a + b)[i:i + stueck])
            self.assertEqual(erg, [(6, b'{"art":"X"}'), (5, b"\x05\x01\x00")], "Stuecke zu %d" % stueck)
            self.assertEqual(l.rest(), 0)

    def test_rahmenleser_unvollstaendig_und_kaputt(self):
        l = br.Rahmenleser()
        self.assertEqual(l.nimm(auf_leitung(6, b"{}")[:-1]), [])
        self.assertEqual(l.rest(), 6)
        for roh, code in ((b"\x00\x00\x00\x00", "rahmen.laenge"), (b"\xff\xff\xff\xff", "rahmen.zu_gross")):
            with self.assertRaises(br.StromFehler) as e:
                br.Rahmenleser().nimm(roh)
            self.assertEqual(e.exception.code, code)

    def test_beispielrahmen_ohne_deflate(self):
        l = br.Rasterleser(False)
        (n1, b1, s1), (n2, b2, s2), (n3, b3, s3) = anwenden(l, [HEX_VOLL, HEX_SCHUB, HEX_LEER])
        self.assertEqual((n1, b1.stand, b1.spalten, b1.zeilen, s1, b1.cursor, b1.invers), (1, 5, 20, 6, None, (0, 0, False), False))
        self.assertEqual(b1.rows, ["alpha!", "", "beta", "", "", "gamma"])
        self.assertEqual((s2, b2.stand, b2.cursor, b2.invers), ((0, 6, 1), 6, (1, 1, True), True))
        self.assertEqual(b2.rows, ["", "beta", "", "", "gamma", "delta"])
        self.assertEqual((b3.stand, b3.cursor, b3.invers, b3.rows), (9, (1, 1, True), False, ["", "beta", "", "", "gamma", "delta"]))

    def test_schub_nach_dem_crate(self):
        l = br.Rasterleser(False)
        _, b1, _ = l.nimm(bytes.fromhex(HEX_S_VOLL))
        self.assertEqual(b1.rows, ["alpha0", "beta1", "gamma2", "delta3", "epsilon4", "zeta5", "eta6", "theta7"])
        _, b2, s = l.nimm(bytes.fromhex(HEX_S_SCHUB))
        self.assertEqual(s, (1, 6, 2))
        self.assertEqual(b2.rows, ["alpha0", "delta3", "epsilon4", "zeta5", "", "neu", "eta6", "theta7"])

    def test_mit_deflate_ein_kontext_fuer_alle_rahmen(self):
        folge = [HEX_VOLL, HEX_SCHUB, HEX_LEER]
        ohne = br.Rasterleser(False)
        soll = [ohne.nimm(bytes.fromhex(h))[1].rows for h in folge]
        p, mit = Packer(), br.Rasterleser(True)
        ist = [mit.nimm(p.pack(bytes.fromhex(h)))[1].rows for h in folge]
        self.assertEqual(ist, soll)

    def test_deflate_zweiter_gleicher_abzug_ist_kleiner(self):
        # Ein Kontext je Verbindung: Der zweite gleiche Abzug verweist auf den ersten (RA11)
        z = {i: "Zeile %03d Gruesse aeoe" % i for i in range(30)}
        p = Packer()
        e1 = p.pack(rahmen(1, 1, groesse=(80, 30), zeilen=z))
        e2 = p.pack(rahmen(2, 1, groesse=(80, 30), zeilen=z))
        self.assertLess(len(e2), len(e1) / 4)
        l = br.Rasterleser(True)
        self.assertEqual(l.nimm(e1)[1].rows, l.nimm(e2)[1].rows)
        # ein neuer Empfaenger ohne den ersten Rahmen kann den zweiten nicht lesen
        with self.assertRaises(br.StromFehler):
            br.Rasterleser(True).nimm(e2)

    def test_kaputte_rahmen_fail_closed(self):
        voll = bytes.fromhex(HEX_VOLL)
        faelle = {
            "rahmen.art_ungueltig": b"\x06" + voll[1:],
            "rahmen.flags_unbekannt": voll[:2] + b"\x14" + voll[3:],
            "rahmen.abgeschnitten": voll[:-3],
            "rahmen.rest": voll + b"\x00",
            "rahmen.kein_abzug": bytes.fromhex(HEX_SCHUB),
            "rahmen.groesse_ungueltig": rahmen(1, 1, groesse=(1, 6)),
            "rahmen.vu_ungueltig": b"\x05\x01\x00" + b"\x80" * 11,
            "rahmen.zeile_ausserhalb": rahmen(1, 1, groesse=(20, 6), zeilen={6: "x"}),
            "rahmen.zeile_nicht_kanonisch": rahmen(1, 1, groesse=(20, 6), zeilen={0: "ab "}),
            "rahmen.cursor_ungueltig": rahmen(1, 1, groesse=(20, 6), cursor=(0, 20, True)),
            "rahmen.zeichen_ungueltig": rahmen(1, 1, groesse=(20, 6), zeilen={0: "a\x1bb"}),
        }
        for code, roh in faelle.items():
            l = br.Rasterleser(False)
            with self.assertRaises(br.StromFehler, msg=code) as e:
                l.nimm(roh)
            self.assertEqual(e.exception.code, code)
        # ein Schub ausserhalb, ein altes Stand, ungueltiges UTF-8
        l = br.Rasterleser(False)
        l.nimm(rahmen(1, 5, groesse=(20, 6), zeilen={0: "a"}))
        for code, roh in (("rahmen.schub_ungueltig", rahmen(1, 6, schub=(0, 7, 1))), ("rahmen.stand_alt", rahmen(1, 5, zeilen={1: "b"}))):
            with self.assertRaises(br.StromFehler) as e:
                l.nimm(roh)
            self.assertEqual(e.exception.code, code)
        kaputt = bytearray(rahmen(1, 6, zeilen={0: "ab"}))
        kaputt[-1] = 0xFF
        with self.assertRaises(br.StromFehler) as e:
            l.nimm(bytes(kaputt))
        self.assertEqual(e.exception.code, "rahmen.utf8_ungueltig")
        # das Bild blieb, wie es war
        self.assertEqual((l.bilder[1].stand, l.bilder[1].rows[0]), (5, "a"))

    def test_deflate_fehler_und_zu_grosse_rahmen(self):
        for roh in (bytes.fromhex("ffffffff"), b"", bytes.fromhex("62656461156163626200024666062606")):
            with self.assertRaises(br.StromFehler, msg=roh.hex()):
                br.Rasterleser(True).nimm(roh)
        c = zlib.compressobj(6, zlib.DEFLATED, -15)
        riesig = c.compress(b"\x00" * (br.RAHMEN_MAX + 1024)) + c.flush(zlib.Z_SYNC_FLUSH)
        with self.assertRaises(br.StromFehler) as e:
            br.Rasterleser(True).nimm(riesig[:-4])
        self.assertEqual(e.exception.code, "rahmen.zu_gross")


def bild(spalten, zeilen, rows):
    b = br.Bild(spalten, zeilen)
    b.rows = list(rows) + [""] * (zeilen - len(rows))
    return b


def zaehle(text, n):
    return len(lauf.TOKEN(n).findall(text))


class Verlauf(unittest.TestCase):
    def test_token_waechst_in_einer_zeile(self):
        p = br.Protokoll()
        for t in ("FERT", "FERTIG T", "FERTIG T4"):
            p.aktualisiere(bild(40, 5, ["● " + t]))
            self.assertEqual(zaehle(p.text, 4), 1 if t.endswith("T4") else 0)
        self.assertEqual(p.text, "● FERTIG T4")

    def test_token_ueber_zwei_rahmen_und_zwei_zeilen(self):
        p = br.Protokoll()
        p.aktualisiere(bild(20, 5, ["Ausgabe ohne Ende", "FERTIG"]))
        self.assertEqual(zaehle(p.text, 4), 0)
        p.aktualisiere(bild(20, 5, ["Ausgabe ohne Ende", "FERTIG T4"]))
        self.assertEqual(zaehle(p.text, 4), 1)

    def test_token_ueber_umgebrochene_zeile(self):
        # Zeile voll (20 Zellen): die naechste Bildzeile setzt sie ohne Trennzeichen fort
        for erste, zweite in (("a" * 14 + "FERTIG", " T4"), ("a" * 16 + "FERT", "IG T4"), ("a" * 19 + "F", "ERTIG T4")):
            for getrennt in (False, True):
                p = br.Protokoll()
                if getrennt:  # die zweite Bildzeile kommt erst im naechsten Rahmen
                    p.aktualisiere(bild(20, 5, [erste]))
                    p.aktualisiere(bild(20, 5, [erste, zweite]))
                else:
                    p.aktualisiere(bild(20, 5, [erste, zweite]))
                self.assertEqual(zaehle(p.text, 4), 1, (erste, zweite, getrennt, p.text))

    def test_leerzeichen_in_der_letzten_spalte_wird_abgeschnitten(self):
        # Der Sender schneidet Leerzeichen am Ende der Zeile ab: aus `...FERTIG ` in Spalte 20 wird eine Zeile mit 19 Zellen
        p = br.Protokoll()
        p.aktualisiere(bild(20, 5, ["a" * 13 + "FERTIG", "T4"]))
        self.assertEqual(p.text, "a" * 13 + "FERTIG\nT4")
        self.assertEqual(zaehle(p.text, 4), 1)

    def test_token_wird_nur_als_ganze_zahl_gezaehlt(self):
        p = br.Protokoll()
        p.aktualisiere(bild(40, 5, ["FERTIG T41"]))
        self.assertEqual((zaehle(p.text, 4), zaehle(p.text, 41)), (0, 1))

    def test_andere_zeile_dazwischen_verliert_nichts_und_doppelt_nichts(self):
        p = br.Protokoll()
        p.aktualisiere(bild(40, 6, ["FERTIG T"]))
        p.aktualisiere(bild(40, 6, ["FERTIG T", "", "", "", "", "Status 1"]))
        p.aktualisiere(bild(40, 6, ["FERTIG T4", "", "", "", "", "Status 1"]))
        self.assertEqual(zaehle(p.text, 4), 1)
        p.aktualisiere(bild(40, 6, ["FERTIG T4", "", "", "", "", "Status 2"]))
        self.assertEqual(zaehle(p.text, 4), 1)

    def test_schub_wiederholt_keine_zeile(self):
        p = br.Protokoll()
        p.aktualisiere(bild(20, 4, ["eins", "zwei", "drei"]))
        p.aktualisiere(bild(20, 4, ["zwei", "drei", "vier"]), schub=(0, 4, 1))
        self.assertEqual(p.text, "eins\nzwei\ndrei\nvier")
        # ohne gemeldeten Schub erschiene alles erneut (Grenze, siehe README)
        q = br.Protokoll()
        q.aktualisiere(bild(20, 4, ["eins", "zwei", "drei"]))
        q.aktualisiere(bild(20, 4, ["zwei", "drei", "vier"]))
        self.assertGreater(len(q.text), len(p.text))

    def test_umgebrochene_zeile_deren_anfang_herausrollt_wird_nicht_erneut_protokolliert(self):
        lang1, lang2 = "a" * 20, "MK-7Q2 Rest"
        p = br.Protokoll()
        p.aktualisiere(bild(20, 4, [lang1, lang2, "zwei"]))
        vorher = p.text
        p.aktualisiere(bild(20, 4, [lang2, "zwei", "drei"]), schub=(0, 4, 1))
        self.assertEqual(p.text, vorher + "\ndrei")
        self.assertEqual(p.text.count("MK-7Q2"), 1)

    def test_gleiche_zeile_zweimal_ueber_zwei_zuege_wird_zweimal_gezaehlt(self):
        # RC=0 nach zwei Befehlen: die zweite Zeile steht an einer anderen Stelle des Bildes und zaehlt
        p = br.Protokoll()
        p.aktualisiere(bild(20, 6, ["RC=0"]))
        p.aktualisiere(bild(20, 6, ["RC=0", "", "RC=0"]))
        self.assertEqual(p.text.count("RC=0"), 2)

    def test_schub_aus_zwei_bildern_der_seite(self):
        # die Seite meldet keinen Schub; der Laeufer leitet ihn ab, auch wenn in der letzten Stufe schon neuer Text steht
        alt = ["a", "b", "c", "d", "", "Fuss"]
        self.assertEqual(br.schub_zwischen(alt, ["b", "c", "d", "e", "", "Fuss"]), (0, 5, 1))
        self.assertEqual(br.schub_zwischen(alt, ["c", "d", "e", "f", "", "Fuss"]), (0, 5, 2))
        self.assertIsNone(br.schub_zwischen(alt, ["a", "b", "c", "x", "", "Fuss"]))   # nur eine Zeile geaendert
        self.assertIsNone(br.schub_zwischen(alt, ["", "", "", "", "", ""]))
        p = br.Protokoll()
        b1, b2 = bild(20, 6, alt), bild(20, 6, ["b", "c", "d", "e", "", "Fuss"])
        p.aktualisiere(b1)
        p.aktualisiere(b2, br.schub_zwischen(b1.rows, b2.rows))
        self.assertEqual(p.text, "a\nb\nc\nd\nFuss\ne")

    def test_breite_von_zeichen(self):
        self.assertEqual(br._breite("ab"), 2)
        self.assertEqual(br._breite("aあ"), 3)       # breites Zeichen: 2 Zellen
        self.assertEqual(br._breite("é"), 1)       # kombinierendes Zeichen: 0 Zellen
        self.assertEqual(br._breite("●⎿"), 2)  # ● ⎿ : eine Zelle je Zeichen


# --- der ganze Weg ueber einen Prueftisch-Server ------------------------------------------------------------------------------
KENNUNG = "0123456789abcdef0123456789abcdef"
TOKEN = "t" * 64


class Tisch:
    """Ein kleiner HTTP-Server, der `GET /strom` und `POST /eingabe` wie die Bruecke beantwortet (Koepfe, Rahmen byteweise) und die Ereignisse merkt."""

    def __init__(self, deflate, auf_abo):
        self.deflate, self.auf_abo = deflate, auf_abo
        self.ereignisse, self.posts = [], []
        self.packer = Packer() if deflate else None
        self.q = []
        self.lock = threading.Lock()
        tisch = self

        class H(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *a):
                pass

            def do_GET(self):
                assert self.path == "/strom"
                tisch.get_kopf = {k.lower(): v for k, v in self.headers.items()}
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Connection", "close")
                self.send_header("X-Verbindung", KENNUNG)
                self.send_header("X-Deflate", "1" if tisch.deflate else "0")
                self.end_headers()
                self.close_connection = True
                tisch.senden(6, json.dumps({"art": "Sitzungsliste", "sitzungen": [
                    {"id": "s-1", "harness": "pi", "modell": "m", "route": "lokal", "kunde": "", "zustand": "laeuft"}]}).encode())
                bis = time.monotonic() + 8
                while time.monotonic() < bis and not tisch.ende:
                    with tisch.lock:
                        roh, tisch.q = b"".join(tisch.q), []
                    for i in range(0, len(roh), 1):  # Byte fuer Byte: jeder Rahmen kommt ueber viele Lesevorgaenge
                        self.wfile.write(roh[i:i + 1])
                        self.wfile.flush()
                    time.sleep(0.01)

            def do_POST(self):
                n = int(self.headers.get("Content-Length") or 0)
                koerper = json.loads(self.rfile.read(n).decode("utf-8"))
                tisch.posts.append({"pfad": self.path, "origin": self.headers.get("Origin"), "typ": self.headers.get("Content-Type"),
                                    "verbindung": self.headers.get("X-Verbindung"), "koerper": koerper})
                self.send_response(204)
                self.send_header("Connection", "close")
                self.end_headers()
                self.close_connection = True
                tisch.auf_abo(tisch, koerper)

        self.ende = False
        self.srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
        self.srv.daemon_threads = True
        self.port = self.srv.server_address[1]
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()

    def senden(self, art, nutz):
        with self.lock:
            self.q.append(auf_leitung(art, nutz))

    def raster(self, roh):
        self.senden(5, self.packer.pack(roh) if self.deflate else roh)

    def zu(self):
        self.ende = True
        self.srv.shutdown()
        self.srv.server_close()


def warte(bedingung, frist=6.0):
    bis = time.monotonic() + frist
    while time.monotonic() < bis:
        if bedingung():
            return True
        time.sleep(0.01)
    return False


class GanzerWeg(unittest.TestCase):
    def _client(self, tisch):
        return br.Bruecke({"url": "http://127.0.0.1:%d" % tisch.port, "token": TOKEN, "frist_s": 5, "abo": {"spalten": 20, "zeilen": 6}})

    def _lauf(self, deflate):
        def auf_abo(tisch, e):
            if e.get("art") == "Abo":
                tisch.raster(rahmen(1, 1, groesse=(20, 6), cursor=(0, 0, True), zeilen={0: "Eingabe: " + "x" * 11}))
            elif e.get("art") == "Taste":
                tisch.raster(rahmen(1, 2, cursor=(1, 6, True), zeilen={0: "Eingabe: " + "x" * 11, 1: "FERTIG"}))
                tisch.raster(rahmen(1, 3, zeilen={1: "FERTIG T4"}))
                tisch.senden(7, json.dumps({"grund": "taste_unbekannt"}).encode())
        tisch = Tisch(deflate, auf_abo)
        c = self._client(tisch)
        try:
            marke = c.ablehnungen_marke()
            self.assertEqual(tisch.get_kopf["authorization"], "Bearer " + TOKEN)
            self.assertEqual(c._offen().deflate, deflate)
            self.assertTrue(warte(lambda: c.raster("s-1")["rahmen"] >= 1), "kein Rahmen nach dem Abo")
            r = c.raster("s-1")
            self.assertEqual((r["groesse"], r["cursor"], r["zeilen"][0]), ([20, 6], [0, 0, True], "Eingabe: " + "x" * 11))
            s = c.sitzung("s-1")
            self.assertEqual((s["zustand"], s["start"]["route"]), ("laeuft", "lokal"))
            n0 = c.verlauf("s-1")[0]
            self.assertEqual(c.eingabe("s-1", "hallo"), (202, {"ok": True}))
            self.assertTrue(warte(lambda: zaehle(c.verlauf("s-1", n0)[1], 4) == 1), "FERTIG T4 nicht im Verlauf: %r" % c.verlauf("s-1", n0)[1])
            self.assertTrue(warte(lambda: c.ablehnungen_seit(marke)))
            self.assertEqual(c.ablehnungen_seit(marke), [("taste_unbekannt", "")])
            arten = [p["koerper"]["art"] for p in tisch.posts]
            self.assertEqual(arten, ["Abo", "Text", "Taste"])
            self.assertEqual(tisch.posts[0]["koerper"], {"art": "Abo", "sitzung": "s-1", "an": True, "spalten": 20, "zeilen": 6})
            self.assertEqual(tisch.posts[1]["koerper"], {"art": "Text", "sitzung": "s-1", "text": "hallo"})
            self.assertEqual(tisch.posts[2]["koerper"], {"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": []})
            for p in tisch.posts:  # jeder POST traegt Origin, Inhaltstyp und die Kennung der Verbindung
                self.assertEqual((p["pfad"], p["origin"], p["typ"], p["verbindung"]), ("/eingabe", c.url, "application/json", KENNUNG))
        finally:
            c.zumachen()
            tisch.zu()

    def test_mit_deflate(self):
        self._lauf(True)

    def test_ohne_deflate(self):
        self._lauf(False)

    def test_kaputter_rahmen_beendet_den_strom_und_jeder_aufruf_wirft(self):
        def auf_abo(tisch, e):
            tisch.raster(rahmen(1, 1, groesse=(20, 6), zeilen={0: "ok"}))
            tisch.raster(rahmen(1, 1, zeilen={0: "alt"}))  # Stand nicht gestiegen: RA10 `stand_alt`
        tisch = Tisch(False, auf_abo)
        c = self._client(tisch)
        try:
            c.raster("s-1")
            self.assertTrue(warte(lambda: c._strom.fehler is not None))
            for aufruf in (lambda: c.raster("s-1"), lambda: c.verlauf("s-1"), lambda: c.eingabe("s-1", "x"), lambda: c.sitzung("s-1")):
                with self.assertRaises(br.StromFehler) as e:
                    aufruf()
                self.assertEqual(e.exception.code, "rahmen.stand_alt")
                self.assertNotIn(TOKEN, str(e.exception))
        finally:
            c.zumachen()
            tisch.zu()

    def test_dienstfehler_und_unbekannte_art(self):
        for art, nutz, code in ((8, b'{"fehler": "art_nicht_erlaubt"}', "strom.dienst_art_nicht_erlaubt"), (9, b"{}", "strom.art_unbekannt"),
                                (6, b"kein json", "strom.json_ungueltig")):
            tisch = Tisch(False, lambda t, e: None)
            c = self._client(tisch)
            try:
                c._offen()
                tisch.senden(art, nutz)
                self.assertTrue(warte(lambda: c._strom.fehler is not None))
                with self.assertRaises(br.StromFehler) as e:
                    c.raster("s-1")
                self.assertEqual(e.exception.code, code)
            finally:
                c.zumachen()
                tisch.zu()

    def test_kein_strom_ohne_200(self):
        class H(BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                self.send_response(401)
                self.send_header("Content-Length", "0")
                self.end_headers()
        srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            c = br.Bruecke({"url": "http://127.0.0.1:%d" % srv.server_address[1], "token": TOKEN})
            with self.assertRaises(br.StromFehler) as e:
                c.raster("s-1")
            self.assertEqual(e.exception.code, "strom.status_401")
        finally:
            srv.shutdown()
            srv.server_close()


if __name__ == "__main__":
    unittest.main(verbosity=1)
