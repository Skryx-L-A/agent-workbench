"""Reference of the `rasterauszug` contract (rust/spec/VERTRAEGE.md). Standard library only.

  ra-ref <fall.json>      or      ra-ref < fall.json      (also: ra-ref -)
  ra-ref --gegenproben    prints the names of all built-in mutants, one per line

Reads one case, writes one JSON object `{"ergebnisse": [...]}` on stdout (exit 0). An invalid case ends with exit 2
and an empty stdout. REF_GEGENPROBE=<mutant> switches one deliberate fault on (counter-check of the case sets).
Every expectation of the case sets is produced by this file; the Deflate bytes are produced by zlib but are never
compared (the runner only inflates them again).
"""
import json
import os
import sys
import zlib

MUT = os.environ.get("REF_GEGENPROBE", "")

MUTANTEN = [
    # RA1 .. RA4: Zeilenform
    "sgr_reset_vergisst_farbe", "sgr_aus_22_nur_fett", "sgr_truecolor_vertauscht", "sgr_leerer_parameter_ueberspringen",
    "sgr_unbekannt_still", "sgr_hell_als_256", "sgr_laeufe_nicht_verschmelzen",
    "sgr_ende_leerzeichen_bleiben", "sgr_ende_leerzeichen_alle", "sgr_steuerzeichen_erlaubt", "sgr_c1_erlaubt",
    "sgr_schreiben_ohne_reset", "sgr_ende_reset_anhaengen", "laeufe_kanon_ungeprueft", "stil_ungeprueft",
    # RA5, RA6, RA8: Abzug, Aenderung, Cursor, invers
    "abzug_alle_zeilen", "abzug_ohne_cursor_reset", "aenderung_leer_senden", "stand_nicht_vorruecken",
    "cursor_immer", "cursor_aenderung_fehlt", "invers_nicht_senden", "invers_allein_kein_rahmen",
    # RA7: Verschiebung M2
    "m2_aus", "m2_schwelle_kosten0", "m2_lauf_mindestens_zwei", "m2_gewinn_eins", "m2_leere_zeilen_zaehlen",
    "m2_letzter_laengster", "m2_beste_nicht_nachfuehren", "m2_unten_falsch", "m2_gewinn_gleich",
    # RA9: Rahmen
    "rahmen_stile_sortiert", "rahmen_vu_gross_zuerst", "rahmen_flags_vertauscht", "rahmen_leere_zeilen_im_abzug",
    "rahmen_stand_fehlt",
    # RA10: Dekoder
    "dek_schub_ungeprueft", "dek_index_ausserhalb_still", "dek_nichtkanonisch_ok", "dek_rest_ok", "dek_nicht_atomar",
    "dek_stand_alt_ok", "dek_reset_fehlt", "dek_vu_nicht_minimal_ok", "dek_reihenfolge_ok",
    "dek_cursor_ungeprueft",
    # RA11: Deflate
    "deflate_endung_bleibt", "deflate_voller_flush", "deflate_neu_je_nachricht", "deflate_grenze_aus",
    # RA12, RA13: Nachrichten, Betrachter
    "von_nicht_geprueft", "stand_nicht_steigend_ok", "zeile_fuehrende_null_ok", "abzug_groesse_1001_ok",
    "vorbild_geteilt", "vergessen_ignoriert", "trennen_behaelt_vorbild", "zeilen_laenge_ungeprueft",
]

MAXRAHMEN = 16777216
MAXGROESSE = 1000
I64 = 2 ** 63
ESC = "\x1b"


def m(name):
    return MUT == name


class Fehler(Exception):
    def __init__(self, code):
        Exception.__init__(self, code)
        self.code = code


class UngueltigerFall(Exception):
    pass


# ---------------------------------------------------------------------------------------------- RA1 .. RA4

def zeichen_ok(c):
    o = ord(c)
    if o < 0x20 or o == 0x7F:
        return m("sgr_steuerzeichen_erlaubt") and o != 0x1B and o != 0x7F
    if 0x80 <= o <= 0x9F:
        return m("sgr_c1_erlaubt")
    return True


def stil_gueltig(fg, bg, fl):
    def farbe(c):
        return 0 <= c <= 256 or 0x01000000 <= c <= 0x01FFFFFF
    return farbe(fg) and farbe(bg) and 0 <= fl < 128


def parameter(roh):
    ps = []
    for tok in roh.split(";"):
        if tok == "":
            ps.append(0)
            continue
        if len(tok) > 3 or not tok.isdigit() or not tok.isascii():
            raise Fehler("sgr_parameter")
        ps.append(int(tok))
    return ps


def wende_sgr(ps, st):
    fg, bg, fl = st
    k = 0
    while k < len(ps):
        v = ps[k]
        if m("sgr_leerer_parameter_ueberspringen") and v == 0 and len(ps) > 1:
            k += 1
            continue
        if v == 0:
            if m("sgr_reset_vergisst_farbe"):
                fl = 0
            else:
                fg, bg, fl = 0, 0, 0
        elif v in (1, 2, 3, 4, 7, 8, 9):
            fl |= {1: 1, 2: 2, 3: 4, 4: 8, 7: 16, 8: 32, 9: 64}[v]
        elif v in (22, 23, 24, 27, 28, 29):
            if v == 22:
                fl &= ~1 if m("sgr_aus_22_nur_fett") else ~3
            else:
                fl &= ~{23: 4, 24: 8, 27: 16, 28: 32, 29: 64}[v]
        elif 30 <= v <= 37:
            fg = 1 + v - 30
        elif 40 <= v <= 47:
            bg = 1 + v - 40
        elif 90 <= v <= 97:
            fg = 9 + v - 90
        elif 100 <= v <= 107:
            bg = 9 + v - 100
        elif v == 39:
            fg = 0
        elif v == 49:
            bg = 0
        elif v in (38, 48):
            if k + 1 >= len(ps):
                raise Fehler("sgr_parameter")
            mode = ps[k + 1]
            if mode == 5:
                if k + 2 >= len(ps) or ps[k + 2] > 255:
                    raise Fehler("sgr_parameter")
                col = 1 + ps[k + 2]
                k += 2
            elif mode == 2:
                if k + 4 >= len(ps) or max(ps[k + 2:k + 5]) > 255:
                    raise Fehler("sgr_parameter")
                r, g, b = ps[k + 2], ps[k + 3], ps[k + 4]
                if m("sgr_truecolor_vertauscht"):
                    r, b = b, r
                col = 0x01000000 | (r << 16) | (g << 8) | b
                k += 4
            else:
                raise Fehler("sgr_parameter")
            if v == 38:
                fg = col
            else:
                bg = col
        else:
            if m("sgr_unbekannt_still"):
                k += 1
                continue
            raise Fehler("sgr_parameter")
        k += 1
    return (fg, bg, fl)


def normalisiere(laeufe):
    """Merge neighbours of equal style, drop empty runs, cut the default-style spaces at the end of the line."""
    out = []
    for fg, bg, fl, text in laeufe:
        if text == "":
            continue
        if out and (out[-1][0], out[-1][1], out[-1][2]) == (fg, bg, fl) and not m("sgr_laeufe_nicht_verschmelzen"):
            out[-1] = (fg, bg, fl, out[-1][3] + text)
        else:
            out.append((fg, bg, fl, text))
    while out and not m("sgr_ende_leerzeichen_bleiben"):
        fg, bg, fl, text = out[-1]
        if (fg, bg, fl) == (0, 0, 0) or m("sgr_ende_leerzeichen_alle"):
            text = text.rstrip(" ")
            if text == "":
                out.pop()
                continue
            out[-1] = (fg, bg, fl, text)
        break
    return tuple(out)


def zeile_lesen(s):
    st = (0, 0, 0)
    teile = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c == ESC:
            if i + 1 >= n:
                raise Fehler("sgr_unvollstaendig")
            if s[i + 1] != "[":
                raise Fehler("esc_ungueltig")
            j = i + 2
            while j < n and (s[j] in "0123456789;" and s[j].isascii()):
                j += 1
            if j >= n:
                raise Fehler("sgr_unvollstaendig")
            if s[j] != "m":
                if "@" <= s[j] <= "~":
                    raise Fehler("sgr_unbekannt")
                raise Fehler("sgr_parameter")
            st = wende_sgr(parameter(s[i + 2:j]), st)
            i = j + 1
            continue
        if not zeichen_ok(c):
            raise Fehler("zeichen_ungueltig")
        teile.append((st[0], st[1], st[2], c))
        i += 1
    return normalisiere(teile)


def farbe_param(c, hintergrund):
    if c >= 0x01000000:
        return "%d;2;%d;%d;%d" % (48 if hintergrund else 38, (c >> 16) & 255, (c >> 8) & 255, c & 255)
    n = c - 1
    if n < 8 and not m("sgr_hell_als_256"):
        return str((40 if hintergrund else 30) + n)
    if n < 16 and not m("sgr_hell_als_256"):
        return str((100 if hintergrund else 90) + n - 8)
    return "%d;5;%d" % (48 if hintergrund else 38, n)


def zeile_schreiben(z):
    out = []
    prev = (0, 0, 0)
    for fg, bg, fl, text in z:
        st = (fg, bg, fl)
        if st != prev:
            if st == (0, 0, 0):
                out.append(ESC + "[0m")
            else:
                ps = [] if m("sgr_schreiben_ohne_reset") else ["0"]
                for bit, code in ((1, 1), (2, 2), (4, 3), (8, 4), (16, 7), (32, 8), (64, 9)):
                    if fl & bit:
                        ps.append(str(code))
                if fg:
                    ps.append(farbe_param(fg, False))
                if bg:
                    ps.append(farbe_param(bg, True))
                out.append(ESC + "[" + ";".join(ps) + "m")
        out.append(text)
        prev = st
    if m("sgr_ende_reset_anhaengen") and prev != (0, 0, 0):
        out.append(ESC + "[0m")
    return "".join(out)


def ganz(x):
    return isinstance(x, int) and not isinstance(x, bool) and -I64 <= x < I64


def laeufe_pruefen(roh):
    """Validate a run list given as JSON ([[fg,bg,fl,text],...]); returns the tuple form or raises Fehler."""
    if not isinstance(roh, list):
        raise UngueltigerFall("laeufe")
    z = []
    for e in roh:
        if not (isinstance(e, list) and len(e) == 4 and ganz(e[0]) and ganz(e[1]) and ganz(e[2]) and isinstance(e[3], str)):
            raise UngueltigerFall("lauf")
        z.append((e[0], e[1], e[2], e[3]))
    for fg, bg, fl, text in z:
        if not stil_gueltig(fg, bg, fl) and not m("stil_ungeprueft"):
            raise Fehler("stil_ungueltig")
    for fg, bg, fl, text in z:
        for c in text:
            if not zeichen_ok(c):
                raise Fehler("zeichen_ungueltig")
    if not m("laeufe_kanon_ungeprueft") and kanonisch_fehler(tuple(z)):
        raise Fehler("zeile_nicht_kanonisch")
    return tuple(z)


def kanonisch_fehler(z):
    for k, (fg, bg, fl, text) in enumerate(z):
        if text == "":
            return True
        if k and (z[k - 1][0], z[k - 1][1], z[k - 1][2]) == (fg, bg, fl):
            return True
    if z and (z[-1][0], z[-1][1], z[-1][2]) == (0, 0, 0) and z[-1][3].endswith(" "):
        return True
    return False


# ---------------------------------------------------------------------------------------------- Bild, Auszug (RA5 .. RA8)

class Bild:
    def __init__(self, spalten, zeilen, lines, invers, cursor, stand):
        self.spalten, self.zeilen = spalten, zeilen
        self.lines = lines
        self.invers = invers
        self.cursor = cursor
        self.stand = stand


def schub_zeile(alt, i, a, b, sh):
    if a <= i < b:
        return alt[i + sh]
    if b <= i < b + sh:
        return ()
    return alt[i]


def berechne_diff(alt, neu):
    """Models M1/M2 of the measuring runner (modell.rs `diff`), on the line lists only; returns (schub, zeilen)."""
    n = len(neu)
    leer = ()
    kosten0 = sum(1 for i in range(n) if alt[i] != neu[i])
    schub = None
    beste = kosten0
    schwelle = 2 if m("m2_schwelle_kosten0") else 3
    if not m("m2_aus") and kosten0 > schwelle:
        for sh in range(1, n):
            bs = bl = 0
            start = lang = 0
            for i in range(0, n - sh):
                if neu[i] == alt[i + sh] and (neu[i] != leer or m("m2_leere_zeilen_zaehlen")):
                    if lang == 0:
                        start = i
                    lang += 1
                    if (lang >= bl) if m("m2_letzter_laengster") else (lang > bl):
                        bl = lang
                        bs = start
                else:
                    lang = 0
            if bl < (2 if m("m2_lauf_mindestens_zwei") else 3):
                continue
            a, b = bs, bs + bl
            kosten = sum(1 for i in range(n) if schub_zeile(alt, i, a, b, sh) != neu[i])
            if m("m2_gewinn_gleich"):
                ok = kosten + 2 <= beste
            else:
                ok = kosten + (1 if m("m2_gewinn_eins") else 2) < beste
            if ok:
                if not m("m2_beste_nicht_nachfuehren"):
                    beste = kosten
                schub = (a, (b if m("m2_unten_falsch") else b + sh), sh)
    zeilen = []
    for i in range(n):
        if schub is not None:
            a, u, sh = schub
            b = u - sh if not m("m2_unten_falsch") else u
            h = schub_zeile(alt, i, a, b, sh)
        else:
            h = alt[i]
        if h != neu[i]:
            zeilen.append((i, neu[i]))
    return schub, zeilen


class Auszug:
    def __init__(self, nr, stand):
        self.nr = nr
        self.stand = stand
        self.groesse = None
        self.invers = False
        self.schub = None
        self.cursor = None
        self.zeilen = []


def cursor_pruefen(c, spalten, zeilen):
    z, s, v = c
    if not (0 <= z < zeilen and 0 <= s < spalten):
        raise Fehler("cursor_ungueltig")


def cursor_aus_nachricht(msg):
    if "cursor" not in msg:
        return None
    c = msg["cursor"]
    if not (isinstance(c, list) and len(c) == 3 and ganz(c[0]) and ganz(c[1]) and isinstance(c[2], bool)):
        raise Fehler("nachricht_ungueltig")
    return (c[0], c[1], c[2])


# ---------------------------------------------------------------------------------------------- RA9 Rahmen (Bytes)

def vu(n):
    out = bytearray()
    while n >= 0x80:
        out.append((n & 0x7F) | 0x80)
        n >>= 7
    out.append(n)
    if m("rahmen_vu_gross_zuerst"):
        out.reverse()
    return bytes(out)


def rahmen_kodieren(a):
    flags = (1 if a.schub else 0) | (2 if a.cursor else 0) | (4 if a.groesse else 0) | (8 if a.invers else 0)
    if m("rahmen_flags_vertauscht"):
        flags = (1 if a.cursor else 0) | (2 if a.schub else 0) | (4 if a.groesse else 0) | (8 if a.invers else 0)
    if m("invers_nicht_senden"):
        flags &= ~8
    out = bytearray([5, a.nr, flags])
    if not m("rahmen_stand_fehlt"):
        out += vu(a.stand)
    if a.groesse:
        out += vu(a.groesse[0]) + vu(a.groesse[1])
    if a.schub:
        for x in a.schub:
            out += vu(x)
    if a.cursor:
        out += vu(a.cursor[0]) + vu(a.cursor[1]) + vu(1 if a.cursor[2] else 0)
    stile = []
    for _, z in a.zeilen:
        for fg, bg, fl, text in z:
            if (fg, bg, fl) not in stile:
                stile.append((fg, bg, fl))
    if m("rahmen_stile_sortiert"):
        stile.sort()
    out += vu(len(stile))
    for s in stile:
        out += vu(s[0]) + vu(s[1]) + vu(s[2])
    out += vu(len(a.zeilen))
    for i, z in a.zeilen:
        out += vu(i) + vu(len(z))
        for fg, bg, fl, text in z:
            t = text.encode("utf-8")
            out += vu(stile.index((fg, bg, fl))) + vu(len(t)) + t
    return bytes(out)


class Leser:
    def __init__(self, b):
        self.b = b
        self.p = 0

    def rest(self):
        return len(self.b) - self.p

    def byte(self):
        if self.p >= len(self.b):
            raise Fehler("rahmen_abgeschnitten")
        x = self.b[self.p]
        self.p += 1
        return x

    def vu(self):
        n = 0
        sh = 0
        for k in range(10):
            x = self.byte()
            n |= (x & 0x7F) << sh
            if not x & 0x80:
                if k > 0 and x == 0 and not m("dek_vu_nicht_minimal_ok"):
                    raise Fehler("vu_ungueltig")
                if n >= 2 ** 64:
                    raise Fehler("vu_ungueltig")
                return n
            sh += 7
        raise Fehler("vu_ungueltig")

    def zahl(self):
        """A count: cannot exceed the bytes still unread (each element needs at least one byte)."""
        n = self.vu()
        if n > self.rest() and not m("dek_zahl_ungeprueft"):
            raise Fehler("rahmen_abgeschnitten")
        return n

    def bytes_(self, n):
        if n > self.rest():
            raise Fehler("rahmen_abgeschnitten")
        x = self.b[self.p:self.p + n]
        self.p += n
        return x


class Abbild:
    def __init__(self, spalten, zeilen):
        self.spalten, self.zeilen = spalten, zeilen
        self.lines = [()] * zeilen
        self.invers = False
        self.cursor = (0, 0, False)
        self.stand = 0


def rahmen_dekodieren(b, abbilder):
    """Decode and check a frame against the receiver state; returns (Auszug, new Abbild). Never mutates `abbilder`."""
    p = Leser(b)
    if p.byte() != 5:
        raise Fehler("art_ungueltig")
    nr = p.byte()
    flags = p.byte()
    if flags & ~15:
        raise Fehler("flags_unbekannt")
    stand = p.vu()
    a = Auszug(nr, stand)
    a.invers = bool(flags & 8)
    voll = bool(flags & 4)
    if voll:
        sp, ze = p.vu(), p.vu()
        if not (1 <= sp <= MAXGROESSE and 1 <= ze <= MAXGROESSE) and not (m("abzug_groesse_1001_ok") and sp <= 1001 and ze <= 1001):
            raise Fehler("groesse_ungueltig")
        if flags & 1:
            raise Fehler("rahmen_ungueltig")
        a.groesse = (sp, ze)
        alt = None
        spalten, zeilen = sp, ze
    else:
        alt = abbilder.get(nr)
        if alt is None:
            raise Fehler("kein_abzug")
        if stand <= alt.stand and not m("dek_stand_alt_ok"):
            raise Fehler("stand_alt")
        spalten, zeilen = alt.spalten, alt.zeilen
    if flags & 1:
        o, u, n = p.vu(), p.vu(), p.vu()
        if not (n >= 1 and o + n <= u and u <= zeilen) and not m("dek_schub_ungeprueft"):
            raise Fehler("schub_ungueltig")
        a.schub = (o, u, n)
    if flags & 2:
        z, s, v = p.vu(), p.vu(), p.vu()
        if v > 1:
            raise Fehler("cursor_ungueltig")
        if not m("dek_cursor_ungeprueft"):
            cursor_pruefen((z, s, v), spalten, zeilen)
        a.cursor = (z, s, bool(v))
    ns = p.zahl()
    stile = []
    for _ in range(ns):
        fg, bg, fl = p.vu(), p.vu(), p.vu()
        if not stil_gueltig(fg, bg, fl):
            raise Fehler("stil_ungueltig")
        stile.append((fg, bg, fl))
    nz = p.zahl()
    letzte = -1
    for _ in range(nz):
        i = p.vu()
        if i >= zeilen:
            if m("dek_index_ausserhalb_still"):
                letzte = i
                nl = p.zahl()
                for _k in range(nl):
                    p.vu()
                    p.bytes_(p.zahl())
                continue
            raise Fehler("zeile_ausserhalb")
        if i <= letzte and not m("dek_reihenfolge_ok"):
            raise Fehler("zeile_reihenfolge")
        letzte = i
        nl = p.zahl()
        z = []
        for _k in range(nl):
            sn = p.vu()
            if sn >= len(stile):
                raise Fehler("stilnr_ungueltig")
            raw = p.bytes_(p.zahl())
            try:
                text = raw.decode("utf-8")
            except UnicodeDecodeError:
                raise Fehler("utf8_ungueltig")
            for c in text:
                if not zeichen_ok(c):
                    raise Fehler("zeichen_ungueltig")
            z.append((stile[sn][0], stile[sn][1], stile[sn][2], text))
        z = tuple(z)
        if kanonisch_fehler(z) and not m("dek_nichtkanonisch_ok"):
            raise Fehler("zeile_nicht_kanonisch")
        a.zeilen.append((i, z))
    if p.rest() and not m("dek_rest_ok"):
        raise Fehler("rahmen_rest")
    # apply on a copy
    if voll:
        neu = Abbild(spalten, zeilen)
        if m("dek_reset_fehlt") and abbilder.get(nr) is not None and (abbilder[nr].spalten, abbilder[nr].zeilen) == (spalten, zeilen):
            neu.lines = list(abbilder[nr].lines)
    else:
        neu = Abbild(spalten, zeilen)
        neu.lines = list(alt.lines)
        neu.cursor = alt.cursor
    if a.schub:
        o, u, n = a.schub
        for i in range(o, u - n):
            neu.lines[i] = neu.lines[i + n]
        for i in range(u - n, u):
            neu.lines[i] = ()
    for i, z in a.zeilen:
        neu.lines[i] = z
    if a.cursor:
        neu.cursor = a.cursor
    neu.invers = a.invers
    neu.stand = stand
    return a, neu


# ---------------------------------------------------------------------------------------------- Verbindung, Empfang

GETEILT = {}


class Verbindung:
    def __init__(self, deflate):
        self.deflate = deflate
        self.vorbild = GETEILT if m("vorbild_geteilt") else (dict(GETEILT) if m("trennen_behaelt_vorbild") else {})
        self.comp = self.neuer_kompressor() if deflate else None
        self.empfang = {}
        self.decomp = zlib.decompressobj(-15) if deflate else None
        self.max_rahmen = MAXRAHMEN

    @staticmethod
    def neuer_kompressor():
        return zlib.compressobj(6, zlib.DEFLATED, -15)

    def verpacken(self, roh):
        if not self.deflate:
            return roh
        if m("deflate_neu_je_nachricht"):
            self.comp = self.neuer_kompressor()
        c = self.comp
        out = c.compress(roh) + c.flush(zlib.Z_FULL_FLUSH if m("deflate_voller_flush") else zlib.Z_SYNC_FLUSH)
        if out.endswith(b"\x00\x00\xff\xff") and not m("deflate_endung_bleibt"):
            out = out[:-4]
        return out

    def auspacken(self, wire):
        if not self.deflate:
            if len(wire) > MAXRAHMEN:
                raise Fehler("rahmen_zu_gross")
            return wire
        grenze = MAXRAHMEN if not m("deflate_grenze_aus") else 1 << 40
        try:
            out = self.decomp.decompress(wire + b"\x00\x00\xff\xff", grenze + 1)
        except zlib.error:
            raise Fehler("inflate_fehler")
        if len(out) > grenze or self.decomp.unconsumed_tail:
            raise Fehler("rahmen_zu_gross")
        return out

    # ---- sender side ----
    def nimm(self, nr, msg):
        """Returns the uncompressed frame bytes or None. Changes the state only on success."""
        if not isinstance(msg, dict):
            raise Fehler("nachricht_ungueltig")
        art = msg.get("art")
        if art == "Abzug":
            return self.abzug(nr, msg)
        if art == "Rasteraenderung":
            return self.aenderung(nr, msg)
        if isinstance(art, str):
            raise Fehler("art_unbekannt")
        raise Fehler("nachricht_ungueltig")

    def abzug(self, nr, msg):
        for k in ("sitzung", "stand", "groesse", "invers", "zeilen"):
            if k not in msg:
                raise Fehler("nachricht_ungueltig")
        g = msg["groesse"]
        if not (isinstance(msg["sitzung"], str) and ganz(msg["stand"]) and msg["stand"] >= 0 and isinstance(msg["invers"], bool)
                and isinstance(g, list) and len(g) == 2 and ganz(g[0]) and ganz(g[1]) and isinstance(msg["zeilen"], list)
                and all(isinstance(x, str) for x in msg["zeilen"])):
            raise Fehler("nachricht_ungueltig")
        sp, ze = g
        if not (1 <= sp <= MAXGROESSE and 1 <= ze <= MAXGROESSE) and not (m("abzug_groesse_1001_ok") and sp <= 1001 and ze <= 1001):
            raise Fehler("groesse_ungueltig")
        if len(msg["zeilen"]) != ze and not m("zeilen_laenge_ungeprueft"):
            raise Fehler("nachricht_ungueltig")
        cursor = cursor_aus_nachricht(msg)
        if cursor is not None:
            cursor_pruefen(cursor, sp, ze)
        lines = []
        for s in msg["zeilen"]:
            try:
                lines.append(zeile_lesen(s))
            except Fehler:
                raise Fehler("zeile_ungueltig")
        lines = (lines + [()] * ze)[:ze]
        a = Auszug(nr, msg["stand"])
        a.groesse = (sp, ze)
        a.invers = msg["invers"]
        a.cursor = cursor
        a.zeilen = [(i, z) for i, z in enumerate(lines) if z != () or m("rahmen_leere_zeilen_im_abzug") or m("abzug_alle_zeilen")]
        if m("abzug_ohne_cursor_reset") and cursor is None and nr in self.vorbild:
            cursor = self.vorbild[nr].cursor
        self.vorbild[nr] = Bild(sp, ze, lines, msg["invers"], cursor if cursor is not None else (0, 0, False), msg["stand"])
        return rahmen_kodieren(a)

    def aenderung(self, nr, msg):
        for k in ("sitzung", "von", "stand", "invers", "zeilen"):
            if k not in msg:
                raise Fehler("nachricht_ungueltig")
        zl = msg["zeilen"]
        if not (isinstance(msg["sitzung"], str) and ganz(msg["von"]) and msg["von"] >= 0 and ganz(msg["stand"]) and msg["stand"] >= 0
                and isinstance(msg["invers"], bool) and isinstance(zl, dict) and all(isinstance(v, str) for v in zl.values())):
            raise Fehler("nachricht_ungueltig")
        v = self.vorbild.get(nr)
        if v is None:
            raise Fehler("kein_abzug")
        if msg["von"] != v.stand and not m("von_nicht_geprueft"):
            raise Fehler("luecke")
        if msg["stand"] <= msg["von"] and not m("stand_nicht_steigend_ok"):
            raise Fehler("stand_alt")
        cursor = cursor_aus_nachricht(msg)
        if cursor is not None:
            cursor_pruefen(cursor, v.spalten, v.zeilen)
        neue = {}
        for k, s in zl.items():
            ok = k.isdigit() and k.isascii() and (k == "0" or not k.startswith("0") or m("zeile_fuehrende_null_ok"))
            if not ok or int(k) >= v.zeilen:
                raise Fehler("zeile_ausserhalb")
            try:
                neue[int(k)] = zeile_lesen(s)
            except Fehler:
                raise Fehler("zeile_ungueltig")
        neu_lines = list(v.lines)
        for i, z in neue.items():
            neu_lines[i] = z
        schub, zeilen = berechne_diff(v.lines, neu_lines)
        a = Auszug(nr, msg["stand"])
        a.invers = msg["invers"]
        a.schub = schub
        a.zeilen = zeilen
        if cursor is not None and (cursor != v.cursor or m("cursor_immer")) and not m("cursor_aenderung_fehlt"):
            a.cursor = cursor
        if m("cursor_immer") and cursor is None:
            a.cursor = v.cursor
        nichts = not zeilen and schub is None and a.cursor is None
        invers_neu = msg["invers"] != v.invers
        raus = None
        if not nichts or (invers_neu and not m("invers_allein_kein_rahmen")) or m("aenderung_leer_senden"):
            raus = rahmen_kodieren(a)
        v.lines = neu_lines
        v.invers = msg["invers"]
        if cursor is not None:
            v.cursor = cursor
        if not m("stand_nicht_vorruecken") or raus is not None:
            v.stand = msg["stand"]
        return raus

    def vergessen(self, nr):
        if m("vergessen_ignoriert"):
            return
        self.vorbild.pop(nr, None)
        self.empfang.pop(nr, None)

    # ---- receiver side ----
    def empfange(self, wire):
        roh = self.auspacken(wire)
        try:
            a, neu = rahmen_dekodieren(roh, self.empfang)
        except Fehler:
            if m("dek_nicht_atomar") and roh[:1] == b"\x05" and len(roh) > 3:
                self.empfang.setdefault(roh[1], Abbild(1, 1)).stand = 10 ** 6
            raise
        self.empfang[a.nr] = neu
        return roh, a, neu


# ---------------------------------------------------------------------------------------------- Ausgabe und Schritte

def j_cursor(c):
    return None if c is None else [c[0], c[1], bool(c[2])]


def j_auszug(a):
    return {"nr": a.nr, "stand": a.stand, "groesse": list(a.groesse) if a.groesse else None, "invers": a.invers,
            "schub": list(a.schub) if a.schub else None, "cursor": j_cursor(a.cursor),
            "zeilen": [[i, zeile_schreiben(z)] for i, z in a.zeilen]}


def j_abbild(nr, ab):
    return {"nr": nr, "stand": ab.stand, "groesse": [ab.spalten, ab.zeilen], "invers": ab.invers,
            "cursor": j_cursor(ab.cursor), "zeilen": [zeile_schreiben(z) for z in ab.lines]}


def abbild_gleich_vorbild(v, ab):
    return (v.spalten, v.zeilen, v.invers, v.cursor, v.lines) == (ab.spalten, ab.zeilen, ab.invers, ab.cursor, ab.lines)


def feld(schritt, name, typ):
    if name not in schritt or not typ(schritt[name]):
        raise UngueltigerFall("feld " + name)
    return schritt[name]


def ist_nr(x):
    return ganz(x) and 0 <= x <= 255


def ist_str(x):
    return isinstance(x, str)


def ist_bool(x):
    return isinstance(x, bool)


def hex_lesen(s):
    if len(s) % 2 or any(c not in "0123456789abcdef" for c in s):
        raise UngueltigerFall("hex")
    return bytes.fromhex(s)


def ausfuehren(fall):
    if not isinstance(fall, dict) or not isinstance(fall.get("schritte"), list):
        raise UngueltigerFall("schritte")
    verb = {}
    ergebnisse = []
    for sch in fall["schritte"]:
        if not isinstance(sch, dict) or not isinstance(sch.get("art"), str):
            raise UngueltigerFall("schritt")
        art = sch["art"]
        if art == "Zeile":
            s = feld(sch, "sgr", ist_str)
            try:
                z = zeile_lesen(s)
            except Fehler as e:
                ergebnisse.append({"ok": False, "fehler": e.code})
                continue
            kanon = zeile_schreiben(z)
            ergebnisse.append({"ok": True, "laeufe": [list(x) for x in z], "sgr": kanon,
                               "rundreise": zeile_lesen(kanon) == z})
        elif art == "Laeufe":
            try:
                z = laeufe_pruefen(sch.get("laeufe"))
            except Fehler as e:
                ergebnisse.append({"ok": False, "fehler": e.code})
                continue
            kanon = zeile_schreiben(z)
            try:
                rund = zeile_lesen(kanon) == z
            except Fehler:
                rund = False
            ergebnisse.append({"ok": True, "sgr": kanon, "rundreise": rund})
        elif art == "Verbinden":
            vid = feld(sch, "verbindung", ist_str)
            if vid in verb:
                raise UngueltigerFall("doppelte Verbindung")
            verb[vid] = Verbindung(feld(sch, "deflate", ist_bool))
            ergebnisse.append({"ok": True})
        elif art in ("Nachricht", "Rahmen", "Vergessen", "Trennen"):
            vid = feld(sch, "verbindung", ist_str)
            if vid not in verb:
                raise UngueltigerFall("unbekannte Verbindung")
            v = verb[vid]
            if art == "Trennen":
                if m("trennen_behaelt_vorbild"):
                    GETEILT.update(v.vorbild)
                del verb[vid]
                ergebnisse.append({"ok": True})
            elif art == "Vergessen":
                v.vergessen(feld(sch, "nr", ist_nr))
                ergebnisse.append({"ok": True})
            elif art == "Nachricht":
                nr = feld(sch, "nr", ist_nr)
                if "nachricht" not in sch:
                    raise UngueltigerFall("nachricht")
                try:
                    roh = v.nimm(nr, sch["nachricht"])
                except Fehler as e:
                    ergebnisse.append({"ok": False, "fehler": e.code})
                    continue
                if roh is None:
                    ab = v.empfang.get(nr)
                    ergebnisse.append({"ok": True, "hex": None, "auszug": None, "abbild": j_abbild(nr, ab) if ab else None,
                                       "rundreise": True, "leitung_hex": None, "leitung_laenge": 0})
                    continue
                wire = v.verpacken(roh)
                try:
                    _, a, neu = v.empfange(wire)
                except Fehler as e:
                    ergebnisse.append({"ok": False, "fehler": e.code})
                    continue
                ergebnisse.append({"ok": True, "hex": roh.hex(), "auszug": j_auszug(a), "abbild": j_abbild(nr, neu),
                                   "rundreise": abbild_gleich_vorbild(v.vorbild[nr], neu),
                                   "leitung_hex": wire.hex(), "leitung_laenge": len(wire)})
            else:  # Rahmen
                wire = hex_lesen(feld(sch, "hex", ist_str))
                try:
                    roh, a, neu = v.empfange(wire)
                except Fehler as e:
                    ergebnisse.append({"ok": False, "fehler": e.code})
                    continue
                ergebnisse.append({"ok": True, "hex": roh.hex(), "auszug": j_auszug(a), "abbild": j_abbild(a.nr, neu)})
        else:
            raise UngueltigerFall("art " + art)
    return {"ergebnisse": ergebnisse}


def main(argv):
    if "--gegenproben" in argv:
        for n in MUTANTEN:
            print(n)
        return 0
    args = [a for a in argv[1:] if a != "-"]
    try:
        roh = open(args[0], "rb").read() if args else sys.stdin.buffer.read()
        if len(roh) > 4 * 1024 * 1024:
            raise UngueltigerFall("zu gross")

        def nein(c):
            raise UngueltigerFall("konstante " + c)

        fall = json.loads(roh.decode("utf-8"), parse_constant=nein)
        aus = ausfuehren(fall)
    except (UngueltigerFall, ValueError, RecursionError, OSError) as e:
        sys.stderr.write("ungueltiger Fall: %s\n" % e)
        return 2
    sys.stdout.write(json.dumps(aus, ensure_ascii=False) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
