"""Reference of the state core contract (the contract text). Standard library only.

  zk-ref [fall.json]      read one case (file or stdin), write the output JSON to stdout
  zk-ref --gegenproben    list the mutant names, one per line

REF_GEGENPROBE=<name> switches on exactly one mutant from the catalogue of the contract text.
No system time, no randomness: the only clock is the field `t` of each event.
"""
from __future__ import annotations

import base64
import json
import os
import re
import sys

MUTANTEN = [
    "umschalt_enter_cr",
    "paste_ohne_klammern",
    "paste_endmarke_durch",
    "paste_lf_bleibt",
    "paste_cr_ohne_klammern",
    "mod_pfeil_fehlt",
    "osc52_grenze_exklusiv",
    "osc52_kuerzen",
    "osc52_inhalt_an_alle",
    "osc52_aus_hinweis",
    "osc52_lesen_antwort",
    "groesse_jede_meldung",
    "groesse_folge_ausfuehren",
    "abzug_nur_erstes_abo",
    "aenderung_ohne_abo",
    "buendel_ohne_frist",
    "sync_ignorieren",
    "sync_ohne_zeitgeber",
    "dialog_sperrt_nicht",
    "dialog_sperrt_antwort",
    "sammelfreigabe",
    "agent_freigabe",
    "nachweis_mehrfach",
    "route_gesperrt_unsichtbar",
    "route_gesperrt_waehlbar",
    "raum_ohne_leeren",
    "raum_abo_bleibt",
    "agent_unbekannt_endstand",
    "agent_alter_neu",
    "absender_aus_text",
    "absender_vorgabe_mensch",
    "nutzer_aendert_regel",
    "nutzer_lockert",
    "invers_ignorieren",
    "invers_nicht_im_abzug",
    "bildrate_ignorieren",
    "bildrate_abrunden",
    "rueckstau_ignorieren",
    "rueckstau_grenze_exklusiv",
    "rueckstau_kein_nachholen",
    "rueckstau_nur_letzte_zeilen",
    "rueckstau_warteschlange",
    "rueckstau_bremst_alle",
    "rueckstau_nicht_gezaehlt",
    "rueckstau_grenze_global",
    "rueckstau_vorgabe_halb",
    "rueckstau_abzug_gesperrt",
    "rueckstau_nachholen_zaehlt_versand",
    "rueckstau_nachholen_ohne_abstand",
    "cursor_ignorieren",
    "cursor_nicht_im_abzug",
    "cursor_nicht_in_aenderung",
    "cursor_nur_bei_zeilen",
    "cursor_aelterer_gilt",
    "cursor_sync_ignorieren",
    "cursor_sync_freigabe_ohne_versand",
    "cursor_groesse_nicht_klemmen",
    "cursor_groesse_vergessen",
    "cursor_ungueltig_angenommen",
    "cursor_ungueltig_ignoriert",
    "cursor_sichtbar_nicht_geprueft",
    "cursor_nachholen_fehlt",
    "cursor_anfang_ignoriert",
    "cursor_zustand_fehlt",
    "cursor_vorgabe_null",
    "cursor_umgeht_deckel",
]

BUENDEL_MS = 16
# Vorgabe der Obergrenze der Bildrate (Bilder je s): 63 ist die kleinste ganze Zahl, bei der der Abstand 16 ms bleibt (1000 / 16 = 62,5)
BILDRATE_VORGABE = 63
# Vorgabe der Rueckstaugrenze je Betrachter in Bytes: der kleinste Wert, der in der Messung des Laeufers alle Abnahmepunkte haelt (Vertragstext, Abschnitt 7)
RUECKSTAU_VORGABE = 16384
SYNC_MS = 150
PASTE_AN = b"\x1b[200~"
PASTE_AUS = b"\x1b[201~"
MAX_GROESSE = 1000

TASTEN = {
    ("Enter", ()): b"\r",
    ("Enter", ("umschalt",)): b"\x1b\r",
    ("Tab", ()): b"\t",
    ("Tab", ("umschalt",)): b"\x1b[Z",
    ("Escape", ()): b"\x1b",
    ("Ruecktaste", ()): b"\x7f",
    ("Ruecktaste", ("alt",)): b"\x1b\x7f",
    ("Entf", ()): b"\x1b[3~",
    ("Hoch", ()): b"\x1b[A",
    ("Runter", ()): b"\x1b[B",
    ("Rechts", ()): b"\x1b[C",
    ("Links", ()): b"\x1b[D",
    ("Links", ("strg",)): b"\x1b[1;5D",
    ("Rechts", ("strg",)): b"\x1b[1;5C",
    ("Links", ("alt",)): b"\x1b[1;3D",
    ("Rechts", ("alt",)): b"\x1b[1;3C",
    ("Pos1", ()): b"\x1b[H",
    ("Ende", ()): b"\x1b[F",
    ("BildAuf", ()): b"\x1b[5~",
    ("BildAb", ()): b"\x1b[6~",
}
UMSCHALTER = {"umschalt", "strg", "alt"}

VORGABE_REGELN = {
    "osc52_schreiben": {"art": "stufen", "stufen": ["an", "aus"], "vorgabe": "an", "nutzer_darf": True},
    "osc52_grenze": {"art": "zahl", "vorgabe": 100000, "nutzer_darf": True},
    "bildrate_max": {"art": "zahl", "vorgabe": BILDRATE_VORGABE, "nutzer_darf": True, "min": 1},
    "rueckstau_max": {"art": "zahl", "vorgabe": RUECKSTAU_VORGABE, "nutzer_darf": True, "min": 1},
}


class Ungueltig(Exception):
    """The case itself is malformed: exit code 2."""


class Abgelehnt(Exception):
    def __init__(self, grund: str):
        super().__init__(grund)
        self.grund = grund


def ist_int(x) -> bool:
    return isinstance(x, int) and not isinstance(x, bool)


def cursor_gueltig(c, spalten: int, zeilen: int, mut: str = "") -> bool:
    """R15: [zeile, spalte, sichtbar] mit ganzen Zahlen innerhalb der Groesse und einem Wahrheitswert."""
    if not isinstance(c, list) or len(c) != 3 or not ist_int(c[0]) or not ist_int(c[1]):
        return False
    if not isinstance(c[2], bool) and mut != "cursor_sichtbar_nicht_geprueft":
        return False
    return mut == "cursor_ungueltig_angenommen" or (0 <= c[0] < zeilen and 0 <= c[1] < spalten)


class Kern:
    def __init__(self, anfang: dict, mut: str):
        self.m = mut
        self.t = 0
        self.idx = -1
        self.timer: list[tuple[int, int, str, str]] = []  # (zeit, folge, art, schluessel)
        self.folge = 0
        self.aus = {"nachrichten": {}, "pty": [], "groessen": [], "ablehnungen": [], "protokoll": [],
                    "an_stelle": [], "abfragen": []}
        self.personen = {}
        for pid, p in (anfang.get("personen") or {}).items():
            self.personen[pid] = {"rolle": p.get("rolle", "nutzer"), "art": p.get("art", "mensch")}
        self.nachweise = {}
        for nid, n in (anfang.get("nachweise") or {}).items():
            self.nachweise[nid] = {"person": n.get("person"), "benutzt": False}
        self.routen = [dict(name=r["name"], status=r.get("status", "frei"), grund=r.get("grund"),
                            zone=r.get("zone")) for r in (anfang.get("routen") or [])]
        self.regeln = {}
        eigene = anfang.get("regeln") or {}
        for k in list(VORGABE_REGELN) + [k for k in eigene if k not in VORGABE_REGELN]:
            d = dict(VORGABE_REGELN.get(k, {}))
            d.update(eigene.get(k, {}))
            self.regeln[k] = {"art": d.get("art", "stufen"), "stufen": d.get("stufen"), "min": d.get("min", 0),
                              "vorgabe": d.get("vorgabe"), "nutzer_darf": bool(d.get("nutzer_darf", False)),
                              "firma": d.get("firma"), "gruppe": d.get("gruppe"),
                              "nutzer": dict(d.get("nutzer") or {})}
        self.sitzungen = {}
        self.reihenfolge = []
        for s in anfang.get("sitzungen") or []:
            sp, ze = s.get("groesse", [80, 24])
            zeilen = list(s.get("zeilen") or [])
            zeilen = (zeilen + [""] * ze)[:ze]
            cursor = None
            if "cursor" in s and mut != "cursor_anfang_ignoriert":
                if not cursor_gueltig(s["cursor"], sp, ze):
                    raise ValueError("cursor ungueltig")
                cursor = list(s["cursor"])
            self.sitzungen[s["id"]] = {
                "id": s["id"], "titel": s.get("titel", ""), "route": s.get("route"),
                "kundenraum": s.get("kundenraum"), "person": s.get("person"),
                "groesse": [sp, ze], "zustand": "laeuft", "code": None,
                "bracketed_paste": bool(s.get("bracketed_paste", False)),
                "zeilen": zeilen, "stand": s.get("stand", 0),
                "schmutzig": set(), "letzter_versand": None, "faellig": None,
                "sync": False, "sync_zeilen": {}, "sync_stand": None,
                "invers": bool(s.get("invers", False)), "sync_invers": None,
                "cursor": cursor, "sync_cursor": None,
                "letzte_eingabe": None, "dialog": None,
            }
            self.reihenfolge.append(s["id"])
        self.worker = {}
        for w in anfang.get("worker") or []:
            self.worker[w["id"]] = {"sitzung": w.get("sitzung"), "stand": None}
        self.anfragen = {}
        self.anfrage_folge = []
        self.betrachter = {}       # verbunden
        self.erste_abos = {}       # (betrachter, sitzung) -> stand; only for the mutant abzug_nur_erstes_abo

    # ---------- output helpers ----------
    def an(self, bid: str, nachricht: dict):
        n = {"t": self.t}
        n.update(nachricht)
        self.aus["nachrichten"].setdefault(bid, []).append(n)

    def ablehnen(self, grund: str):
        self.aus["ablehnungen"].append({"t": self.t, "ereignis": self.idx, "grund": grund})

    def pty(self, sid: str, b: bytes):
        self.aus["pty"].append({"t": self.t, "sitzung": sid, "hex": b.hex()})

    def im_raum(self, raum):
        return [bid for bid, b in self.betrachter.items() if b["raum"] == raum]

    def abonnenten(self, sid):
        return [bid for bid, b in self.betrachter.items() if sid in b["abos"]]

    # ---------- settings ----------
    def strenger(self, k, a, b) -> bool:
        r = self.regeln[k]
        if r["art"] == "zahl":
            return a < b
        st = r["stufen"]
        return st.index(a) > st.index(b)

    def basis(self, k):
        r = self.regeln[k]
        if r["gruppe"] is not None:
            return r["gruppe"], "gruppe"
        if r["firma"] is not None:
            return r["firma"], "firma"
        return r["vorgabe"], "vorgabe"

    def wirksam(self, k, person):
        r = self.regeln[k]
        w, h = self.basis(k)
        if person in r["nutzer"]:
            v = r["nutzer"][person]
            if self.m == "nutzer_lockert" or self.strenger(k, v, w):
                return v, "nutzer"
        return w, h

    def einstellungen_fuer(self, person):
        return {k: {"wert": self.wirksam(k, person)[0], "vorgabe": self.regeln[k]["vorgabe"],
                    "herkunft": self.wirksam(k, person)[1]} for k in sorted(self.regeln)}

    # ---------- session lists ----------
    def sitzungsliste(self, raum):
        aus = []
        for sid in self.reihenfolge:
            s = self.sitzungen[sid]
            if s["kundenraum"] != raum:
                continue
            aus.append({"id": sid, "titel": s["titel"], "route": s["route"], "groesse": list(s["groesse"]),
                        "zustand": s["zustand"], "code": s["code"]})
        return aus

    def sitzungsliste_senden(self, raum):
        for bid in self.im_raum(raum):
            self.an(bid, {"art": "Sitzungsliste", "sitzungen": self.sitzungsliste(raum)})

    def routenliste(self):
        aus = []
        for r in self.routen:
            if self.m == "route_gesperrt_unsichtbar" and r["status"] != "frei":
                continue
            waehlbar = r["status"] == "frei" or self.m == "route_gesperrt_waehlbar"
            aus.append({"name": r["name"], "status": r["status"], "grund": r["grund"], "zone": r["zone"],
                        "waehlbar": waehlbar})
        return aus

    def anfrage_bild(self, a):
        return {"id": a["id"], "sitzung": a["sitzung"], "werkzeug": a["werkzeug"], "vorschau": a["vorschau"],
                "frist": a["frist"], "zustand": a["zustand"]}

    def offene_anfragen(self, raum):
        return [self.anfrage_bild(self.anfragen[i]) for i in self.anfrage_folge
                if self.anfragen[i]["zustand"] == "offen"
                and self.sitzungen.get(self.anfragen[i]["sitzung"], {}).get("kundenraum") == raum]

    def agenten(self, raum):
        aus = []
        for wid in sorted(self.worker):
            w = self.worker[wid]
            if w["stand"] is None:
                continue
            if self.sitzungen.get(w["sitzung"], {}).get("kundenraum") != raum:
                continue
            aus.append(dict(worker=wid, **w["stand"]))
        return aus

    def raumbild(self, bid):
        raum = self.betrachter[bid]["raum"]
        self.an(bid, {"art": "Sitzungsliste", "sitzungen": self.sitzungsliste(raum)})
        self.an(bid, {"art": "Anfragen", "anfragen": self.offene_anfragen(raum)})
        self.an(bid, {"art": "Agenten", "agenten": self.agenten(raum)})

    # ---------- grid ----------
    def rueckstand_loeschen(self, bid, sid):
        b = self.betrachter[bid]
        for k in ("verpasst", "letzter", "nachgeholt", "warteschlange"):
            b[k].pop(sid, None)
        self.timer_loeschen("nachholen", (bid, sid))

    def abzug(self, bid, sid):
        s = self.sitzungen[sid]
        # Ein Abzug traegt den ganzen Stand: er gilt nie als verfallen und loescht den Rueckstand dieser Sitzung
        self.rueckstand_loeschen(bid, sid)
        if self.m == "rueckstau_abzug_gesperrt" and self.gesperrt(bid, sid):
            return
        invers = s["invers"] and self.m != "invers_nicht_im_abzug"
        nachricht = {"art": "Abzug", "sitzung": sid, "stand": s["stand"], "groesse": list(s["groesse"]),
                     "invers": invers, "zeilen": list(s["zeilen"])}
        if self.m != "cursor_nicht_im_abzug":
            self.cursor_feld(s, nachricht)
        self.an(bid, nachricht)
        self.betrachter[bid]["stand"][sid] = s["stand"]

    def cursor_feld(self, s, nachricht):
        """R15: Abzug und Rasteraenderung tragen den Cursor der Sitzung, sobald der Kern einen kennt; sonst fehlt das Feld."""
        c = s["cursor"]
        if c is None and self.m == "cursor_vorgabe_null":
            c = [0, 0, False]
        if c is not None:
            nachricht["cursor"] = list(c)

    def cursor_klemmen(self, c, groesse):
        """R15: Nach einer Groessenaenderung liegt der Cursor innerhalb der neuen Groesse (Spalte und Zeile je auf den letzten Index)."""
        if self.m == "cursor_groesse_nicht_klemmen":
            return list(c)
        return [min(c[0], groesse[1] - 1), min(c[1], groesse[0] - 1), c[2]]

    def timer_setzen(self, zeit, art, schluessel):
        self.folge += 1
        self.timer.append((zeit, self.folge, art, schluessel))

    def timer_loeschen(self, art, schluessel):
        self.timer = [x for x in self.timer if not (x[2] == art and x[3] == schluessel)]

    def abstand(self, s):
        """Mindestabstand zweier Rasteraenderungen in ganzen ms: die Buendelung oder, wenn laenger, der aufgerundete Kehrwert
        der wirksamen Obergrenze der Bildrate des Sitzungsbesitzers."""
        if self.m == "bildrate_ignorieren":
            return BUENDEL_MS
        rate = max(1, self.wirksam("bildrate_max", s["person"])[0])
        rund = 1000 // rate if self.m == "bildrate_abrunden" else -(-1000 // rate)
        return max(BUENDEL_MS, rund)

    def schmutzig_machen(self, sid, zeilen):
        s = self.sitzungen[sid]
        s["schmutzig"].update(zeilen)
        if self.m == "buendel_ohne_frist":
            self.versenden(sid)
            return
        if s["faellig"] is not None:
            return
        ab = self.abstand(s)
        if s["letzter_versand"] is None or self.t - s["letzter_versand"] >= ab:
            self.versenden(sid)
        else:
            s["faellig"] = s["letzter_versand"] + ab
            self.timer_setzen(s["faellig"], "versand", sid)

    def grenze(self, bid, sid):
        """Wirksame Rueckstaugrenze in Bytes: je Betrachter, also nach dessen Person (R14)."""
        person = self.betrachter[bid]["person"]
        if self.m == "rueckstau_grenze_global":
            person = self.sitzungen[sid]["person"]
        wert, herkunft = self.wirksam("rueckstau_max", person)
        if self.m == "rueckstau_vorgabe_halb" and herkunft == "vorgabe":
            wert //= 2
        return wert

    def gesperrt(self, bid, sid):
        """Liegen zu viele nicht abgenommene Bytes am Socket dieses Betrachters? Genau die Grenze geht noch."""
        if self.m == "rueckstau_ignorieren":
            return False
        a, g = self.betrachter[bid]["ausstehend"], self.grenze(bid, sid)
        return a >= g if self.m == "rueckstau_grenze_exklusiv" else a > g

    def versenden(self, sid):
        s = self.sitzungen[sid]
        s["faellig"] = None
        zeilen_neu = {i for i in s["schmutzig"] if i < len(s["zeilen"])}
        if self.m == "aenderung_ohne_abo":
            ziele = [b for b in self.im_raum(s["kundenraum"])]
        else:
            ziele = self.abonnenten(sid)
        if self.m == "rueckstau_bremst_alle" and any(
                self.gesperrt(bid, sid) and self.betrachter[bid]["stand"].get(sid, -1) < s["stand"] for bid in ziele):
            return  # der Kern wartet auf den langsamsten Betrachter: die Zeilen bleiben schmutzig, niemand bekommt etwas
        s["letzter_versand"] = self.t
        s["schmutzig"] = set()
        for bid in ziele:
            b = self.betrachter[bid]
            von = b["stand"].get(sid)
            if von is not None and von >= s["stand"]:
                continue
            b["verpasst"].setdefault(sid, set()).update(zeilen_neu)
            self.beliefern(bid, sid, True)

    def beliefern(self, bid, sid, durch_versand):
        """Bringt einen Betrachter auf den Stand der Sitzung. Der Versand der Sitzung (durch_versand) kann an der Rueckstaugrenze
        verfallen; das Nachholen (Socket-Meldung unter der Grenze oder Zeitgeber) holt den ganzen Endzustand nach (R14)."""
        s, b = self.sitzungen[sid], self.betrachter[bid]
        von = b["stand"].get(sid)
        if von is not None and von >= s["stand"]:
            b["verpasst"].pop(sid, None)
            return
        if self.gesperrt(bid, sid):
            if durch_versand:
                if self.m != "rueckstau_nicht_gezaehlt":
                    b["verfallen"] += 1
                if self.m == "rueckstau_warteschlange":
                    b["warteschlange"].setdefault(sid, []).append(
                        (s["stand"], {i: s["zeilen"][i] for i in b["verpasst"].get(sid, set()) if i < len(s["zeilen"])},
                         s["invers"]))
                    b["verpasst"][sid] = set()
            return
        ab = self.abstand(s)
        letzter = b["letzter"].get(sid)
        if self.m != "rueckstau_nachholen_ohne_abstand" and letzter is not None and self.t - letzter < ab \
                and (not durch_versand or b["nachgeholt"].get(sid)):
            self.timer_loeschen("nachholen", (bid, sid))
            self.timer_setzen(letzter + ab, "nachholen", (bid, sid))
            return
        if self.m == "rueckstau_warteschlange" and b["warteschlange"].get(sid):
            for stand, zl, invers in b["warteschlange"].pop(sid):
                self.an(bid, {"art": "Rasteraenderung", "sitzung": sid, "von": b["stand"].get(sid), "stand": stand,
                              "invers": invers, "zeilen": {str(i): v for i, v in sorted(zl.items())}})
                b["stand"][sid] = stand
        if self.m == "rueckstau_nur_letzte_zeilen" and not durch_versand:
            idx = {i for i in s["schmutzig"] if i < len(s["zeilen"])}
        else:
            idx = {i for i in b["verpasst"].get(sid, set()) if i < len(s["zeilen"])}
            if not durch_versand:
                idx |= {i for i in s["schmutzig"] if i < len(s["zeilen"])}
        von = b["stand"].get(sid)
        if von is not None and von >= s["stand"]:
            return
        nachricht = {"art": "Rasteraenderung", "sitzung": sid, "von": von, "stand": s["stand"],
                     "invers": s["invers"], "zeilen": {str(i): s["zeilen"][i] for i in sorted(idx)}}
        if self.m != "cursor_nicht_in_aenderung" and not (self.m == "cursor_nachholen_fehlt" and not durch_versand):
            self.cursor_feld(s, nachricht)
        self.an(bid, nachricht)
        b["stand"][sid] = s["stand"]
        b["verpasst"].pop(sid, None)
        b["letzter"][sid] = self.t
        b["nachgeholt"][sid] = not durch_versand
        if not durch_versand and self.m == "rueckstau_nachholen_zaehlt_versand":
            s["letzter_versand"] = self.t

    def nachholen(self, bid):
        """Alle Sitzungen des Betrachters, in denen er hinter dem Stand liegt, nachholen."""
        for sid in list(self.betrachter[bid]["abos"]):
            s, b = self.sitzungen[sid], self.betrachter[bid]
            if b["stand"].get(sid) is not None and b["stand"][sid] < s["stand"]:
                self.beliefern(bid, sid, False)

    def sync_freigeben(self, sid):
        s = self.sitzungen[sid]
        s["sync"] = False
        self.timer_loeschen("sync", sid)
        zeilen = {i: v for i, v in s["sync_zeilen"].items() if i < len(s["zeilen"])}
        stand, invers, cursor = s["sync_stand"], s["sync_invers"], s["sync_cursor"]
        s["sync_zeilen"], s["sync_stand"], s["sync_invers"], s["sync_cursor"] = {}, None, None, None
        if stand is None:
            return
        for i, v in zeilen.items():
            s["zeilen"][i] = v
        s["stand"] = stand
        cursor_geaendert = False
        if cursor is not None:
            cursor = self.cursor_klemmen(cursor, s["groesse"])
            cursor_geaendert = cursor != s["cursor"]
            s["cursor"] = cursor
        if invers is not None and invers != s["invers"]:
            s["invers"] = invers
            self.schmutzig_machen(sid, zeilen.keys())
            return
        if zeilen or invers is None or (cursor_geaendert and self.m != "cursor_sync_freigabe_ohne_versand"):
            self.schmutzig_machen(sid, zeilen.keys())

    def groesse_setzen(self, sid, sp, ze):
        s = self.sitzungen[sid]
        if s["groesse"] == [sp, ze]:
            return
        s["groesse"] = [sp, ze]
        s["zeilen"] = (s["zeilen"] + [""] * ze)[:ze]
        s["schmutzig"] = {i for i in s["schmutzig"] if i < ze}
        if s["cursor"] is not None:
            s["cursor"] = None if self.m == "cursor_groesse_vergessen" else self.cursor_klemmen(s["cursor"], s["groesse"])
        self.aus["groessen"].append({"t": self.t, "sitzung": sid, "spalten": sp, "zeilen": ze})
        for bid in self.abonnenten(sid):
            self.abzug(bid, sid)
        self.sitzungsliste_senden(s["kundenraum"])

    # ---------- timers ----------
    def zeit_bis(self, t):
        while True:
            faellig = [x for x in self.timer if x[0] <= t]
            if not faellig:
                break
            x = min(faellig)
            self.timer.remove(x)
            self.t = x[0]
            zeit, _, art, key = x
            if art == "versand":
                self.versenden(key)
            elif art == "sync":
                self.sync_freigeben(key)
            elif art == "nachholen":
                bid, sid = key
                if bid in self.betrachter and sid in self.betrachter[bid]["abos"]:
                    self.beliefern(bid, sid, False)
            elif art == "frist":
                a = self.anfragen[key]
                if a["zustand"] == "offen":
                    a["zustand"] = "abgelaufen"
                    self.anfrage_melden(a)
        self.t = t

    def anfrage_melden(self, a):
        raum = self.sitzungen.get(a["sitzung"], {}).get("kundenraum")
        for bid in self.im_raum(raum):
            self.an(bid, dict(art="Anfrage", **self.anfrage_bild(a)))

    # ---------- checks ----------
    def betr(self, e):
        bid = e.get("betrachter")
        if bid not in self.betrachter:
            raise Abgelehnt("betrachter_unbekannt")
        return bid, self.betrachter[bid]

    def sitz(self, e):
        sid = e.get("sitzung")
        if sid not in self.sitzungen:
            raise Abgelehnt("sitzung_unbekannt")
        return sid, self.sitzungen[sid]

    def groesse_pruefen(self, e):
        sp, ze = e.get("spalten"), e.get("zeilen")
        if not (ist_int(sp) and ist_int(ze) and 1 <= sp <= MAX_GROESSE and 1 <= ze <= MAX_GROESSE):
            raise Abgelehnt("groesse_ungueltig")
        return sp, ze

    # ---------- input to the PTY ----------
    def eingabe(self, e, art):
        bid, b = self.betr(e)
        sid, s = self.sitz(e)
        if sid not in b["abos"]:
            raise Abgelehnt("kein_abo")
        if s["zustand"] == "kindende":
            raise Abgelehnt("sitzung_beendet")
        if art == "Antwort":
            d = s["dialog"]
            if d is None or d["id"] != e.get("dialog"):
                raise Abgelehnt("dialog_unbekannt")
            if self.m == "dialog_sperrt_antwort":
                raise Abgelehnt("pane_busy")
            if d["beantwortet"]:
                raise Abgelehnt("bereits_beantwortet")
            w = e.get("wahl")
            if not (ist_int(w) and 1 <= w <= min(9, len(d["optionen"]))):
                raise Abgelehnt("wahl_ungueltig")
            d["beantwortet"] = True
            daten = str(w).encode()
        else:
            if s["dialog"] is not None and self.m != "dialog_sperrt_nicht":
                raise Abgelehnt("pane_busy")
            if art == "Text":
                daten = self.text_bytes(e)
            elif art == "Taste":
                daten = self.taste_bytes(e)
            else:
                daten = self.einfuegen_bytes(e, s)
        if not daten:
            return
        self.pty(sid, daten)
        s["letzte_eingabe"] = bid
        gem = b["gemeldet"].get(sid)
        if gem is not None:
            self.groesse_setzen(sid, *gem)

    def text_bytes(self, e):
        text = e.get("text")
        if not isinstance(text, str):
            raise Abgelehnt("ereignis_ungueltig")
        if any(ord(c) < 0x20 or 0x7f <= ord(c) <= 0x9f for c in text):
            raise Abgelehnt("steuerzeichen_im_text")
        return text.encode("utf-8")

    def taste_bytes(self, e):
        taste = e.get("taste")
        um = e.get("umschalter", [])
        if not isinstance(taste, str) or not isinstance(um, list) or not set(um) <= UMSCHALTER \
                or len(set(um)) != len(um):
            raise Abgelehnt("taste_unbekannt")
        um = tuple(sorted(um))
        if taste == "Enter" and um == ("umschalt",) and self.m == "umschalt_enter_cr":
            return b"\r"
        if taste in ("Links", "Rechts") and um in (("strg",), ("alt",)) and self.m == "mod_pfeil_fehlt":
            raise Abgelehnt("taste_unbekannt")
        if (taste, um) in TASTEN:
            return TASTEN[(taste, um)]
        if len(taste) == 1 and "a" <= taste <= "z":
            if um == ("strg",):
                return bytes([ord(taste) - 96])
            if um == ("alt",):
                return b"\x1b" + taste.encode()
            if um == ("alt", "strg"):
                return b"\x1b" + bytes([ord(taste) - 96])
        raise Abgelehnt("taste_unbekannt")

    def einfuegen_bytes(self, e, s):
        text = e.get("text")
        if not isinstance(text, str):
            raise Abgelehnt("ereignis_ungueltig")
        if s["bracketed_paste"]:
            # With brackets the program takes CR, LF and CRLF alike as a line break; CR is what a terminal sends.
            if self.m != "paste_lf_bleibt":
                text = text.replace("\r\n", "\r").replace("\n", "\r")
        else:
            # Without brackets Claude Code submits on CR, so the core never sends CR: CR and CRLF become LF, LF stays LF.
            text = text.replace("\r\n", "\n")
            if self.m != "paste_cr_ohne_klammern":
                text = text.replace("\r", "\n")
        b = text.encode("utf-8")
        if self.m != "paste_endmarke_durch":
            b = b.replace(PASTE_AN, b"").replace(PASTE_AUS, b"")
        if not b:
            return b""
        if s["bracketed_paste"] and self.m != "paste_ohne_klammern":
            b = PASTE_AN + b + PASTE_AUS
        return b

    # ---------- events ----------
    def ereignis(self, e):
        art = e.get("art")
        fn = getattr(self, "e_" + str(art), None)
        if fn is None:
            raise Abgelehnt("ereignis_unbekannt")
        fn(e)

    # viewers
    def e_Verbinden(self, e):
        bid, pid = e.get("betrachter"), e.get("person")
        if not isinstance(bid, str) or not isinstance(e.get("raum"), str):
            raise Abgelehnt("ereignis_ungueltig")
        if bid in self.betrachter:
            raise Abgelehnt("bereits_verbunden")
        if pid not in self.personen:
            raise Abgelehnt("person_unbekannt")
        self.betrachter[bid] = {"person": pid, "art": e.get("ansicht", "fenster"), "raum": e.get("raum"),
                                "abos": [], "stand": {}, "gemeldet": {}, "ausstehend": 0, "verfallen": 0,
                                "verpasst": {}, "letzter": {}, "nachgeholt": {}, "warteschlange": {}}
        self.an(bid, {"art": "Einstellungen", "werte": self.einstellungen_fuer(pid)})
        self.an(bid, {"art": "Routen", "routen": self.routenliste()})
        self.raumbild(bid)

    def abos_loesen(self, bid):
        for sid, s in self.sitzungen.items():
            if s["letzte_eingabe"] == bid:
                s["letzte_eingabe"] = None
        b = self.betrachter[bid]
        b["abos"], b["stand"], b["gemeldet"] = [], {}, {}
        for sid in list(b["verpasst"]) + list(b["letzter"]):
            self.timer_loeschen("nachholen", (bid, sid))
        b["verpasst"], b["letzter"], b["nachgeholt"], b["warteschlange"] = {}, {}, {}, {}

    def e_Trennen(self, e):
        bid, b = self.betr(e)
        self.abos_loesen(bid)
        del self.betrachter[bid]

    def e_RaumWechsel(self, e):
        bid, b = self.betr(e)
        raum = e.get("raum")
        if not isinstance(raum, str):
            raise Abgelehnt("ereignis_ungueltig")
        if self.m != "raum_abo_bleibt" and self.m != "raum_ohne_leeren":
            self.abos_loesen(bid)
        b["raum"] = raum
        if self.m != "raum_ohne_leeren":
            self.an(bid, {"art": "Leeren", "raum": raum})
        self.raumbild(bid)

    def e_Abo(self, e):
        bid, b = self.betr(e)
        sid, s = self.sitz(e)
        if s["kundenraum"] != b["raum"]:
            raise Abgelehnt("fremder_kundenraum")
        if e.get("an", True):
            sp, ze = self.groesse_pruefen(e)
            if sid not in b["abos"]:
                b["abos"].append(sid)
            b["gemeldet"][sid] = (sp, ze)
            if self.m == "abzug_nur_erstes_abo" and (bid, sid) in self.erste_abos:
                b["stand"][sid] = self.erste_abos[(bid, sid)]
            else:
                self.abzug(bid, sid)
                self.erste_abos.setdefault((bid, sid), s["stand"])
            if s["dialog"] is not None:
                self.an(bid, self.dialog_bild(sid))
            if self.m == "groesse_jede_meldung":
                self.groesse_setzen(sid, sp, ze)
        else:
            if sid not in b["abos"]:
                raise Abgelehnt("kein_abo")
            b["abos"].remove(sid)
            b["stand"].pop(sid, None)
            b["gemeldet"].pop(sid, None)
            self.rueckstand_loeschen(bid, sid)
            if s["letzte_eingabe"] == bid:
                s["letzte_eingabe"] = None

    def e_Groesse(self, e):
        bid, b = self.betr(e)
        sid, s = self.sitz(e)
        if sid not in b["abos"]:
            raise Abgelehnt("kein_abo")
        sp, ze = self.groesse_pruefen(e)
        b["gemeldet"][sid] = (sp, ze)
        if s["letzte_eingabe"] == bid or self.m == "groesse_jede_meldung":
            self.groesse_setzen(sid, sp, ze)

    def e_Text(self, e):
        self.eingabe(e, "Text")

    def e_Taste(self, e):
        self.eingabe(e, "Taste")

    def e_Einfuegen(self, e):
        self.eingabe(e, "Einfuegen")

    def e_Antwort(self, e):
        self.eingabe(e, "Antwort")

    def e_Takt(self, e):
        pass

    def e_Socket(self, e):
        """Der Traeger meldet den Stand des Sockets eines Betrachters: nicht abgenommene Bytes (R14)."""
        bid, b = self.betr(e)
        a = e.get("ausstehend")
        if not ist_int(a) or a < 0:
            raise Abgelehnt("ereignis_ungueltig")
        b["ausstehend"] = a
        if self.m != "rueckstau_kein_nachholen":
            self.nachholen(bid)

    # carrier
    def e_Ausgabe(self, e):
        sid, s = self.sitz(e)
        stand, zeilen = e.get("stand"), e.get("zeilen")
        if not ist_int(stand) or not isinstance(zeilen, dict):
            raise Abgelehnt("ereignis_ungueltig")
        cursor = None
        if "cursor" in e:
            if cursor_gueltig(e["cursor"], s["groesse"][0], s["groesse"][1], self.m):
                cursor = list(e["cursor"])
            elif self.m != "cursor_ungueltig_ignoriert":
                raise Abgelehnt("ereignis_ungueltig")
        self.stand_pruefen(s, stand)
        neu = {}
        for k, v in zeilen.items():
            if not re.fullmatch(r"0|[1-9][0-9]*", k) or int(k) >= s["groesse"][1] or not isinstance(v, str):
                raise Abgelehnt("zeile_ausserhalb")
            neu[int(k)] = v
        self.uebernehmen(sid, s, stand, neu, None, cursor)

    def stand_pruefen(self, s, stand):
        hoechster = s["sync_stand"] if s["sync_stand"] is not None else s["stand"]
        if stand <= hoechster:
            raise Abgelehnt("stand_alt")

    def uebernehmen(self, sid, s, stand, neu, invers, cursor=None):
        """Zeilen, inverser Bildschirm und Cursor eines Standes uebernehmen; in einer synchronisierten Folge wartet alles."""
        if s["sync"] and self.m != "sync_ignorieren":
            s["sync_zeilen"].update(neu)
            s["sync_stand"] = stand
            if invers is not None:
                s["sync_invers"] = invers
            if cursor is not None and self.m != "cursor_ignorieren":
                if self.m == "cursor_sync_ignorieren":
                    s["cursor"] = cursor
                else:
                    s["sync_cursor"] = cursor
            return
        for i, v in neu.items():
            s["zeilen"][i] = v
        s["stand"] = stand
        if cursor is not None and self.m != "cursor_ignorieren" \
                and not (self.m == "cursor_aelterer_gilt" and s["faellig"] is not None):
            s["cursor"] = cursor
        if self.m == "cursor_umgeht_deckel" and not neu and cursor is not None and invers is None:
            self.versenden(sid)  # ein reiner Cursorschritt geht sofort hinaus, am Abstand von R5 vorbei
            return
        if invers is not None and invers != s["invers"]:
            s["invers"] = invers
            self.schmutzig_machen(sid, neu.keys())
        elif neu or (invers is None and not (self.m == "cursor_nur_bei_zeilen" and cursor is not None)):
            self.schmutzig_machen(sid, neu.keys())

    def e_SyncBeginn(self, e):
        sid, s = self.sitz(e)
        if s["sync"]:
            return
        s["sync"] = True
        if self.m != "sync_ohne_zeitgeber":
            self.timer_setzen(self.t + SYNC_MS, "sync", sid)

    def e_SyncEnde(self, e):
        sid, s = self.sitz(e)
        if not s["sync"]:
            return
        self.sync_freigeben(sid)

    def e_Modus(self, e):
        sid, s = self.sitz(e)
        bp, inv = e.get("bracketed_paste"), e.get("invers")
        if ("bracketed_paste" not in e and "invers" not in e) or ("bracketed_paste" in e and not isinstance(bp, bool)) \
                or ("invers" in e and (not isinstance(inv, bool) or not ist_int(e.get("stand")))):
            raise Abgelehnt("ereignis_ungueltig")
        if "invers" in e:
            self.stand_pruefen(s, e["stand"])
        if "bracketed_paste" in e:
            s["bracketed_paste"] = bp
        if "invers" in e:
            if self.m == "invers_ignorieren":
                return
            self.uebernehmen(sid, s, e["stand"], {}, inv)

    def e_Titel(self, e):
        sid, s = self.sitz(e)
        text = e.get("text")
        if not isinstance(text, str):
            raise Abgelehnt("ereignis_ungueltig")
        s["titel"] = text
        self.sitzungsliste_senden(s["kundenraum"])

    def e_Kindende(self, e):
        sid, s = self.sitz(e)
        code = e.get("code")
        if not ist_int(code):
            raise Abgelehnt("ereignis_ungueltig")
        s["zustand"], s["code"] = "kindende", code
        self.sitzungsliste_senden(s["kundenraum"])

    def e_GroessenFolge(self, e):
        sid, s = self.sitz(e)
        if self.m == "groesse_folge_ausfuehren":
            sp, ze = self.groesse_pruefen(e)
            self.groesse_setzen(sid, sp, ze)
            return
        raise Abgelehnt("groesse_nur_kern")

    def e_Zwischenablage(self, e):
        sid, s = self.sitz(e)
        if e.get("lesen"):
            if self.m == "osc52_lesen_antwort":
                self.pty(sid, b"\x1b]52;c;\x07")
            raise Abgelehnt("osc52_lesen")
        daten = e.get("daten_b64")
        if not isinstance(daten, str):
            raise Abgelehnt("ereignis_ungueltig")
        try:
            roh = base64.b64decode(daten, validate=True)
        except (ValueError, base64.binascii.Error):
            raise Abgelehnt("osc52_ungueltig")
        schreiben, _ = self.wirksam("osc52_schreiben", s["person"])
        grenze, _ = self.wirksam("osc52_grenze", s["person"])
        abos = self.abonnenten(sid)
        if schreiben == "aus":
            if self.m == "osc52_aus_hinweis":
                for bid in abos:
                    self.an(bid, {"art": "Hinweis", "hinweis": "osc52_geschrieben", "sitzung": sid,
                                  "laenge": len(roh)})
            return
        zu_gross = len(roh) >= grenze if self.m == "osc52_grenze_exklusiv" else len(roh) > grenze
        if zu_gross:
            if self.m != "osc52_kuerzen":
                raise Abgelehnt("osc52_zu_gross")
            roh = roh[:grenze]
        ziel = s["letzte_eingabe"]
        if ziel is None or ziel not in abos:
            raise Abgelehnt("osc52_kein_empfaenger")
        inhalt = base64.b64encode(roh).decode()
        for bid in (abos if self.m == "osc52_inhalt_an_alle" else [ziel]):
            self.an(bid, {"art": "Zwischenablage", "sitzung": sid, "daten_b64": inhalt})
        for bid in abos:
            self.an(bid, {"art": "Hinweis", "hinweis": "osc52_geschrieben", "sitzung": sid, "laenge": len(roh)})

    # dialog recognition (core, on the grid)
    def dialog_bild(self, sid):
        d = self.sitzungen[sid]["dialog"]
        return {"art": "Dialog", "sitzung": sid, "id": d["id"], "dialog_art": d["art"],
                "optionen": list(d["optionen"])}

    def e_Dialog(self, e):
        sid, s = self.sitz(e)
        did, opt = e.get("id"), e.get("optionen")
        if not isinstance(did, str) or not isinstance(opt, list) or not opt:
            raise Abgelehnt("ereignis_ungueltig")
        s["dialog"] = {"id": did, "art": e.get("dialog_art"), "optionen": opt, "beantwortet": False}
        for bid in self.abonnenten(sid):
            self.an(bid, self.dialog_bild(sid))

    def e_DialogEnde(self, e):
        sid, s = self.sitz(e)
        if s["dialog"] is None:
            raise Abgelehnt("dialog_unbekannt")
        did = s["dialog"]["id"]
        s["dialog"] = None
        for bid in self.abonnenten(sid):
            self.an(bid, {"art": "DialogEnde", "sitzung": sid, "id": did})

    # decision point and gateway
    def e_Anfrage(self, e):
        aid, sid, frist = e.get("id"), e.get("sitzung"), e.get("frist")
        if not isinstance(aid, str) or not ist_int(frist):
            raise Abgelehnt("ereignis_ungueltig")
        if sid not in self.sitzungen:
            raise Abgelehnt("sitzung_unbekannt")
        if aid in self.anfragen:
            a = self.anfragen[aid]
            if a["zustand"] != "offen":
                raise Abgelehnt("nicht_offen")
            a["vorschau"] = e.get("vorschau")
        else:
            a = {"id": aid, "sitzung": sid, "werkzeug": e.get("werkzeug"), "vorschau": e.get("vorschau"),
                 "frist": frist, "zustand": "offen"}
            self.anfragen[aid] = a
            self.anfrage_folge.append(aid)
            if frist <= self.t:
                a["zustand"] = "abgelaufen"
            else:
                self.timer_setzen(frist, "frist", aid)
        self.anfrage_melden(a)

    def e_Nachweis(self, e):
        nid, pid = e.get("nachweis"), e.get("person")
        if not isinstance(nid, str) or pid not in self.personen:
            raise Abgelehnt("ereignis_ungueltig")
        if nid in self.nachweise:
            raise Abgelehnt("nachweis_doppelt")
        self.nachweise[nid] = {"person": pid, "benutzt": False}

    def e_Freigabe(self, e):
        bid, b = self.betr(e)
        ids = e.get("anfragen")
        if not isinstance(ids, list) or not all(isinstance(x, str) for x in ids):
            raise Abgelehnt("ereignis_ungueltig")
        if len(ids) != 1 and not (self.m == "sammelfreigabe" and ids):
            raise Abgelehnt("sammelfreigabe")
        pid = b["person"]
        if self.personen[pid]["art"] != "mensch" and self.m != "agent_freigabe":
            raise Abgelehnt("agent_kennung")
        nid = e.get("nachweis")
        if nid is None:
            raise Abgelehnt("nachweis_fehlt")
        n = self.nachweise.get(nid)
        if n is None or n["person"] != pid:
            raise Abgelehnt("nachweis_ungueltig")
        if n["benutzt"] and self.m != "nachweis_mehrfach":
            raise Abgelehnt("nachweis_verbraucht")
        for aid in ids:
            a = self.anfragen.get(aid)
            if a is None:
                raise Abgelehnt("anfrage_unbekannt")
            if self.sitzungen[a["sitzung"]]["kundenraum"] != b["raum"]:
                raise Abgelehnt("fremder_kundenraum")
            if a["zustand"] == "abgelaufen":
                raise Abgelehnt("abgelaufen")
            if a["zustand"] != "offen":
                raise Abgelehnt("nicht_offen")
        n["benutzt"] = True
        for aid in ids:
            a = self.anfragen[aid]
            a["zustand"] = "erteilt"
            self.timer_loeschen("frist", aid)
            self.aus["an_stelle"].append({"t": self.t, "anfrage": aid, "person": pid, "nachweis": nid})
            self.anfrage_melden(a)

    # routes
    def e_RouteWaehlen(self, e):
        bid, b = self.betr(e)
        sid, s = self.sitz(e)
        if s["kundenraum"] != b["raum"]:
            raise Abgelehnt("fremder_kundenraum")
        name = e.get("route")
        r = next((r for r in self.routen if r["name"] == name), None)
        if r is None:
            self.an(bid, {"art": "Hinweis", "hinweis": "route_ohne_klasse", "route": name})
            raise Abgelehnt("route_ohne_klasse")
        if r["status"] != "frei" and self.m != "route_gesperrt_waehlbar":
            self.an(bid, {"art": "Hinweis", "hinweis": "route_gesperrt", "route": name, "grund": r["grund"]})
            raise Abgelehnt("route_gesperrt")
        s["route"] = name
        self.sitzungsliste_senden(s["kundenraum"])

    # agent state
    def e_AgentAbfrage(self, e):
        wid = e.get("worker")
        if wid not in self.worker:
            raise Abgelehnt("worker_unbekannt")
        z = e.get("zustand")
        if z not in ("laeuft", "wartet", "fertig", "unbekannt"):
            raise Abgelehnt("ereignis_ungueltig")
        w = self.worker[wid]
        if z == "unbekannt":
            self.aus["abfragen"].append({"t": self.t, "worker": wid})
            if self.m == "agent_unbekannt_endstand":
                w["stand"] = {"zustand": "unbekannt", "gesehen_um": self.t, "quelle": e.get("quelle")}
            elif self.m == "agent_alter_neu" and w["stand"] is not None:
                w["stand"] = dict(w["stand"], gesehen_um=self.t)
            else:
                return
        else:
            w["stand"] = {"zustand": z, "gesehen_um": self.t, "quelle": e.get("quelle")}
        raum = self.sitzungen.get(w["sitzung"], {}).get("kundenraum")
        for bid in self.im_raum(raum):
            self.an(bid, dict(art="Agentenzustand", worker=wid, **w["stand"]))

    # messages between sessions
    def e_Nachricht(self, e):
        sid, s = self.sitz(e)
        text = e.get("text")
        if not isinstance(text, str):
            raise Abgelehnt("ereignis_ungueltig")
        h = e.get("herkunft")
        absender = None
        if self.m == "absender_aus_text":
            m = re.match(r"Von ([^:]+):", text)
            if m:
                absender = {"art": "mensch", "person": m.group(1)}
        if absender is None:
            if not isinstance(h, dict) or h.get("art") not in ("sitzung", "mensch"):
                if self.m == "absender_vorgabe_mensch":
                    absender = {"art": "mensch", "person": None}
                else:
                    raise Abgelehnt("absender_fehlt")
            elif h["art"] == "sitzung":
                if h.get("sitzung") not in self.sitzungen:
                    raise Abgelehnt("absender_unbekannt")
                absender = {"art": "sitzung", "sitzung": h["sitzung"]}
            else:
                hb = self.betrachter.get(h.get("betrachter"))
                if hb is None or self.personen[hb["person"]]["art"] != "mensch":
                    raise Abgelehnt("absender_unbekannt")
                absender = {"art": "mensch", "person": hb["person"]}
        for bid in self.im_raum(s["kundenraum"]):
            self.an(bid, {"art": "Nachricht", "sitzung": sid, "absender": absender, "text": text,
                          "zeit": self.t})

    # settings
    def e_EinstellungSetzen(self, e):
        bid, b = self.betr(e)
        k, wert, ebene = e.get("schluessel"), e.get("wert"), e.get("ebene")
        if k not in self.regeln:
            raise Abgelehnt("schluessel_unbekannt")
        if ebene not in ("firma", "gruppe", "nutzer"):
            raise Abgelehnt("ereignis_ungueltig")
        r = self.regeln[k]
        if r["art"] == "zahl":
            if not ist_int(wert) or wert < max(0, r["min"]):
                raise Abgelehnt("wert_ungueltig")
        elif wert not in r["stufen"]:
            raise Abgelehnt("wert_ungueltig")
        pid = b["person"]
        admin = self.personen[pid]["rolle"] == "admin"
        if ebene in ("firma", "gruppe"):
            if not admin and self.m != "nutzer_aendert_regel":
                raise Abgelehnt("nur_admin")
            alt = r[ebene]
            r[ebene] = wert
            betroffen = sorted({x["person"] for x in self.betrachter.values()})
        else:
            if not r["nutzer_darf"]:
                raise Abgelehnt("regel_nicht_beim_nutzer")
            basis, _ = self.basis(k)
            if not self.strenger(k, wert, basis) and self.m != "nutzer_lockert":
                raise Abgelehnt("nur_verschaerfen")
            alt = r["nutzer"].get(pid)
            r["nutzer"][pid] = wert
            betroffen = [pid]
        self.aus["protokoll"].append({"t": self.t, "person": pid, "schluessel": k, "ebene": ebene,
                                      "alt": alt, "neu": wert})
        for xb, x in self.betrachter.items():
            if x["person"] in betroffen:
                self.an(xb, {"art": "Einstellungen", "werte": self.einstellungen_fuer(x["person"])})

    # ---------- end ----------
    def zustand(self):
        sitzungen = []
        for sid in self.reihenfolge:
            s = self.sitzungen[sid]
            z = {"id": sid, "titel": s["titel"], "route": s["route"], "kundenraum": s["kundenraum"],
                 "groesse": list(s["groesse"]), "zustand": s["zustand"], "code": s["code"],
                 "stand": s["stand"], "letzte_eingabe": s["letzte_eingabe"],
                 "dialog": s["dialog"]["id"] if s["dialog"] else None,
                 "bracketed_paste": s["bracketed_paste"], "invers": s["invers"]}
            if self.m != "cursor_zustand_fehlt":
                self.cursor_feld(s, z)
            sitzungen.append(z)
        return {
            "sitzungen": sitzungen,
            "betrachter": {bid: {"person": b["person"], "raum": b["raum"], "abos": list(b["abos"]),
                                 "ausstehend": b["ausstehend"], "verfallen": b["verfallen"],
                                 "rueckstand": {sid: sorted(b["verpasst"].get(sid, ())) for sid in b["abos"]
                                                if b["stand"].get(sid) is not None
                                                and b["stand"][sid] < self.sitzungen[sid]["stand"]}}
                           for bid, b in sorted(self.betrachter.items())},
            "anfragen": {aid: self.anfragen[aid]["zustand"] for aid in self.anfrage_folge},
            "agenten": {wid: w["stand"] for wid, w in sorted(self.worker.items())},
            "regeln": {k: {"firma": r["firma"], "gruppe": r["gruppe"], "nutzer": dict(sorted(r["nutzer"].items()))}
                       for k, r in sorted(self.regeln.items())},
        }


def laufen(fall, mut: str) -> dict:
    if not isinstance(fall, dict):
        raise Ungueltig("Fall ist kein Objekt")
    anfang = fall.get("anfang", {})
    ereignisse = fall.get("ereignisse")
    if not isinstance(anfang, dict) or not isinstance(ereignisse, list):
        raise Ungueltig("anfang oder ereignisse fehlen")
    try:
        k = Kern(anfang, mut)
    except (KeyError, TypeError, ValueError) as exc:
        raise Ungueltig(f"anfang ungueltig: {exc!r}")
    vorher = 0
    for i, e in enumerate(ereignisse):
        if not isinstance(e, dict) or not ist_int(e.get("t")) or e["t"] < vorher:
            raise Ungueltig(f"Ereignis {i}: t fehlt oder faellt")
        vorher = e["t"]
        k.zeit_bis(e["t"])
        k.idx = i
        try:
            k.ereignis(e)
        except Abgelehnt as a:
            k.ablehnen(a.grund)
    aus = dict(k.aus)
    aus["zustand"] = k.zustand()
    return aus


def main(argv):
    mut = os.environ.get("REF_GEGENPROBE", "")
    if mut and mut not in MUTANTEN:
        print(f"zk-ref: unbekannte Gegenprobe {mut!r}", file=sys.stderr)
        return 3
    if argv[1:] == ["--gegenproben"]:
        print("\n".join(MUTANTEN))
        return 0
    if len(argv) > 2:
        print("Aufruf: zk-ref [fall.json]", file=sys.stderr)
        return 2
    try:
        if len(argv) == 2 and argv[1] != "-":
            with open(argv[1], encoding="utf-8") as f:
                roh = f.read()
        else:
            roh = sys.stdin.read()
        fall = json.loads(roh)
        aus = laufen(fall, mut)
    except (Ungueltig, json.JSONDecodeError, UnicodeDecodeError, OSError) as exc:
        print(f"zk-ref: Fall ungueltig: {exc}", file=sys.stderr)
        return 2
    sys.stdout.write(json.dumps(aus, ensure_ascii=False, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
