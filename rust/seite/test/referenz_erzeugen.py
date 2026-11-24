"""Erzeugt test/daten/gleichheit.json.gz (gzip, ohne Zeitstempel): die Empfangsseite der Referenz `ra_ref.py` (Vertrag `rasterauszug`), Schritt fuer Schritt.

  RA_REF=<ra_ref.py> RA_VERTRAG=<Ordner mit aussentests/> [RA_SAETZE="aussentests verdeckt"] python3 -I -B referenz_erzeugen.py <ausgabe.json.gz>

Es braucht die Referenz nur zum Erzeugen; der Test selbst (gleichheit.mjs) liest die fertige Datei. Jeder Eintrag der Datei ist ein Aufruf
von `Verbindung.empfange` der Referenz: die Bytes auf der Leitung und das Ergebnis (Bild und Auszug als Laeufe, oder der Fehlercode).
Quellen der Eintraege:
  - die Faelle des Vertrags aus den Saetzen in `RA_SAETZE` (Vorgabe: nur `aussentests`), soweit sie eine Empfangsseite haben (Schritte `Nachricht`,
    `Rahmen`). Die verdeckten Faelle des Vertrags duerfen nie in eine Datei des Repos: `RA_SAETZE` mit dem verdeckten Satz nur fuer eine Ausgabe
    ausserhalb des Repos (`lauf.sh --neu`);
  - eine Zufallsprobe mit festem Samen: Sitzungen mit Abzug, Aenderung, Verschiebung, Cursor, invers, Deflate in den Stufen 0 bis 9,
    dazu beschaedigte Rahmen (Bitfehler, Kuerzung, Anhang, Austausch eines Bytes, mit und ohne Deflate). Ein beschaedigter Rahmen steht hinter
    einem Eintrag `isoliert`: Der Empfaenger probiert ihn an einer Kopie seines Zustands, der echte Zustand bleibt.
Die Datei enthaelt keine Zeit und keinen Pfad; der Samen ist fest, die Ausgabe haengt nur von der Referenz und von zlib ab.
"""
import gzip
import hashlib
import importlib.util
import json
import os
import random
import sys
import zlib
from pathlib import Path

SAMEN = 20261120
ZUFALL_SITZUNGEN = 250

ref_pfad = Path(os.environ["RA_REF"])
vertrag = Path(os.environ["RA_VERTRAG"])
spec = importlib.util.spec_from_file_location("ra_ref", ref_pfad)
RA = importlib.util.module_from_spec(spec)
spec.loader.exec_module(RA)


def laeufe(z):
    return [[fg, bg, fl, t] for fg, bg, fl, t in z]


def auszug_json(a):
    return {"nr": a.nr, "stand": str(a.stand), "groesse": list(a.groesse) if a.groesse else None, "invers": a.invers,
            "schub": list(a.schub) if a.schub else None,
            "cursor": None if a.cursor is None else [a.cursor[0], a.cursor[1], bool(a.cursor[2])],
            "zeilen": [[i, laeufe(z)] for i, z in a.zeilen]}


def abbild_json(ab):
    return {"nr": None, "stand": str(ab.stand), "groesse": [ab.spalten, ab.zeilen], "invers": ab.invers,
            "cursor": [ab.cursor[0], ab.cursor[1], bool(ab.cursor[2])], "zeilen": [laeufe(z) for z in ab.lines]}


class Schreiber:
    """Haengt sich in `Verbindung` ein und merkt sich jeden Empfang und jedes Vergessen."""

    def __init__(self):
        self.ereignisse = []
        self.zaehler = 0
        orig_init = RA.Verbindung.__init__
        orig_emp = RA.Verbindung.empfange
        orig_verg = RA.Verbindung.vergessen
        s = self

        def init(selbst, deflate):
            orig_init(selbst, deflate)
            selbst._kennung = s.zaehler
            s.zaehler += 1
            s.ereignisse.append({"art": "verbinden", "verbindung": selbst._kennung, "deflate": bool(deflate)})

        def emp(selbst, wire):
            try:
                roh, a, neu = orig_emp(selbst, wire)
            except RA.Fehler as e:
                s.ereignisse.append({"art": "rahmen", "verbindung": selbst._kennung, "draht": bytes(wire).hex(),
                                     "erwartet": {"ok": False, "fehler": e.code}})
                raise
            b = abbild_json(neu)
            b["nr"] = a.nr
            s.ereignisse.append({"art": "rahmen", "verbindung": selbst._kennung, "draht": bytes(wire).hex(),
                                 "erwartet": {"ok": True, "auszug": auszug_json(a), "abbild": b}})
            return roh, a, neu

        def verg(selbst, nr):
            if not RA.m("vergessen_ignoriert"):
                s.ereignisse.append({"art": "vergessen", "verbindung": selbst._kennung, "nr": nr})
            return orig_verg(selbst, nr)

        RA.Verbindung.__init__ = init
        RA.Verbindung.empfange = emp
        RA.Verbindung.vergessen = verg


def faelle_lesen():
    ergebnis = []
    ausgelassen = []
    for satz in os.environ.get("RA_SAETZE", "aussentests").split():
        for d in sorted((vertrag / satz / "faelle").glob("*.json")):
            ergebnis.append((f"{satz}/{d.stem}", d))
    return ergebnis, ausgelassen


def vertragsfaelle(sch):
    eintraege = []
    ohne_empfang = []
    ungueltig = []
    for name, pfad in faelle_lesen()[0]:
        start = len(sch.ereignisse)
        try:
            def nein(c):
                raise RA.UngueltigerFall(c)
            fall = json.loads(pfad.read_bytes().decode("utf-8"), parse_constant=nein)
            RA.ausfuehren(fall)
        except (RA.UngueltigerFall, ValueError, RecursionError):
            del sch.ereignisse[start:]
            ungueltig.append(name)
            continue
        ev = sch.ereignisse[start:]
        if not any(e["art"] == "rahmen" for e in ev):
            ohne_empfang.append(name)
            del sch.ereignisse[start:]
            continue
        eintraege.append({"name": name, "ereignisse": ev})
        del sch.ereignisse[start:]
    return eintraege, ohne_empfang, ungueltig


# ------------------------------------------------------------------------------------------------ Zufallsprobe

WOERTER = ["alpha", "beta", "x", "Grüße äöü", "漢字", "👍🏽", "é", "a b", "﻿kopf", "ein Satz mit Raum", "~/AI $ ", "0123456789", "  "]


def zufall_stil(rng):
    def farbe():
        w = rng.random()
        if w < 0.5:
            return 0
        if w < 0.8:
            return rng.randrange(1, 17)
        if w < 0.9:
            return rng.randrange(17, 257)
        return 0x01000000 | rng.randrange(1 << 24)
    return (farbe(), farbe(), rng.choice([0, 0, 0, 1, 2, 4, 8, 16, 32, 64, rng.randrange(128)]))


def zufall_zeile(rng, pool):
    if rng.random() < 0.15:
        return ""
    teile = []
    for _ in range(rng.randrange(1, 5)):
        st = rng.choice(pool)
        teile.append((st[0], st[1], st[2], rng.choice(WOERTER)))
    return RA.zeile_schreiben(RA.normalisiere(teile))


def zufall_bild(rng, pool, ze):
    return [zufall_zeile(rng, pool) for _ in range(ze)]


def beschaedigen(rng, b):
    b = bytearray(b)
    w = rng.randrange(6)
    if w == 0 and b:
        b[rng.randrange(len(b))] ^= 1 << rng.randrange(8)
    elif w == 1 and b:
        return bytes(b[:rng.randrange(len(b))])
    elif w == 2:
        b += bytes(rng.randrange(256) for _ in range(rng.randrange(1, 6)))
    elif w == 3 and b:
        b[rng.randrange(len(b))] = rng.randrange(256)
    elif w == 4 and len(b) > 3:
        b[rng.randrange(min(len(b), 4))] = rng.randrange(256)  # Kopf: Art, Nummer, Marken, Stand
    elif w == 5 and b:
        k = rng.randrange(len(b))
        del b[k:k + rng.randrange(1, 4)]
    return bytes(b)


def zufallsprobe(sch):
    rng = random.Random(SAMEN)
    eintraege = []
    for k in range(ZUFALL_SITZUNGEN):
        start = len(sch.ereignisse)
        deflate = rng.random() < 0.6
        v = RA.Verbindung(deflate)
        if deflate:
            v.comp = zlib.compressobj(rng.randrange(0, 10), zlib.DEFLATED, -15, rng.randrange(1, 10), rng.choice([0, 0, 1, 2, 3, 4]))
        pool = [zufall_stil(rng) for _ in range(rng.randrange(2, 9))] + [(0, 0, 0)] * 3
        sitzungen = {}
        stand = {}
        for _ in range(rng.randrange(5, 30)):
            nr = rng.choice([0, 1, 7, 200, 255]) if rng.random() < 0.5 else rng.choice(list(sitzungen) or [0])
            if rng.random() < 0.05:
                v.vergessen(nr)
                sitzungen.pop(nr, None)
                continue
            if nr not in sitzungen or rng.random() < 0.1:
                sp, ze = rng.randrange(2, 90), rng.randrange(1, 40)
                if rng.random() < 0.05:
                    sp, ze = 2, 1
                bild = zufall_bild(rng, pool, ze)
                st = stand.get(nr, 0) + rng.randrange(1, 5)
                msg = {"art": "Abzug", "sitzung": f"s{nr}", "stand": st, "groesse": [sp, ze], "invers": rng.random() < 0.2, "zeilen": bild}
                if rng.random() < 0.7:
                    msg["cursor"] = [rng.randrange(ze), rng.randrange(sp), rng.random() < 0.7]
                kandidat = (sp, ze, bild)
            else:
                sp, ze, bild = sitzungen[nr]
                neu = list(bild)
                art = rng.randrange(4)
                if art == 0 and ze >= 8:  # nach oben rollen: soll die Verschiebung ausloesen
                    sh = rng.randrange(1, max(2, ze // 3))
                    a = rng.randrange(0, ze // 3)
                    b = rng.randrange(ze - ze // 3, ze + 1)
                    seg = neu[a + sh:b] + [zufall_zeile(rng, pool) for _ in range(sh)]
                    neu[a:b] = seg[:b - a]
                elif art == 1:
                    for _ in range(rng.randrange(1, 4)):
                        neu[rng.randrange(ze)] = zufall_zeile(rng, pool)
                elif art == 2:
                    pass  # unveraendert: kein Rahmen
                else:
                    neu = zufall_bild(rng, pool, ze)
                zl = {str(i): neu[i] for i in range(ze) if neu[i] != bild[i] or rng.random() < 0.05}
                st = stand.get(nr, 0)
                vb = v.vorbild.get(nr)
                von = vb.stand if vb else st
                msg = {"art": "Rasteraenderung", "sitzung": f"s{nr}", "von": von, "stand": von + rng.randrange(1, 4), "invers": rng.random() < 0.2, "zeilen": zl}
                if rng.random() < 0.4:
                    msg["cursor"] = [rng.randrange(ze), rng.randrange(sp), rng.random() < 0.7]
                kandidat = (sp, ze, neu)
            try:
                roh = v.nimm(nr, msg)
            except RA.Fehler:
                continue
            sitzungen[nr] = kandidat
            stand[nr] = msg["stand"]
            if roh is None:
                continue
            wire = v.verpacken(roh)
            if rng.random() < 0.5:
                # Beschaedigte Rahmen gegen den jetzigen Zustand: danach wird der Zustand der Verbindung wiederhergestellt
                # (Deflate-Kontext ueber `copy()`), damit die Probe nicht nach dem ersten Bitfehler nur noch `inflate_fehler` sieht.
                for _ in range(rng.randrange(1, 4)):
                    schlecht = beschaedigen(rng, wire)
                    gerettet = (v.decomp.copy() if v.decomp else None, dict(v.empfang))
                    sch.ereignisse.append({"art": "isoliert", "verbindung": v._kennung})
                    try:
                        v.empfange(schlecht)
                    except RA.Fehler:
                        pass
                    v.decomp, v.empfang = gerettet
            try:
                v.empfange(wire)
            except RA.Fehler:
                pass
        eintraege.append({"name": f"zufall/{k}", "ereignisse": sch.ereignisse[start:]})
        del sch.ereignisse[start:]
    return eintraege


def sgr_tabelle():
    """Jede SGR-Nummer aus RA1 (und einige Folgen), von der Referenz gelesen: Text -> Laeufe. Dient dem Test von `zeileZuHtml`."""
    nummern = [0, 1, 2, 3, 4, 7, 8, 9, 22, 23, 24, 27, 28, 29, 39, 49]
    nummern += list(range(30, 38)) + list(range(40, 48)) + list(range(90, 98)) + list(range(100, 108))
    folgen = [str(n) for n in nummern]
    folgen += [f"{c};5;{n}" for c in (38, 48) for n in (0, 1, 7, 8, 15, 16, 21, 100, 231, 232, 255)]
    folgen += [f"{c};2;{r};{g};{b}" for c in (38, 48) for r, g, b in ((0, 0, 0), (255, 255, 255), (1, 2, 3), (18, 52, 86), (255, 0, 128))]
    folgen += ["1;31;44", "7;31", "7;41", "7;38;5;100;48;5;200", "7;38;2;1;2;3;48;2;4;5;6", "4;9", "1;2;3;4;7;8;9", "31;1;0", "1;22", "2;22", "7;27", "8;28", "9;29", "4;24", "3;23"]
    ergebnis = []
    for f in folgen:
        sgr = f"\x1b[{f}mX"
        z = RA.zeile_lesen(sgr)
        ergebnis.append({"sgr": sgr, "laeufe": laeufe(z)})
    # zwei Laeufe mit Stilwechsel, ein Lauf ohne Stil davor, ein Lauf mit Leerzeichen im Stil am Ende
    for sgr in ("a\x1b[1mb\x1b[0mc", "\x1b[31m  \x1b[0m", "\x1b[7m \x1b[0mx", "ab\x1b[44m  "):
        ergebnis.append({"sgr": sgr, "laeufe": laeufe(RA.zeile_lesen(sgr))})
    return ergebnis


def main(ziel):
    sch = Schreiber()
    vf, ohne, ungueltig = vertragsfaelle(sch)
    zf = zufallsprobe(sch)
    daten = {
        "samen": SAMEN,
        "referenz_sha256": hashlib.sha256(ref_pfad.read_bytes()).hexdigest(),
        "faelle_ohne_empfangsseite": ohne,
        "faelle_ungueltig": ungueltig,
        "faelle": vf + zf,
        "sgr": sgr_tabelle(),
    }
    roh = (json.dumps(daten, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
    Path(ziel).write_bytes(gzip.compress(roh, 9, mtime=0))
    n = sum(1 for f in daten["faelle"] for e in f["ereignisse"] if e["art"] == "rahmen")
    print(f"{len(vf)} Vertragsfaelle, {len(zf)} Zufallsverbindungen, {n} Rahmen, {len(ohne)} Faelle ohne Empfangsseite, {len(ungueltig)} ungueltig")


if __name__ == "__main__":
    main(sys.argv[1])
