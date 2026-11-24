"""Gemeinsamer Zustand eines Laufs: Sitzungen starten, Zuege (Eingabe an den Agenten) ausfuehren, Einstellungen schreiben.

Ein Zug ist eine Eingabe an den Agenten, etwa `#T4 BEFEHL cat ...`. Der Laeufer wartet, bis der Agent `FERTIG T4`
schreibt, und sammelt dazu: den neuen Terminalverlauf, das Ergebnis des Werkzeugs (aus dem Mitschnitt der
Modell-Attrappe), die neuen Bytes im Mitschnitt der Attrappe und die neuen Bytes zwischen Suche und Agent.
"""
from __future__ import annotations

import glob
import json
import os
import re
import time

from . import konfig, messen


class Zeitueberschreitung(Exception):
    pass


# Feste Fehlertexte, die im letzten Bild einer Sitzung ohne Eingabefeld gezaehlt werden (Diagnose bei Sitzungsende).
ENDE_TEXTE = ("Failed to extract bundled package", "Failed to find the platform package", "ENOSPC", "EROFS", "EACCES", "Permission denied",
              "No such file", "Traceback")
ENDE_CODE = re.compile(r"lauf\.[a-z][a-z0-9_]*(?=:)")


def ende_befund(text):
    """Aus dem Bild einer Sitzung nur Codes und Zahlen: die `lauf.*`-Codes (sortiert, je einmal) und je fester Text die Zahl der Stellen.
    Leerraum zaehlt nicht, damit ein Umbruch des Terminals nichts zerreisst. Nie ein anderer Inhalt des Bildes."""
    flach = re.sub(r"\s+", "", text or "")
    return sorted(set(ENDE_CODE.findall(flach))), {t: flach.count(re.sub(r"\s+", "", t)) for t in ENDE_TEXTE}


def TOKEN(n):
    """Das Ende eines Zugs: `FERTIG T<n>`, nicht `T<n>` mit weiteren Ziffern. Zwischen FERTIG und T<n> darf Leerraum stehen, auch ein
    Zeilenumbruch (steht das Leerzeichen in der letzten Spalte, ist die Zeile dort zu Ende)."""
    return re.compile(r"FERTIG\s+T%d(?!\d)" % n)


class ZaehlDateiFehler(Exception):
    """Eine Zaehldatei enthaelt mehr als Marker und Zahlen. Das ist ein Fehler des Pruefmittels (Code `zaehldatei.inhalt`)."""


def pruefe_zaehldatei(d, sid, art, marker):
    """Eine Zaehldatei traegt nur Marker und Zahlen, nie Text: genau die Felder des Formats, Zaehler als nichtnegative ganze
    Zahlen, Schluessel nur aus der Markerliste, die Sitzung als Kennung. Sonst ZaehlDateiFehler."""
    zahl = "antworten" if art == "suche" else "anfragen"
    if not isinstance(d, dict) or set(d) != {"sitzung", zahl, "marker"}:
        raise ZaehlDateiFehler("Felder der Zaehldatei (%s) weichen vom Format ab" % art)
    if d["sitzung"] != sid:
        raise ZaehlDateiFehler("Zaehldatei gehoert zu einer anderen Sitzung")
    if not isinstance(d[zahl], int) or isinstance(d[zahl], bool) or d[zahl] < 0:
        raise ZaehlDateiFehler("Zaehler der Zaehldatei ist keine Zahl")
    if not isinstance(d["marker"], dict) or (art == "suche" and set(d["marker"]) - set(marker)):  # Liste des Weiterreichers: eigener Befund
        raise ZaehlDateiFehler("Marker-Schluessel der Zaehldatei nicht aus der Markerliste")
    if any(not isinstance(v, int) or isinstance(v, bool) or v < 0 for v in d["marker"].values()):
        raise ZaehlDateiFehler("Marker-Zaehler der Zaehldatei ist keine Zahl")


class Aufzeichnung:
    """Haelt die Bruecke und schreibt mit, was der Laeufer von den Sitzungen gesehen hat: Terminalverlauf (je Sitzung nur der neue
    Teil) und Raster (nur wenn sich das Bild aendert). Daraus entsteht die Ausgabedatei der Suche nach dem Schluessel (Vertrag 8,
    9a, S8). Alles andere (Methoden wie `starten`, `eingabe`, `cursor`) gibt sie unveraendert an die Bruecke weiter."""

    def __init__(self, bruecke, gesehen):
        self._b, self._gesehen = bruecke, gesehen
        self._bis, self._bild = {}, {}

    def __getattr__(self, name):
        return getattr(self._b, name)

    def raster(self, sid):
        r = self._b.raster(sid)
        zeilen = r.get("zeilen") if isinstance(r, dict) else None
        if isinstance(zeilen, list):
            bild = "\n".join(str(z) for z in zeilen)
            if self._bild.get(sid) != bild:
                self._bild[sid] = bild
                self._gesehen.append(bild)
        return r

    def verlauf(self, sid, ab=0):
        laenge, text = self._b.verlauf(sid, ab)
        bis = self._bis.get(sid, 0)
        if isinstance(text, str) and isinstance(laenge, int) and laenge > bis:
            neu = text[max(0, bis - ab):] if ab <= bis else text
            if neu:
                self._gesehen.append(neu)
            self._bis[sid] = laenge
        return laenge, text


class StartLimit(Exception):
    """Ein weiterer Start eines Harness, der Token kostet, ueberschritte die Grenze `probelauf.max_starts`."""


class KeinEingabefeld(Exception):
    """Eine Eingabe an eine Sitzung, deren Eingabefeld beim Start nicht erkannt wurde (Abschluss 21.11.: im echten Start ging eine Eingabe
    an eine CLI, die nie ein Eingabefeld zeigte). Der Laeufer sendet dann nichts. `zustand` und `code` sind der Stand der Sitzung in der
    Sitzungsliste der Bruecke (`laeuft`, `beendet` und ihr Code), soweit lesbar, sonst None."""

    def __init__(self, sid, zustand=None, code=None):
        super().__init__("Sitzung %s ohne erkanntes Eingabefeld (zustand=%s, code=%s)" % (sid, zustand, code))
        self.sid, self.zustand, self.code = sid, zustand, code


class NichtAbgesendet(Exception):
    """Der Text eines Zugs stand nach dem Enter noch im Eingabefeld der CLI (Entscheidung vom 22.11.2026): Die Eingabe ist nicht
    abgesendet. Der Laeufer sendet kein zweites Enter (das waere ein ungemessener Weg); das Bild geht in die Diagnosedatei."""


# Senden eines Zugs (Enter mit Abstand): erst der Text, dann warten, bis sein Ende im Raster steht (hoechstens EINGABE_SICHTBAR_S), dann
# ENTER_ABSTAND_S, dann Enter; danach hoechstens ABGESENDET_S warten, bis das Ende nicht mehr im Eingabefeld (ab der letzten Zeile mit
# PROMPT_ZEICHEN) steht. Gemessen am 22.11.: Text und Enter direkt hintereinander sandte die echte CLI 1.0.88 einmal ab und einmal
# nicht (Messung in zwei Testlaeufen, VM2); mit 300 ms Abstand sandte sie zweimal ab.
EINGABE_SICHTBAR_S = 5.0
ENTER_ABSTAND_S = 0.3
ABGESENDET_S = 5.0
PROMPT_ZEICHEN = "❯"  # wie die CLI und `attrappen/copilot_attrappe.py:eingabefeld`
MERKMAL_ZEICHEN = 16
_RAHMEN = set(chr(c) for c in range(0x2500, 0x2580))


def _flach(text):
    """Text ohne Leerraum und ohne Rahmenzeichen (U+2500 bis U+257F): Umbrueche und Kastenraender der Oberflaeche stoeren den Vergleich nicht."""
    return "".join(c for c in text if not c.isspace() and c not in _RAHMEN)


def merkmal(text):
    """Das Ende des Zugtexts (hoechstens MERKMAL_ZEICHEN Zeichen ohne Leerraum): steht es im Raster, ist der Text ganz getippt."""
    return _flach(text)[-MERKMAL_ZEICHEN:]


def im_eingabefeld(zeilen, merk):
    """Steht `merk` im Eingabefeld? Das Feld beginnt an der letzten Zeile, die mit PROMPT_ZEICHEN anfaengt, und endet an der ersten
    folgenden Zeile nur aus Rahmenzeichen oder Leerraum (Kastenrand) oder am Ende. Ohne Zeile mit PROMPT_ZEICHEN: None (nicht feststellbar)."""
    start = None
    for i, z in enumerate(zeilen):
        if _flach(z).startswith(PROMPT_ZEICHEN):  # auch "│ ❯ ..." in einem Kasten
            start = i
    if start is None:
        return None
    feld = [zeilen[start]]
    for z in zeilen[start + 1:]:
        if not _flach(z):
            break
        feld.append(z)
    return merk in _flach("".join(feld))


class EingabeLimit(Exception):
    """Eine weitere Eingabe an eine Cloud-Sitzung ueberschritte die Grenze `probelauf.cloud_eingaben_max` (jede Eingabe kostet Kontingent)."""


class Zug:
    def __init__(self, n, verlauf, ergebnis, ist_fehler, mit, suche, mw, proto):
        """`suche` und `mw`: Zuwachs der Zaehldateien der Suche und des Weiterreichers waehrend des Zugs ({"antworten"|"anfragen",
        "marker"}) oder None; `proto`: neue Protokollzeilen dieser Sitzung; `mit`: Bytes am Modellweg (nur Attrappen)."""
        self.n, self.verlauf, self.ergebnis, self.ist_fehler, self.mit = n, verlauf, ergebnis, ist_fehler, mit
        self.suche, self.mw, self.proto = suche, mw, proto
        self.sid = None
        self.mw_liste = None  # Schluessel von `marker` in der Zaehldatei des Weiterreichers (None: keine Datei oder kein Eintrag)
        self.start_marker = None  # Befund der Markerliste in start.json: "ok", "abweichend", "unlesbar" oder None (nicht geprueft)
        self.gesperrt = sum(d.get("anzahl", 0) for d in proto if isinstance(d.get("anzahl"), int))
        self.vorversuche = []  # frueherer Zug derselben Suche, der mit {"error": "protokoll"} endete
        # ohne Werkzeugergebnis vom Modellweg (Option) bleibt der Terminalverlauf als Quelle des Rueckgabecodes
        self.rc = messen.rueckgabecode(ergebnis if ergebnis is not None else verlauf)

    def ausgabe(self):
        """Ausgabe des Befehls: das Werkzeugergebnis, sonst (ohne Modellweg) der neue Terminalverlauf."""
        return self.ergebnis if self.ergebnis is not None else self.verlauf

    def protokoll_fehler(self):
        """Wahr, wenn der Agent im Zug die Antwort {"error": "protokoll"} der Suche bekam (nur wo das Werkzeugergebnis sichtbar ist)."""
        return self.ergebnis is not None and re.search(r'"error"\s*:\s*"protokoll"', self.ergebnis) is not None

    def antworten_gesehen(self):
        """Zahl der Antworten der Suche, die der Agent im Zug bekam: 1, wenn das Werkzeugergebnis eine Antwort ist; None, wenn der
        Laeufer das Ergebnis nicht sieht (kein Modellweg)."""
        if self.ergebnis is None:
            return None
        for z in reversed(self.ergebnis.splitlines()):
            try:
                d = json.loads(z)
            except ValueError:
                continue
            if isinstance(d, dict) and ("hits" in d or "error" in d):
                return 1
        return 0

    def antwort_text(self):
        """Antwort der Suche als Text: das Werkzeugergebnis am Modellweg, sonst None (dann gelten Zaehldateien und Terminal)."""
        return self.ergebnis

    def antwort_json(self):
        t = self.antwort_text()
        if t is None:
            return None
        for z in reversed(t.splitlines()):
            try:
                d = json.loads(z)
            except ValueError:
                continue
            if isinstance(d, dict) and ("hits" in d or "error" in d):
                return d
        return None


class Lauf:
    def __init__(self, k, bruecke, mitschnitte, fremd, arbeit):
        """`mitschnitte`: {"cloud": Mitschnitt, "lokal": Mitschnitt}; ein Weg zaehlt nur, wenn `modell.<art>.zaehlen` gilt."""
        self.gesehen = []  # alles, was der Laeufer von den Sitzungen gesehen hat (Terminalverlauf, Raster)
        self.k, self.b, self.fremd, self.arbeit = k, Aufzeichnung(bruecke, self.gesehen), fremd, arbeit
        self.mit = mitschnitte
        self.zaehlen = {a: bool(k["modell"][a].get("zaehlen", True)) for a in ("cloud", "lokal")}
        self.art_von = {}
        self.klasse_hoch = k["klassen"][-1]
        self.starts = 0  # angenommene Starts von Harnesses mit `zaehlt_als_start`
        self.max_starts = k.get("probelauf", {}).get("max_starts")
        # Hoechstens so viele Eingaben an Cloud-Sitzungen im ganzen Lauf (Kosten je Eingabe). Gesetzt: knapper Cloud-Lauf, die Pruefungen
        # mit weiteren Eingaben an die Cloud-Sitzung laufen dann nicht (`<schritt>.nur_attrappe`); None: keine Grenze (Attrappen).
        self.cloud_eingaben_max = k.get("probelauf", {}).get("cloud_eingaben_max")
        self.knapp = self.cloud_eingaben_max is not None
        self.nur_cloud = k.get("umfang") == "nur_cloud"  # Messbetrieb: nur die Cloud-Route (Schalter --nur-cloud)
        self.cloud_eingaben = 0  # angenommene Eingaben an Cloud-Sitzungen
        self.eingaben_je_sitzung = {}
        self.gegenprobe = {"weg": None, "neue_sitzungen": None, "neue_kinder": None}  # Gegenprobe von S1 (Messweg und zwei Zahlen)
        self.buendel = None  # Ergebnis des einen Shell-Buendels der Cloud-Sitzung (Name -> (Zahl, RC)), gesetzt von S1
        self.buendel_zug = None
        self.start_marker = {}  # Sitzung -> Befund der Markerliste in start.json
        self.ziel0 = None  # Zaehler am Ziel der Cloud-Route vor den Eingaben von S1 (knapper Lauf)
        self.gateway_schluessel = None
        self.fehler = []  # eingebaute Fehler der Attrappen (Mutationsprobe)
        self.ca = {"cli_nimmt_an": None, "ausgang": None}  # Vorpruefung der Sitzungs-CA (null = nicht gelaufen)
        self.zinfo = {}  # Attrappen: Marke des Tokens, Zaehler am Ziel, Adressen
        self.turn = 0
        self.sendung = {}  # Ergebnis des letzten `senden`
        self.sitzungen = []
        self.eingabefeld = {}  # Sitzung -> wahr, wenn ihr Eingabefeld beim Start erkannt wurde; ohne Eingabefeld sendet `zug` nie
        self.ende_befund = {}  # Sitzung -> Messwerte der Diagnose bei Sitzungsende (nur Codes und Zahlen), gesetzt, wenn das Eingabefeld ausblieb
        self.sitzung_zustand = {}  # Sitzung -> (zustand, code) aus der Sitzungsliste, gelesen, wenn das Eingabefeld ausblieb
        self.s1 = None
        self.einst_pfad = k["einstellungen"]["pfad"]
        self._einst_vorher = None
        self.cloud, self.lokal = k["routen"]["cloud"], k["routen"]["lokal"]
        self.nk, self.ni, self.ns = k["marker"]["nk"], k["marker"]["ni"], k["marker"]["ns"]
        self.np = k["marker"].get("np")  # erlaubter Marker ohne Kundenbezug: steht im Prompt an den Cloud-Agenten (Nachtrag 20.11., 9)
        self.frage = k["frage"]
        self.turn_s = float(k["grenzen"].get("turn_s", 10))

    # --- Einstellungen --------------------------------------------------------------------------------
    def einstellungen_schreiben(self, variante):
        if not konfig.unter(self.k["testwurzel"], self.einst_pfad):
            messen.diagnose_notiere("lauf.einstellungen", "Einstellungsdatei liegt nicht unter der Testwurzel: " + self.einst_pfad)
            raise konfig.KonfigFehler("Einstellungsdatei liegt nicht unter der Testwurzel")
        if self._einst_vorher is None:
            try:
                with open(self.einst_pfad, "rb") as f:
                    self._einst_vorher = f.read()
            except FileNotFoundError:
                self._einst_vorher = b""
        basis = None
        if self.k["einstellungen"].get("aus_datei") and self._einst_vorher:
            basis = json.loads(self._einst_vorher.decode("utf-8"))
        d = konfig.einstellungen_datei(self.k, variante, basis)
        os.makedirs(os.path.dirname(self.einst_pfad), exist_ok=True)
        with open(self.einst_pfad, "w", encoding="utf-8") as f:
            json.dump(d, f, indent=1)
            f.write("\n")
        return d

    def einstellungen_zurueck(self):
        if self._einst_vorher is None:
            return
        if self._einst_vorher:
            with open(self.einst_pfad, "wb") as f:
                f.write(self._einst_vorher)
        elif os.path.exists(self.einst_pfad):
            os.unlink(self.einst_pfad)
        self._einst_vorher = None

    # --- Protokoll und Suchmitschnitt -------------------------------------------------------------------
    def protokoll_dateien(self):
        return sorted(glob.glob(os.path.join(self.k["protokoll"]["ordner"], "brain-*.jsonl")))

    def protokoll_zeilen(self):
        erg = []
        for d in self.protokoll_dateien():
            erg += messen.protokollzeilen(d)
        return erg

    def zaehlstand(self, sid, art):
        """Zaehldatei `<zaehlordner>/<sitzung>.suche.json` oder `.modellweg.json`. Gibt None, wenn kein Ordner konfiguriert ist;
        eine fehlende Datei heisst: noch nichts gezaehlt."""
        w = self.k.get("zaehlordner")
        if not w:
            return None
        p = os.path.join(w, "%s.%s.json" % (sid, art))
        try:
            with open(p, "rb") as f:
                roh = f.read(4097)
            if len(roh) > 4096:
                raise ZaehlDateiFehler("Zaehldatei ist laenger als 4096 Byte")
            d = json.loads(roh.decode("utf-8"))
        except FileNotFoundError:
            return {"antworten": 0, "anfragen": 0, "marker": {}}
        except (OSError, ValueError):
            raise ZaehlDateiFehler("Zaehldatei nicht lesbar oder kein JSON")
        pruefe_zaehldatei(d, sid, art, list(self.k["marker"].values()))
        return d

    @staticmethod
    def zuwachs(vorher, nachher):
        if vorher is None or nachher is None:
            return None
        n = {m: nachher.get("marker", {}).get(m, 0) - vorher.get("marker", {}).get(m, 0) for m in nachher.get("marker", {})}
        return {"antworten": nachher.get("antworten", nachher.get("anfragen", 0)) - vorher.get("antworten", vorher.get("anfragen", 0)),
                "marker": n}

    # --- Sitzungen ------------------------------------------------------------------------------------
    def starten(self, start, ohne_token=False, ablehnung=False):
        """`POST /starten` mit Zaehlung. Ein Start eines zaehlenden Harness ueber der Grenze wird nicht gesendet, die Bruecke sieht ihn
        nicht. Ausnahme `ablehnung`: ein Start, den die Bruecke vor jedem Kind ablehnen muss (ohne Token, ohne Route, unbekannte Route,
        S1); er startet nichts, kostet nichts und geht deshalb auch an der Grenze hinaus. Nimmt die Bruecke ihn doch an, zaehlt er."""
        zaehlt = bool(self.k["harness"].get(start.get("harness"), {}).get("zaehlt_als_start"))
        if zaehlt and not ablehnung and self.max_starts is not None and self.starts >= self.max_starts:
            raise StartLimit("copilot.start-limit")
        code, antw, t0 = self.b.starten(start, ohne_token=ohne_token)
        if zaehlt and code in (200, 201):
            self.starts += 1
        return code, antw, t0

    def start_dict(self, art):
        v = dict(self.k["start"][art])
        v["route"] = self.k["routen"][art]
        return dict({"fassung": 1}, **v)

    def sitzung_starten(self, art, frist_s=None):
        start = self.start_dict(art)
        h = self.k["harness"][start["harness"]]
        m = messen.start_bis_eingabefeld(self, start, h["eingabefeld_muster"], frist_s or float(self.k["bruecke"].get("frist_s", 10)),
                                         vertrauen=h.get("vertrauensfrage_muster"), ohne_anmeldung=h.get("ohne_anmeldung_muster"))
        if m["sitzung"]:
            self.sitzungen.append(m["sitzung"])
            self.art_von[m["sitzung"]] = art
            self.eingabefeld[m["sitzung"]] = m["t1"] is not None
            if m["t1"] is None:
                try:
                    info = self.b.sitzung(m["sitzung"])
                except Exception:
                    info = None
                if isinstance(info, dict):
                    m["zustand"], m["code"] = info.get("zustand"), info.get("code")
                self.sitzung_zustand[m["sitzung"]] = (m.get("zustand"), m.get("code"))
                self.ende_diagnose(m["sitzung"])
        return m

    def ende_diagnose(self, sid):
        """Diagnose bei Sitzungsende: Zustand und Code der Sitzung, die `lauf.*`-Codes aus dem letzten Bild und die Zaehler fester Fehlertexte.
        Nur Codes und Zahlen (`ende_zustand`, `ende_code`, `ende_lauf_codes`, `ende_texte`); sie stehen im Bericht, nie in der Ergebnisdatei."""
        try:
            r = self.b.raster(sid)
            text = "\n".join(str(z) for z in r.get("zeilen", [])) if isinstance(r, dict) else ""
            if not text.strip():
                text = self.b.verlauf(sid, 0)[1]
        except Exception:
            text = ""
        codes, zaehler = ende_befund(text)
        zustand, code = self.sitzung_zustand.get(sid, (None, None))
        self.ende_befund[sid] = {"ende_zustand": zustand, "ende_code": code, "ende_lauf_codes": codes, "ende_texte": zaehler}
        return self.ende_befund[sid]

    def beenden(self, sid):
        try:
            self.b.beenden(sid)
        except Exception:
            pass

    def alle_beenden(self):
        for sid in list(self.sitzungen):
            self.beenden(sid)

    # --- Zuege ---------------------------------------------------------------------------------------
    def _eingabefeld_jetzt(self, sid):
        """Kam das Eingabefeld nach der Frist des Starts doch noch (ein spaetes Feld)? Liest das aktuelle Raster einmal und prueft das
        Merkmal des Harness dieser Sitzung. Wahr merkt sich die Sitzung; sonst liest es den Zustand der Sitzung nach."""
        try:
            r = self.b.raster(sid)
            muster = self.k["harness"][self.start_dict(self.art_von.get(sid, "cloud"))["harness"]]["eingabefeld_muster"]
            ok = isinstance(r, dict) and messen.ist_eingabefeld(r.get("zeilen", []), muster)
        except Exception:
            ok = False
        if ok:
            self.eingabefeld[sid] = True
            return True
        try:
            info = self.b.sitzung(sid)
            if isinstance(info, dict):
                self.sitzung_zustand[sid] = (info.get("zustand"), info.get("code"))
        except Exception:
            pass
        return False

    def diagnose_bild(self, sid):
        """Bei einer Zeitueberschreitung (und bei `KeinEingabefeld` einer lebenden Sitzung): das ganze aktuelle Raster mit allen Zeilen, den
        Cursor und die letzten 4000 Zeichen des Verlaufs, nur in die Diagnosedatei (`messen.diagnose_notiere`), nie in Bericht oder
        Konsole. Scheitert ein Lesen, steht dort nur der Name der Ausnahme; der Schritt endet trotzdem mit seinem festen Code."""
        try:
            r = self.b.raster(sid)
            zeilen = r.get("zeilen", []) if isinstance(r, dict) else []
            for i, z in enumerate(zeilen):
                messen.diagnose_notiere("lauf.raster", "zeile %d: %s" % (i, z))
            if isinstance(r, dict) and r.get("cursor") is not None:
                messen.diagnose_notiere("lauf.raster", "cursor: %s" % (r.get("cursor"),))
        except Exception as e:
            messen.diagnose_notiere("lauf.raster", "nicht lesbar: %s" % type(e).__name__)
        try:
            messen.diagnose_notiere("lauf.verlauf", "letzte 4000 Zeichen: %r" % (self.b.verlauf(sid, 0)[1][-4000:],))
        except Exception as e:
            messen.diagnose_notiere("lauf.verlauf", "nicht lesbar: %s" % type(e).__name__)

    def _zeilen(self, sid):
        try:
            r = self.b.raster(sid)
        except Exception:
            return []
        return r.get("zeilen", []) if isinstance(r, dict) else []

    def senden(self, sid, text, marke=None):
        """Sendet einen Zugtext (Entscheidung vom 22.11.2026). Mit Bruecke (`text_und_taste`): erst das Ereignis `Text`, dann warten,
        bis sein Ende im Raster steht (hoechstens EINGABE_SICHTBAR_S), dann ENTER_ABSTAND_S, dann `Taste` Enter, dann hoechstens
        ABGESENDET_S warten, bis es nicht mehr im Eingabefeld steht (oder kein Feld im Bild ist). Steht es nie im Raster oder lehnt der Dienst ab (seit `marke`), geht
        KEIN Enter hinaus. Ohne `text_und_taste` (Seite im Browser, Test-Bruecken) oder ohne Text unveraendert `eingabe`. Das Ergebnis steht
        in `self.sendung` (sichtbar, abgesendet: True, False oder None fuer nicht feststellbar). Gibt (code, antwort) wie `eingabe`."""
        b = self.b
        if not text or not getattr(b, "text_und_taste", False):
            self.sendung = {"weg": "eingabe", "sichtbar": None, "abgesendet": None}
            return b.eingabe(sid, text)
        merk = merkmal(text)
        self.sendung = {"weg": "abstand", "sichtbar": False, "abgesendet": None}
        code, antw = b.text(sid, text)
        if code not in (200, 202):
            return code, antw
        ende = time.monotonic() + EINGABE_SICHTBAR_S
        while time.monotonic() < ende:
            if merk in _flach("".join(self._zeilen(sid))):
                self.sendung["sichtbar"] = True
                break
            if marke is not None and b.ablehnungen_seit(marke):
                break  # `zug` meldet die Ablehnung
            time.sleep(0.05)
        if not self.sendung["sichtbar"]:
            return 202, {"ok": True}  # kein Enter; `zug` endet mit NichtAbgesendet
        time.sleep(ENTER_ABSTAND_S)
        code, antw = b.taste(sid, "Enter")
        if code not in (200, 202):
            return code, antw
        ende = time.monotonic() + ABGESENDET_S
        while time.monotonic() < ende:
            feld = im_eingabefeld(self._zeilen(sid), merk)
            self.sendung["abgesendet"] = None if feld is None else not feld
            if feld is not True:
                break  # leer oder kein Feld mehr im Bild: nicht weiter warten
            time.sleep(0.1)
        return code, antw  # abgesendet None (kein Feld mit PROMPT_ZEICHEN im Bild): kein Halt, keine Diagnosezeile

    def zug(self, sid, art, arg, vorlage=None, frist_s=None):
        if self.eingabefeld.get(sid) is False and not self._eingabefeld_jetzt(sid):
            # vor allem anderen: ohne erkanntes Eingabefeld geht keine Eingabe raus (auch kein spaetes Feld, das jetzt fehlt)
            if self.sitzung_zustand.get(sid, (None, None))[0] != "beendet":
                self.diagnose_bild(sid)  # die Sitzung lebt noch: das Bild sagt, was die CLI zeigt
            raise KeinEingabefeld(sid, *self.sitzung_zustand.get(sid, (None, None)))
        n = self.turn = self.turn + 1
        ab, _ = self.b.verlauf(sid, 10 ** 9)
        cloud = self.art_von.get(sid, "cloud") == "cloud"
        if cloud and self.cloud_eingaben_max is not None and self.cloud_eingaben >= self.cloud_eingaben_max:
            raise EingabeLimit("cloud.eingaben-limit")  # vor der Bruecke: die Eingabe wird nicht gesendet
        weg = self.mit[self.art_von.get(sid, "cloud")] if self.zaehlen[self.art_von.get(sid, "cloud")] else None
        mit0 = weg.laenge() if weg else 0
        s0, w0 = self.zaehlstand(sid, "suche"), self.zaehlstand(sid, "modellweg")
        p0 = len(self.protokoll_zeilen())
        text = (vorlage or self.k["prompts"][art]).format(n=n, arg=arg, np=self.np or "")
        marke = self.b.ablehnungen_marke()
        if sid not in self.start_marker:
            self.start_marker[sid] = self.pruefe_start_marker(sid)  # solange die Sitzung laeuft und start.json da ist
        code, antw = self.senden(sid, text, marke)
        if code not in (200, 202):
            messen.diagnose_notiere("lauf.zug", "Eingabe abgelehnt: %s %s" % (code, antw))  # die Antwort der Bruecke nur in die Diagnosedatei
            self.diagnose_bild(sid)
            raise Zeitueberschreitung("Eingabe abgelehnt")
        if cloud and self.sendung.get("sichtbar") is not False:  # das Enter ging hinaus: es zaehlt, auch wenn die CLI nicht absendet
            self.cloud_eingaben += 1
            self.eingaben_je_sitzung[sid] = self.eingaben_je_sitzung.get(sid, 0) + 1
        if self.sendung.get("sichtbar") is False or self.sendung.get("abgesendet") is False:
            abgelehnt = self.b.ablehnungen_seit(marke)
            if abgelehnt:  # der Dienst lehnte den Text ab: wie im Warten unten
                messen.diagnose_notiere("lauf.zug", "Eingabe vom Dienst abgelehnt: %s" % ", ".join(g or c or "?" for g, c in abgelehnt))
                self.diagnose_bild(sid)
                raise Zeitueberschreitung("Eingabe vom Dienst abgelehnt")
            # Kein zweites Enter (Entscheidung vom 22.11.2026): Halt dieses Zugs mit festem Code, das Bild nur in die Diagnosedatei
            messen.diagnose_notiere("lauf.zug", "Zug %d nicht abgesendet: sichtbar=%s abgesendet=%s" % (
                n, self.sendung.get("sichtbar"), self.sendung.get("abgesendet")))
            self.diagnose_bild(sid)
            raise NichtAbgesendet("Zug %d nicht abgesendet" % n)
        # Leerraum zwischen FERTIG und T<n> darf ein Zeilenende sein: bricht das Terminal dort um, steht im Verlauf ein Zeilenumbruch
        token = TOKEN(n)
        erwartet = 1 + len(token.findall(text))
        ende = time.monotonic() + (frist_s or self.turn_s)
        while True:
            laenge, t = self.b.verlauf(sid, ab)
            if len(token.findall(t)) >= erwartet:
                break
            abgelehnt = self.b.ablehnungen_seit(marke)
            if abgelehnt:
                messen.diagnose_notiere("lauf.zug", "Eingabe vom Dienst abgelehnt: %s" % ", ".join(g or c or "?" for g, c in abgelehnt))
                self.diagnose_bild(sid)
                raise Zeitueberschreitung("Eingabe vom Dienst abgelehnt")
            if time.monotonic() > ende:
                # Bild und Verlauf sind Inhalt der Sitzung: nur in die Diagnosedatei, nie in die Meldung der Ausnahme
                messen.diagnose_notiere("lauf.zug", "Zug %d: kein FERTIG T%d" % (n, n))
                self.diagnose_bild(sid)
                raise Zeitueberschreitung("Zug %d: kein FERTIG T%d nach %.0f s" % (n, n, frist_s or self.turn_s))
            time.sleep(0.03)
        e = weg.tool_ergebnis(n) if weg else None
        s1, w1 = self.zaehlstand(sid, "suche"), self.zaehlstand(sid, "modellweg")
        proto = [d for _, d in self.protokoll_zeilen()[p0:] if isinstance(d, dict) and d.get("sitzung") == sid]
        zg = Zug(n, t, e[0] if e else None, bool(e and e[1]), weg.ab(mit0) if weg else None,
                 self.zuwachs(s0, s1), self.zuwachs(w0, w1), proto)
        zg.sid = sid
        zg.mw_liste = set(w1["marker"]) if w1 and w1.get("marker") else None
        zg.start_marker = self.start_marker.get(sid)
        return zg

    def pruefe_start_marker(self, sid):
        """Markerliste in der Startdatei der Sitzung (`start.json`, Feld `marker` mit `liste` und `ordner`, so wie der Kern sie zusammengesetzt
        hat): `liste` gleich der Soll-Liste, `ordner` gleich dem Zaehlordner. None, wenn die Konfiguration keine Startdatei nennt; sonst
        `ok`, `abweichend` oder `unlesbar`. Der Ort kommt nur aus der Konfiguration (`marker_start_datei`, `{sitzung}`)."""
        vorlage = self.k.get("marker_start_datei")
        if not vorlage or not re.match(r"^[A-Za-z0-9._-]{1,64}$", sid or "") or sid in (".", ".."):
            return None
        try:
            with open(vorlage.replace("{sitzung}", sid), "rb") as f:
                roh = f.read(65537)
            m = json.loads(roh.decode("utf-8"))["marker"]
            liste, ordner = m["liste"], m["ordner"]
        except (OSError, ValueError, KeyError, TypeError):
            return "unlesbar"
        soll = sorted(self.k["marker"].values())
        if not isinstance(liste, list) or sorted(map(str, liste)) != soll or not isinstance(ordner, str):
            return "abweichend"
        zo = self.k.get("zaehlordner")
        return "ok" if zo and os.path.realpath(ordner) == os.path.realpath(zo) else "abweichend"

    def befehl(self, sid, cmd, frist_s=None):
        return self.zug(sid, "befehl", cmd, frist_s=frist_s)

    def suche(self, sid, art="suche"):
        """Die Suche fragen. Scheitert ihre Protokollzeile, bekommt der Agent {"error": "protokoll"}; die Zaehldatei hat diese
        Antwort trotzdem gezaehlt (Vertrag 4, Klarstellung 5). Der Laeufer fragt dann einmal erneut und merkt den fruehen Zug in
        `vorversuche`: Die Positivpruefung aus der Zaehldatei ist fuer ihn nicht vergleichbar (offen)."""
        z = self.zug(sid, art, self.frage)
        if z.protokoll_fehler():
            z2 = self.zug(sid, art, self.frage)
            z2.vorversuche = [z] + z.vorversuche
            return z2
        return z
