"""Client gegen die Bruecke zwischen Browser und Kern-Dienst (Drehbuch Abschnitt 2 und S1), nach den Vertraegen `bruecke`, `dienst`
und `zustandskern`.

Die Bruecke kennt drei Wege (Vertrag bruecke, Abschnitt 5):
  - `GET /strom`: der Ereignisstrom. Kopf `X-Verbindung` (32 Kleinbuchstaben-Hex), Kopf `X-Deflate` (`0` oder `1`), danach Rahmen
    `[n: u32 big endian][art: u8][nutzlast: n - 1 Byte]` (Vertrag dienst, Abschnitt 5, DI3). Art 5 traegt Rasterrahmen (Raw-Deflate mit
    einem Kontext je Verbindung, wenn `X-Deflate: 1`), Art 6 eine Nachricht des Kerns als JSON, Art 7 eine Ablehnung, Art 8 einen Fehler.
  - `POST /starten`: 200 `{"sitzung": id}` oder 422 `{"code", "grund", "text"}`.
  - `POST /eingabe`: ein Ereignis der Ansicht (Vertrag dienst DI12 Punkt 6 und 7, Vertrag zustandskern Abschnitt 4) an die Verbindung aus
    `X-Verbindung`; Antwort 204.
Rasterrahmen kommen erst nach einem `Abo` der Sitzung. Der Client haelt eine Verbindung `GET /strom` je Lauf, liest die Rahmen in einem
eigenen Faden (`Strom`, `Rahmenleser`, `Rasterleser`) und baut daraus Raster, Verlauf und Sitzungsliste auf. Ein kaputter Rahmen beendet
den Strom mit einem `StromFehler`; jeder weitere Aufruf wirft ihn wieder (fail closed). Das Token steht nur im Kopf der Anfrage, nie in
einer Meldung oder einer Ausnahme.
"""
from __future__ import annotations

import json
import re
import socket
import threading
import time
import unicodedata
import urllib.error
import urllib.request
import zlib

RAHMEN_MAX = 16_777_216            # laengster Rasterrahmen nach dem Entpacken (RA11)
STROM_N_MAX = RAHMEN_MAX + 1024    # groesstes `n` eines Rahmens auf der Leitung; mehr ist `rahmen.zu_gross`
KENNUNG = re.compile(r"^[0-9a-f]{32}$")
SITZUNG = re.compile(r"^s-([0-9]{1,3})$")


class StromFehler(Exception):
    """Der Strom ist kaputt (Rahmen, Deflate, Kopf, Verbindung). `code` ist ein fester Name, nie ein Inhalt der Sitzung."""

    def __init__(self, code):
        super().__init__(code)
        self.code = code


# --- Rahmen auf der Leitung -----------------------------------------------------------------------------------------------
class Rahmenleser:
    """Setzt Rahmen `[n: u32][art: u8][nutzlast]` aus beliebigen Stuecken zusammen (DI3). `n` zaehlt Art und Nutzlast, ist also mindestens 1."""

    def __init__(self):
        self._p = bytearray()

    def nimm(self, daten):
        self._p += daten
        erg = []
        while len(self._p) >= 4:
            n = int.from_bytes(self._p[:4], "big")
            if n < 1:
                raise StromFehler("rahmen.laenge")
            if n > STROM_N_MAX:
                raise StromFehler("rahmen.zu_gross")
            if len(self._p) < 4 + n:
                break
            erg.append((self._p[4], bytes(self._p[5:4 + n])))
            del self._p[:4 + n]
        return erg

    def rest(self):
        """Zahl der Bytes eines noch nicht vollstaendigen Rahmens."""
        return len(self._p)


# --- Rasterrahmen (Vertrag rasterauszug, Abschnitte 5 und 6, RA9 bis RA11) --------------------------------------------------
class Bild:
    """Das Bild einer Sitzung beim Betrachter: Zeilen als Klartext (ohne Stil), Cursor, Stand."""

    def __init__(self, spalten, zeilen):
        self.spalten, self.zeilen = spalten, zeilen
        self.rows = [""] * zeilen
        self.cursor = (0, 0, False)
        self.invers = False
        self.stand = 0

    def kopie(self):
        b = Bild(self.spalten, self.zeilen)
        b.rows, b.cursor, b.invers, b.stand = list(self.rows), self.cursor, self.invers, self.stand
        return b


def _vu(b, p):
    """Eine Zahl mit 7 Bit je Byte, niedrigste Gruppe zuerst, hoechstens 10 Byte, kuerzeste Form."""
    n = 0
    for k in range(10):
        if p >= len(b):
            raise StromFehler("rahmen.abgeschnitten")
        x = b[p]
        p += 1
        n |= (x & 0x7F) << (7 * k)
        if not x & 0x80:
            if (k > 0 and x == 0) or n >= 1 << 64:
                raise StromFehler("rahmen.vu_ungueltig")
            return n, p
    raise StromFehler("rahmen.vu_ungueltig")


def _zahl(b, p):
    """Eine Anzahl: nie groesser als die Zahl der noch nicht gelesenen Bytes, denn jedes Element braucht mindestens eines."""
    n, p = _vu(b, p)
    if n > len(b) - p:
        raise StromFehler("rahmen.abgeschnitten")
    return n, p


def _farbe_ok(c):
    return 0 <= c <= 256 or 0x01000000 <= c <= 0x01FFFFFF


def _zeichen_ok(c):
    o = ord(c)
    return not (o < 0x20 or 0x7F <= o <= 0x9F)


def rahmen_dekodieren(b, alt):
    """Liest einen entpackten Rasterrahmen gegen das Bild `alt` (oder None). Gibt (nr, neues Bild, schub). `schub` ist None oder
    (oben, unten, n). Jede Abweichung von RA10 ist ein `StromFehler`; `alt` aendert sich nie."""
    if len(b) < 3:
        raise StromFehler("rahmen.abgeschnitten")
    if b[0] != 5:
        raise StromFehler("rahmen.art_ungueltig")
    nr, flags, p = b[1], b[2], 3
    if flags & 0xF0:
        raise StromFehler("rahmen.flags_unbekannt")
    stand, p = _vu(b, p)
    if flags & 4:
        if flags & 1:
            raise StromFehler("rahmen.ungueltig")
        sp, p = _vu(b, p)
        ze, p = _vu(b, p)
        if not (2 <= sp <= 1000 and 1 <= ze <= 1000):
            raise StromFehler("rahmen.groesse_ungueltig")
        neu = Bild(sp, ze)
    else:
        if alt is None:
            raise StromFehler("rahmen.kein_abzug")
        if stand <= alt.stand:
            raise StromFehler("rahmen.stand_alt")
        neu = alt.kopie()
    spalten, zeilen = neu.spalten, neu.zeilen
    schub = None
    if flags & 1:
        oben, p = _vu(b, p)
        unten, p = _vu(b, p)
        n, p = _vu(b, p)
        if not (n >= 1 and oben + n <= unten <= zeilen):
            raise StromFehler("rahmen.schub_ungueltig")
        schub = (oben, unten, n)
    if flags & 2:
        z, p = _vu(b, p)
        s, p = _vu(b, p)
        v, p = _vu(b, p)
        if v > 1 or z >= zeilen or s >= spalten:
            raise StromFehler("rahmen.cursor_ungueltig")
        neu.cursor = (z, s, v == 1)
    ns, p = _zahl(b, p)
    stile = []
    for _ in range(ns):
        fg, p = _vu(b, p)
        bg, p = _vu(b, p)
        fl, p = _vu(b, p)
        if not (_farbe_ok(fg) and _farbe_ok(bg) and 0 <= fl < 128):
            raise StromFehler("rahmen.stil_ungueltig")
        stile.append((fg, bg, fl))
    if schub:
        oben, unten, n = schub
        neu.rows[oben:unten] = neu.rows[oben + n:unten] + [""] * n
    nz, p = _zahl(b, p)
    letzte = -1
    for _ in range(nz):
        i, p = _vu(b, p)
        if i >= zeilen:
            raise StromFehler("rahmen.zeile_ausserhalb")
        if i <= letzte:
            raise StromFehler("rahmen.zeile_reihenfolge")
        letzte = i
        nl, p = _zahl(b, p)
        teile, vorher = [], None
        for _ in range(nl):
            sn, p = _vu(b, p)
            if sn >= len(stile):
                raise StromFehler("rahmen.stilnr_ungueltig")
            ln, p = _zahl(b, p)
            roh = b[p:p + ln]
            p += ln
            try:
                text = roh.decode("utf-8")
            except UnicodeDecodeError:
                raise StromFehler("rahmen.utf8_ungueltig")
            if not all(_zeichen_ok(c) for c in text):
                raise StromFehler("rahmen.zeichen_ungueltig")
            if not text or (vorher is not None and stile[sn] == vorher):
                raise StromFehler("rahmen.zeile_nicht_kanonisch")
            vorher = stile[sn]
            teile.append((stile[sn], text))
        if teile and teile[-1][0] == (0, 0, 0) and teile[-1][1].endswith(" "):
            raise StromFehler("rahmen.zeile_nicht_kanonisch")
        neu.rows[i] = "".join(t for _, t in teile)
    if p != len(b):
        raise StromFehler("rahmen.rest")
    neu.invers = bool(flags & 8)
    neu.stand = stand
    return nr, neu, schub


class Rasterleser:
    """Der Empfaenger einer Verbindung: ein Bild je Sitzungsnummer und, mit `deflate`, ein Deflate-Kontext fuer alle Rahmen (RA11).
    Der Absender packt jeden Rahmen mit Sync-Flush und schneidet die vier Bytes `00 00 ff ff` ab; hier kommen sie wieder an."""

    def __init__(self, deflate):
        self.deflate = deflate
        self.bilder = {}
        self._inf = zlib.decompressobj(-15) if deflate else None

    def nimm(self, nutzlast):
        """Gibt (nr, Bild, schub). Das Bild der Sitzung aendert sich nur, wenn der ganze Rahmen gueltig ist."""
        if self.deflate:
            try:
                roh = self._inf.decompress(nutzlast + b"\x00\x00\xff\xff", RAHMEN_MAX + 1)
            except zlib.error:
                raise StromFehler("rahmen.inflate")
            if self._inf.unconsumed_tail or len(roh) > RAHMEN_MAX:
                raise StromFehler("rahmen.zu_gross")
        else:
            if len(nutzlast) > RAHMEN_MAX:
                raise StromFehler("rahmen.zu_gross")
            roh = nutzlast
        if len(roh) < 2:
            raise StromFehler("rahmen.abgeschnitten")
        nr, neu, schub = rahmen_dekodieren(roh, self.bilder.get(roh[1]))
        self.bilder[nr] = neu
        return nr, neu, schub


# --- Verlauf aus den Rasterrahmen -----------------------------------------------------------------------------------------
def _breite(text):
    return sum(0 if unicodedata.combining(c) else (2 if unicodedata.east_asian_width(c) in ("W", "F") else 1) for c in text)


def logische_zeilen(bild):
    """Die Zeilen des Bildes als (Anfangszeile, Text). Eine Zeile, die die ganze Breite fuellt, ist umgebrochen: die naechste
    Bildzeile gehoert ohne Trennzeichen dazu. Leere Zeilen und Leerraum am Ende fallen weg."""
    erg, text, start, voll = [], None, 0, False
    for i, z in enumerate(bild.rows):
        if text is not None and voll:
            text += z
        else:
            if text is not None and text.strip():
                erg.append((start, text.rstrip()))
            text, start = z, i
        voll = _breite(z) >= bild.spalten
    if text is not None and text.strip():
        erg.append((start, text.rstrip()))
    return erg


def schub_zwischen(alt, neu):
    """Fuer ein Bild, das nur als Liste von Zeilen vorliegt (die Seite im Browser-Modus): Gibt (oben, unten, n), wenn `neu` ueberwiegend aus
    `alt` durch einen Schub nach oben um n Zeilen entsteht, sonst None. Bereich sind alle Zeilen oder alle ausser der letzten (Fusszeile).
    Gezaehlt werden nicht leere Zeilen, die nach dem Schub gleich sind; gewinnt der kleinste n mit den meisten, und nur, wenn es mehr sind als
    ohne Schub. Eine neue Zeile in der Stufe stoert nicht: sie steht danach als geaenderte Zeile im Bild."""
    if len(alt) != len(neu):
        return None
    beste = None
    for unten in (len(alt) - 1, len(alt)):  # zuerst ohne die Fusszeile: bei gleich vielen Treffern gewinnt der kleinere Bereich
        ohne = sum(1 for i in range(unten) if neu[i].strip() and alt[i] == neu[i])
        for n in range(1, unten):
            z = sum(1 for i in range(unten - n) if neu[i].strip() and alt[i + n] == neu[i])
            if z > ohne and (beste is None or z > beste[0]):
                beste = (z, (0, unten, n))
    return beste[1] if beste else None


class Protokoll:
    """Der Verlauf einer Sitzung: ein Text, der nur waechst. Der Vertrag bietet keinen Verlauf; er wird aus den Rasterrahmen aufgebaut.

    Je Rahmen werden die logischen Zeilen des Bildes (siehe `logische_zeilen`) mit dem verglichen, was zuletzt fuer dieselbe
    Anfangszeile protokolliert wurde:
      - gleicher Text: nichts;
      - der neue Text setzt den alten fort und die Zeile ist die zuletzt protokollierte: nur das Ende wird angehaengt, die offene
        Zeile waechst (so entsteht eine Eingabe oder Ausgabe Zeichen fuer Zeichen zu einer Zeile);
      - sonst: der ganze neue Text als neue Zeile.
    Ein Schub im Rahmen verschiebt die gemerkten Anfangszeilen mit; was aus dem Bereich herausfaellt, wird vergessen. Eine Zeile, die
    ein Programm im Bild verschiebt, ohne dass der Sender einen Schub meldet, erscheint erneut. Der Verlauf kann also Zeilen mehrfach
    enthalten; er verliert keine Zeile, die im Bild stand, ausser einer, die zwischen zwei Rahmen entsteht und wieder verschwindet."""

    def __init__(self):
        self.text = ""
        self._gemerkt = {}
        self._letzte = None
        self._bild = None

    def aktualisiere(self, bild, schub=None):
        if schub and self._bild is not None:
            # Das Bild vor dem Rahmen, um den Schub gerollt: Was davon als Zeile im Bild bleibt, ist schon protokolliert. Eine umgebrochene
            # Zeile, deren erste Bildzeile oben herausfaellt, beginnt danach in der naechsten Bildzeile; ihr Rest zaehlt als bekannt.
            oben, unten, n = schub
            vorher = self._bild.kopie()
            vorher.rows[oben:unten] = vorher.rows[oben + n:unten] + [""] * n
            self._gemerkt = dict(logische_zeilen(vorher))
            if self._letzte is not None and oben <= self._letzte < unten:
                self._letzte = self._letzte - n if self._letzte - n >= oben else None
        gemerkt = {}
        for s, t in logische_zeilen(bild):
            alt = self._gemerkt.get(s)
            if alt != t:
                if alt is not None and self._letzte == s and t.startswith(alt):
                    self.text += t[len(alt):]
                else:
                    self.text += ("\n" if self.text else "") + t
                self._letzte = s
            gemerkt[s] = t
        self._gemerkt = gemerkt
        self._bild = bild.kopie()


# --- Die Verbindung GET /strom ----------------------------------------------------------------------------------------------
def _kopf_lesen(sock, frist_s):
    """Liest Statuszeile und Koepfe bis `\\r\\n\\r\\n`. Gibt (status, {name klein: wert}, Rest der Bytes)."""
    roh = b""
    ende = time.monotonic() + frist_s
    while b"\r\n\r\n" not in roh:
        if len(roh) > 8192 or time.monotonic() > ende:
            raise StromFehler("strom.kopf")
        try:
            d = sock.recv(4096)
        except OSError:
            raise StromFehler("strom.kopf")
        if not d:
            raise StromFehler("strom.kopf")
        roh += d
    kopf, _, rest = roh.partition(b"\r\n\r\n")
    zeilen = kopf.decode("latin-1").split("\r\n")
    m = re.match(r"^HTTP/1\.[01] (\d{3})", zeilen[0])
    if not m:
        raise StromFehler("strom.kopf")
    koepfe = {}
    for z in zeilen[1:]:
        n, _, w = z.partition(":")
        koepfe[n.strip().lower()] = w.strip()
    return int(m.group(1)), koepfe, rest


class Strom:
    """Eine Verbindung `GET /strom` mit dem Faden, der ihre Rahmen liest. Alles, was der Faden aufbaut, liegt unter `self.lock`."""

    def __init__(self, host, port, token, frist_s, protokolle, kopf="Authorization", praefix="Bearer "):
        self.lock = threading.Condition()
        self.fehler = None            # StromFehler oder None
        self.zu = False               # die Verbindung ist zu (vom Dienst, von uns oder wegen eines Fehlers)
        self.selbst_zu = False
        self.protokolle = protokolle  # nr -> Protokoll; gehoert der Bruecke und ueberlebt die Verbindung
        self.sitzungen = {}           # id -> Eintrag der letzten `Sitzungsliste`
        self.liste_da = False
        self.ablehnungen = []         # [(grund, code)] der Rahmen Art 7
        self.rahmen_je_nr = {}
        self._rahmen = Rahmenleser()
        s = socket.create_connection((host, port), timeout=frist_s)
        s.sendall(("GET /strom HTTP/1.1\r\nHost: %s:%d\r\n%s: %s%s\r\nConnection: close\r\n\r\n" % (host, port, kopf, praefix, token)).encode("ascii"))
        try:
            status, koepfe, rest = _kopf_lesen(s, frist_s)
            if status != 200:
                raise StromFehler("strom.status_%d" % status)
            self.kennung = koepfe.get("x-verbindung", "")
            if not KENNUNG.match(self.kennung):
                raise StromFehler("strom.kennung")
            if koepfe.get("x-deflate") not in ("0", "1"):
                raise StromFehler("strom.deflate_kopf")
            self.deflate = koepfe["x-deflate"] == "1"
        except BaseException:
            s.close()
            raise
        self.leser = Rasterleser(self.deflate)
        self._sock = s
        s.settimeout(0.5)
        self._faden = threading.Thread(target=self._lies, args=(rest,), daemon=True, name="strom")
        self._faden.start()

    def _lies(self, rest):
        try:
            daten = rest
            while True:
                if daten:
                    with self.lock:
                        for art, nutz in self._rahmen.nimm(daten):
                            self._rahmen_verarbeiten(art, nutz)
                        self.lock.notify_all()
                try:
                    daten = self._sock.recv(65536)
                except socket.timeout:
                    daten = b""
                    if self.zu:
                        return
                    continue
                except OSError:
                    daten = b""
                    break
                if not daten:
                    break
            with self.lock:
                if self._rahmen.rest() and not self.selbst_zu:
                    self.fehler = self.fehler or StromFehler("strom.abgeschnitten")
        except StromFehler as e:
            with self.lock:
                self.fehler = self.fehler or e
        finally:
            with self.lock:
                self.zu = True
                self.lock.notify_all()
            try:
                self._sock.close()
            except OSError:
                pass

    def _rahmen_verarbeiten(self, art, nutz):
        if art == 5:
            nr, bild, schub = self.leser.nimm(nutz)
            self.rahmen_je_nr[nr] = self.rahmen_je_nr.get(nr, 0) + 1
            self.protokolle.setdefault(nr, Protokoll()).aktualisiere(bild, schub)
        elif art == 6:
            m = self._json(nutz)
            if m.get("art") == "Sitzungsliste" and isinstance(m.get("sitzungen"), list):
                self.sitzungen = {e["id"]: e for e in m["sitzungen"] if isinstance(e, dict) and isinstance(e.get("id"), str)}
                self.liste_da = True
        elif art == 7:
            m = self._json(nutz)
            self.ablehnungen.append((self._wort(m.get("grund")), self._wort(m.get("code"))))
        elif art == 8:
            m = self._json(nutz)
            self.fehler = self.fehler or StromFehler("strom.dienst_" + (self._wort(m.get("fehler")) or "fehler"))
        else:
            raise StromFehler("strom.art_unbekannt")

    @staticmethod
    def _json(nutz):
        try:
            m = json.loads(nutz.decode("utf-8"))
        except (ValueError, UnicodeDecodeError):
            raise StromFehler("strom.json_ungueltig")
        if not isinstance(m, dict):
            raise StromFehler("strom.json_ungueltig")
        return m

    @staticmethod
    def _wort(w):
        """Grund und Code kommen als feste Woerter; was nicht so aussieht, wird nicht weitergegeben."""
        return w if isinstance(w, str) and re.match(r"^[a-z0-9_.]{1,64}$", w) else ""

    def zumachen(self):
        self.selbst_zu = True
        self.zu = True
        try:
            self._sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        try:
            self._sock.close()
        except OSError:
            pass
        self._faden.join(2)


# --- Der Client -----------------------------------------------------------------------------------------------------------
class Bruecke:
    # `Lauf.senden` nutzt `text` und `taste` einzeln (Enter mit Abstand). Eine Unterklasse mit eigenem Eingabeweg (die Seite
    # im Browser) setzt das auf False und behaelt ihr `eingabe`.
    text_und_taste = True

    def __init__(self, k):
        self.url = k["url"].rstrip("/")
        m = re.match(r"^http://([^:/]+):(\d+)$", self.url)
        if not m:
            raise ValueError("Adresse der Bruecke hat nicht die Form http://<host>:<port>")
        self._host, self._port = m.group(1), int(m.group(2))
        self.token = k.get("token")
        self.kopf = k.get("token_kopf", "Authorization")
        self.praefix = k.get("token_praefix", "Bearer ")
        self.frist = float(k.get("frist_s", 10))
        g = k.get("abo", {})
        self.abo = {"spalten": int(g.get("spalten", 120)), "zeilen": int(g.get("zeilen", 40))}
        self.diagnose_pfad = k.get("diagnose_pfad")  # nur Attrappen: die echte Bruecke hat keinen solchen Weg (null)
        self._strom = None
        self._abos = set()
        self._protokolle = {}
        self._sperre = threading.RLock()

    # --- HTTP -------------------------------------------------------------------------------------------------------------
    def _anfrage(self, methode, pfad, body=None, ohne_token=False, koepfe=None):
        daten = None if body is None else (body if isinstance(body, bytes) else json.dumps(body).encode("utf-8"))
        r = urllib.request.Request(self.url + pfad, data=daten, method=methode)
        if methode == "POST":  # die echte Bruecke verlangt bei POST den Kopf Origin der eigenen Adresse (Vertrag bruecke, Abschnitt 4)
            r.add_header("Origin", self.url)
            r.add_header("Content-Type", "application/json")
        if self.token and not ohne_token:
            r.add_header(self.kopf, self.praefix + self.token)
        for n, w in (koepfe or {}).items():
            r.add_header(n, w)
        try:
            with urllib.request.urlopen(r, timeout=self.frist) as a:
                code, roh = a.status, a.read()
        except urllib.error.HTTPError as e:
            code, roh = e.code, e.read()
        try:
            return code, json.loads(roh.decode("utf-8"))
        except ValueError:
            return code, roh.decode("utf-8", "replace")

    # --- Strom ------------------------------------------------------------------------------------------------------------
    def _offen(self):
        """Der Strom dieses Laufs; er wird beim ersten Aufruf geoeffnet. Ein kaputter oder beendeter Strom wirft seinen Fehler."""
        with self._sperre:
            if self._strom is None:
                self._abos = set()
                self._strom = Strom(self._host, self._port, self.token or "", self.frist, self._protokolle, self.kopf, self.praefix)
            st = self._strom
        with st.lock:
            if st.fehler:
                raise st.fehler
            if st.zu and not st.selbst_zu:
                raise StromFehler("strom.beendet")
        return st

    def _ereignis(self, st, ereignis):
        """`POST /eingabe` an die Verbindung des Stroms. Gibt den Statuscode; alles ausser 204 ist ein Fehler des Laeufers."""
        code, _ = self._anfrage("POST", "/eingabe", ereignis, koepfe={"X-Verbindung": st.kennung})
        return code

    @staticmethod
    def _nr(sid):
        m = SITZUNG.match(sid or "")
        return int(m.group(1)) if m else None

    def _abonniere(self, st, sid):
        with self._sperre:
            if sid in self._abos:
                return
            self._abos.add(sid)
        code = self._ereignis(st, {"art": "Abo", "sitzung": sid, "an": True, "spalten": self.abo["spalten"], "zeilen": self.abo["zeilen"]})
        if code != 204:
            with self._sperre:
                self._abos.discard(sid)
            raise StromFehler("abo.status_%d" % code)

    def zumachen(self):
        """Schliesst den Strom. Ein weiterer Aufruf oeffnet einen neuen."""
        with self._sperre:
            st, self._strom = self._strom, None
        if st is not None:
            st.zumachen()

    # --- Schnittstelle fuer den Laeufer ----------------------------------------------------------------------------------
    def starten(self, start, ohne_token=False):
        """Gibt (Statuscode, Antwort, t0). t0 ist der Augenblick unmittelbar vor dem Absenden. Bei 200 wird die Sitzung abonniert."""
        roh = start if isinstance(start, bytes) else json.dumps(start).encode("utf-8")
        st = None if ohne_token else self._offen()  # der Strom steht vor dem Start, damit keine Sitzungsliste verloren geht
        t0 = time.monotonic()
        code, antw = self._anfrage("POST", "/starten", roh, ohne_token)
        if st is not None and code == 200 and isinstance(antw, dict) and isinstance(antw.get("sitzung"), str):
            self._abonniere(st, antw["sitzung"])
        return code, antw, t0

    def sitzung(self, sid, frist_s=2.0):
        """Die Sitzung aus der letzten `Sitzungsliste`: `start` (harness, modell, route, kunde), `zustand` (laeuft, beendet), `code`.
        Ist sie noch nicht in der Liste, wartet der Aufruf bis `frist_s`."""
        st = self._offen()
        ende = time.monotonic() + frist_s
        with st.lock:
            while sid not in st.sitzungen and not st.zu and time.monotonic() < ende:
                st.lock.wait(0.05)
            e = st.sitzungen.get(sid)
        if e is None:
            return {"error": "unbekannt"}
        return {"sitzung": sid, "start": {n: e.get(n) for n in ("harness", "modell", "route", "kunde")},
                "zustand": e.get("zustand"), "code": e.get("code", 0)}

    def sitzungs_ids(self, frist_s=3.0):
        """Menge der Kennungen aller Sitzungen der letzten `Sitzungsliste` (auch beendete); None, wenn die Liste nicht innerhalb von `frist_s`
        kommt."""
        st = self._offen()
        ende = time.monotonic() + frist_s
        with st.lock:
            while not st.liste_da and not st.zu and time.monotonic() < ende:
                st.lock.wait(0.05)
            return set(st.sitzungen) if st.liste_da else None

    def laufende_sitzungen(self, frist_s=3.0):
        """Zahl der Sitzungen mit `zustand` `laeuft` in der Sitzungsliste, die der Strom beim Verbinden schickt; None, wenn die Liste
        nicht innerhalb von `frist_s` kommt."""
        st = self._offen()
        ende = time.monotonic() + frist_s
        with st.lock:
            while not st.liste_da and not st.zu and time.monotonic() < ende:
                st.lock.wait(0.05)
            if not st.liste_da:
                return None
            return sum(1 for e in st.sitzungen.values() if e.get("zustand") == "laeuft")

    def raster(self, sid):
        """Das Bild der Sitzung: `zeilen` (alle Zeilen als Klartext), `cursor` (`[zeile, spalte, sichtbar]`), `stand`, `groesse`, `rahmen`
        (Zahl der bisher angenommenen Rahmen). Vor dem ersten Rahmen ist `zeilen` leer."""
        st = self._offen()
        self._abonniere(st, sid)
        nr = self._nr(sid)
        with st.lock:
            b = st.leser.bilder.get(nr)
            n = st.rahmen_je_nr.get(nr, 0)
            if b is None:
                return {"rahmen": n, "zeilen": []}
            return {"rahmen": n, "zeilen": list(b.rows), "cursor": list(b.cursor), "stand": b.stand, "groesse": [b.spalten, b.zeilen]}

    def verlauf(self, sid, ab=0):
        """Gibt (Laenge, Text ab Zeichen `ab`). Der Text waechst nur (siehe `Protokoll`)."""
        st = self._offen()
        self._abonniere(st, sid)
        with st.lock:
            p = self._protokolle.get(self._nr(sid))
            t = p.text if p else ""
        return len(t), t[ab:]

    def eingabe(self, sid, text):
        """Tippt `text` und Enter (zwei Ereignisse `Text` und `Taste`; ohne Text nur Enter). Gibt (202, {"ok": True}); die Ablehnung
        eines Ereignisses durch den Dienst steht im Strom (`ablehnungen_seit`)."""
        st = self._offen()
        self._abonniere(st, sid)
        for e in ([{"art": "Text", "sitzung": sid, "text": text}] if text else []) + [{"art": "Taste", "sitzung": sid, "taste": "Enter", "umschalter": []}]:
            code = self._ereignis(st, e)
            if code != 204:
                return code, {"error": "eingabe"}
        return 202, {"ok": True}

    def text(self, sid, text):
        """Nur das Ereignis `Text` (ohne Enter), fuer `Lauf.zug` mit Abstand (Entscheidung vom 22.11.2026). Gibt (202, ...) oder den Code."""
        st = self._offen()
        self._abonniere(st, sid)
        code = self._ereignis(st, {"art": "Text", "sitzung": sid, "text": text})
        return (202, {"ok": True}) if code == 204 else (code, {"error": "eingabe"})

    def taste(self, sid, taste="Enter"):
        """Nur das Ereignis `Taste` (ohne Umschalter)."""
        st = self._offen()
        self._abonniere(st, sid)
        code = self._ereignis(st, {"art": "Taste", "sitzung": sid, "taste": taste, "umschalter": []})
        return (202, {"ok": True}) if code == 204 else (code, {"error": "eingabe"})

    def beenden(self, sid):
        st = self._offen()
        code = self._ereignis(st, {"art": "Beenden", "sitzung": sid})
        return (200, {"ok": True}) if code == 204 else (code, {"error": "beenden"})

    def ablehnungen_marke(self):
        """Eine Marke fuer `ablehnungen_seit`: die Zahl der bisherigen Ablehnungen im Strom."""
        st = self._offen()
        with st.lock:
            return len(st.ablehnungen)

    def ablehnungen_seit(self, marke):
        """Die Ablehnungen (Grund, Code) des Dienstes seit der Marke. Der Strom wirft seinen Fehler, wenn er kaputt ist."""
        st = self._offen()
        with st.lock:
            return list(st.ablehnungen[marke:])

    def diagnose(self):
        """Zaehler von Startprogramm und Harness. Nur die Attrappen haben diesen Weg (`bruecke.diagnose_pfad`); die echte Bruecke
        kennt ihn nicht, dort ist die Antwort None und der Schritt meldet `offen`."""
        if not self.diagnose_pfad:
            return None
        code, a = self._anfrage("GET", self.diagnose_pfad)
        return a if code == 200 and isinstance(a, dict) else None
