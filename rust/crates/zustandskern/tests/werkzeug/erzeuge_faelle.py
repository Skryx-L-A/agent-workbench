"""Builds the case files of `tests/faelle/` and the expected effects of `tests/erwartet/` from the Python reference.

Run with Python 3.15 (Python 3.9 reads Base64 differently):

    /opt/homebrew/bin/python3 -I -B erzeuge_faelle.py <contract package> <tests folder>

  <contract package>  folder with `referenz/zk_ref.py` and `aussentests/{faelle,erwartet}/`
  <tests folder>      the `tests/` folder of the crate; `faelle/` and `erwartet/` below it are rewritten

Written: the outside cases of the package (compacted, names `r*` and `x_*`), the 200 random cases `g_2000` to `g_2199`
(seed = number), the 150 random cases with cursors `c_3000` to `c_3149` (the older kind plus cursors from a second
generator), the scenario cases `h_r14_*` and `h_r15_*`; the expectation of every case in `faelle/` (also the older `h_*`,
whose input stays as it is). Every `Freigabe` of a random case carries `entscheidung` (R-L6 point 1); a hand case `h_*` carries it in its file. The generator reads neither the clock nor the date and uses no randomness besides
`random.Random(seed)`: the same call writes the same files.
"""
import importlib.util
import json
import random
import sys
from pathlib import Path

PERSONEN = {"n1": {"rolle": "nutzer", "art": "mensch"}, "n2": {"rolle": "nutzer", "art": "mensch"},
            "adm": {"rolle": "admin", "art": "mensch"}, "ag": {"rolle": "nutzer", "art": "agent"}}
NACHWEISE = {"w1": {"person": "n1"}, "w2": {"person": "n1"}, "w3": {"person": "n2"}, "w4": {"person": "adm"},
             "w5": {"person": "ag"}}
ROUTEN = [{"name": "r-frei", "status": "frei"}, {"name": "r-g", "status": "gesperrt", "grund": "g", "zone": "z"},
          {"name": "r-e", "status": "evaluierung"}]


def dumps(x) -> str:
    return json.dumps(x, ensure_ascii=False, separators=(",", ":"), sort_keys=False)


def referenz(pfad):
    spec = importlib.util.spec_from_file_location("zk_ref", pfad)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Ref:
    def __init__(self, mod):
        self.mod = mod

    def erwartung(self, fall):
        """The expectation of a case: {"exit": 0, "ausgabe": ...} or {"exit": 2, "ausgabe": None}."""
        try:
            aus = self.mod.laufen(fall, "")
        except self.mod.Ungueltig:
            return {"exit": 2, "ausgabe": None}
        return {"exit": 0, "ausgabe": json.loads(json.dumps(aus, ensure_ascii=False, sort_keys=True))}


# ---------------------------------------------------------------- random cases

def zufall_fall(seed: int, mangeln: bool) -> dict:
    r = random.Random(seed)
    regeln = {}
    if r.random() < 0.5:
        regeln["rueckstau_max"] = {}
        for ebene in ("firma", "gruppe"):
            if r.random() < 0.35:
                regeln["rueckstau_max"][ebene] = r.choice([40, 100, 300, 1000, 5000])
        if r.random() < 0.5:
            regeln["rueckstau_max"]["nutzer"] = {p: r.choice([20, 100, 400, 2000]) for p in r.sample(["n1", "n2", "adm"], r.randint(1, 3))}
        if r.random() < 0.25:
            regeln["rueckstau_max"]["vorgabe"] = r.choice([50, 200, 1000])
    if r.random() < 0.5:
        regeln["bildrate_max"] = {}
        if r.random() < 0.5:
            regeln["bildrate_max"]["firma"] = r.choice([5, 10, 20, 30, 40])
        if r.random() < 0.4:
            regeln["bildrate_max"]["nutzer"] = {r.choice(["n1", "n2"]): r.choice([7, 10, 25])}
    if r.random() < 0.2:
        regeln["osc52_grenze"] = {"gruppe": r.choice([4, 9, 40])}
    if r.random() < 0.1:
        regeln["osc52_schreiben"] = {"nutzer_darf": False}
    if r.random() < 0.2:
        regeln["anzahl"] = {"art": "zahl", "vorgabe": 5, "nutzer_darf": True, "min": 0}
    sitzungen = []
    for i, (raum, person) in enumerate([("ra", "n1"), ("rb", "n2"), ("ra", "ag"), ("ra", "adm")]):
        s = {"id": f"s{i + 1}", "titel": r.choice([None, "t", "titel"]), "route": "r-frei", "kundenraum": raum,
             "groesse": [r.choice([5, 20, 80]), r.randint(1, 6)], "person": person}
        if r.random() < 0.4:
            s["zeilen"] = [r.choice(["", "eins", "zwei", "ü"]) for _ in range(r.randint(0, 6))]
        if r.random() < 0.15:
            s["stand"] = r.randint(1, 5)
        if r.random() < 0.15:
            s["invers"] = r.choice([True, 0, 1])
        if r.random() < 0.2:
            s["bracketed_paste"] = r.choice([True, "x"])
        sitzungen.append(s)
    anfang = {"personen": PERSONEN, "nachweise": NACHWEISE, "routen": ROUTEN, "sitzungen": sitzungen,
              "worker": [{"id": "w-a", "sitzung": "s1"}, {"id": "w-b", "sitzung": "s3"}],
              "regeln": regeln}
    bids = [f"b{i}" for i in range(1, 7)]
    verbunden = {}   # bid -> (person, raum)
    abos = {}        # bid -> {sid: True}
    stand = {s["id"]: s.get("stand", 0) for s in sitzungen}
    zeilen = {s["id"]: s["groesse"][1] for s in sitzungen}
    raum_von = {s["id"]: s["kundenraum"] for s in sitzungen}
    ereignisse = []
    t = 0
    dialog_offen = {}
    anfragen = []

    def wahl_b():
        return r.choice(sorted(verbunden)) if verbunden else r.choice(bids)

    def wahl_s(bid=None):
        if bid in verbunden and r.random() < 0.8:
            mit = [s for s in raum_von if raum_von[s] == verbunden[bid][1]]
            if mit:
                return r.choice(mit)
        return r.choice(sorted(raum_von))

    def fehler(e):
        """Replace one scalar field of the event by a value of another type (only scalars, so ids stay hashable)."""
        if not mangeln or r.random() > 0.05:
            return e
        felder = [k for k in e if k not in ("t", "art")]
        if felder:
            e[r.choice(felder)] = r.choice([None, 5, True, 2.5, "x", -1, ""])
        return e

    gewichte = [("Ausgabe", 20), ("Socket", 18), ("Takt", 7), ("Abo", 8), ("Verbinden", 5), ("Trennen", 2),
                ("RaumWechsel", 2), ("Groesse", 3), ("Text", 3), ("Taste", 3), ("Einfuegen", 3), ("Modus", 3),
                ("SyncBeginn", 3), ("SyncEnde", 3), ("EinstellungSetzen", 6), ("Zwischenablage", 2), ("Titel", 1),
                ("Kindende", 1), ("GroessenFolge", 1), ("Dialog", 2), ("DialogEnde", 2), ("Antwort", 2),
                ("AgentAbfrage", 2), ("Nachricht", 2), ("Anfrage", 2), ("Freigabe", 2), ("Nachweis", 1),
                ("RouteWaehlen", 1), ("Unbekannt", 1)]
    arten = [a for a, g in gewichte for _ in range(g)]
    for _ in range(r.randint(40, 110)):
        t += r.choice([0, 0, 1, 2, 5, 9, 15, 16, 17, 20, 33, 50, 99, 100, 101, 250, 700])
        art = r.choice(arten)
        if art == "Ausgabe":
            sid = wahl_s()
            if r.random() < 0.12:
                st = max(0, stand[sid] - r.randint(0, 1))
            else:
                st = stand[sid] + r.choice([1, 1, 1, 2])
            zs = {}
            for _ in range(r.randint(0, 3)):
                zs[str(r.choice([0, 0, 1, 2, 3, 4, 5, 6, 9]))] = r.choice(["", "x", "yy", "zeile", "ü"])
            if r.random() < 0.03:
                zs[r.choice(["01", "-1", "a", ""])] = "q"
            e = {"t": t, "art": "Ausgabe", "sitzung": sid, "stand": st, "zeilen": zs}
            if st > stand[sid]:
                stand[sid] = st
        elif art == "Socket":
            bid = wahl_b()
            a = r.choice([0, 0, 1, 20, 40, 99, 100, 101, 299, 300, 301, 399, 400, 401, 999, 1000, 1001, 5000, 5001,
                          16384, 16385, 20000, 70000])
            if r.random() < 0.04:
                a = r.choice([-1, "x", 2.5, None, True])
            e = {"t": t, "art": "Socket", "betrachter": bid, "ausstehend": a}
        elif art == "Takt":
            e = {"t": t, "art": "Takt"}
        elif art == "Abo":
            bid = wahl_b()
            sid = wahl_s(bid)
            e = {"t": t, "art": "Abo", "betrachter": bid, "sitzung": sid, "spalten": r.choice([10, 80]),
                 "zeilen": r.randint(1, 6)}
            if r.random() < 0.2:
                e["an"] = False
        elif art == "Verbinden":
            bid = r.choice(bids)
            p = r.choice(["n1", "n2", "adm", "ag"])
            e = {"t": t, "art": "Verbinden", "betrachter": bid, "person": p, "raum": r.choice(["ra", "ra", "rb"])}
            if bid not in verbunden:
                verbunden[bid] = (p, e["raum"])
        elif art == "Trennen":
            bid = wahl_b()
            e = {"t": t, "art": "Trennen", "betrachter": bid}
            verbunden.pop(bid, None)
        elif art == "RaumWechsel":
            bid = wahl_b()
            e = {"t": t, "art": "RaumWechsel", "betrachter": bid, "raum": r.choice(["ra", "rb"])}
            if bid in verbunden:
                verbunden[bid] = (verbunden[bid][0], e["raum"])
        elif art == "Groesse":
            bid = wahl_b()
            e = {"t": t, "art": "Groesse", "betrachter": bid, "sitzung": wahl_s(bid), "spalten": r.choice([10, 80]),
                 "zeilen": r.randint(1, 6)}
        elif art in ("Text", "Taste", "Einfuegen", "Antwort"):
            bid = wahl_b()
            e = {"t": t, "art": art, "betrachter": bid, "sitzung": wahl_s(bid)}
            if art == "Text":
                e["text"] = r.choice(["a", "hallo", "ü", "x\x01"])
            elif art == "Taste":
                e["taste"] = r.choice(["Enter", "Tab", "a", "Links", "Hoch", "Nix"])
                e["umschalter"] = r.choice([[], [], ["umschalt"], ["strg"], ["alt"]])
            elif art == "Einfuegen":
                e["text"] = r.choice(["a\nb", "x\r\ny", "\x1b[200~z\x1b[201~", "q"])
            else:
                e["dialog"] = "d1"
                e["wahl"] = r.choice([1, 2, 9, 0])
        elif art == "Modus":
            sid = wahl_s()
            k = r.random()
            if k < 0.4:
                e = {"t": t, "art": "Modus", "sitzung": sid, "bracketed_paste": r.choice([True, False])}
            else:
                st = stand[sid] + r.choice([1, 1, 2]) if r.random() < 0.9 else max(0, stand[sid])
                e = {"t": t, "art": "Modus", "sitzung": sid, "invers": r.choice([True, False]), "stand": st}
                if st > stand[sid]:
                    stand[sid] = st
        elif art in ("SyncBeginn", "SyncEnde"):
            e = {"t": t, "art": art, "sitzung": wahl_s()}
        elif art == "EinstellungSetzen":
            bid = wahl_b()
            k = r.choice(["rueckstau_max", "rueckstau_max", "bildrate_max", "osc52_grenze", "osc52_schreiben", "anzahl",
                          "nix"])
            if k == "osc52_schreiben":
                w = r.choice(["an", "aus", "x"])
            elif k == "rueckstau_max":
                w = r.choice([0, 1, 30, 60, 150, 350, 800, 3000, 16384, 20000])
            elif k == "bildrate_max":
                w = r.choice([0, 1, 5, 10, 20, 40, 63, 100])
            else:
                w = r.choice([0, 3, 7, 50])
            e = {"t": t, "art": "EinstellungSetzen", "betrachter": bid, "schluessel": k,
                 "ebene": r.choice(["firma", "gruppe", "nutzer", "nutzer", "nutzer"]), "wert": w}
        elif art == "Zwischenablage":
            sid = wahl_s()
            e = {"t": t, "art": "Zwischenablage", "sitzung": sid,
                 "daten_b64": r.choice(["YQ==", "YWJj", "YWJjZGVmZ2g=", "=AqcUtg==", "QUJD="])}
            if r.random() < 0.15:
                e = {"t": t, "art": "Zwischenablage", "sitzung": sid, "lesen": True}
        elif art == "Titel":
            e = {"t": t, "art": "Titel", "sitzung": wahl_s(), "text": r.choice(["a", "neu", ""])}
        elif art == "Kindende":
            e = {"t": t, "art": "Kindende", "sitzung": wahl_s(), "code": r.choice([0, 1, 137])}
        elif art == "GroessenFolge":
            e = {"t": t, "art": "GroessenFolge", "sitzung": wahl_s(), "spalten": 9, "zeilen": 9}
        elif art == "Dialog":
            sid = wahl_s()
            e = {"t": t, "art": "Dialog", "sitzung": sid, "id": "d1", "dialog_art": "frage",
                 "optionen": r.choice([["ja", "nein"], ["a"], ["a", "b", "c"]])}
        elif art == "DialogEnde":
            e = {"t": t, "art": "DialogEnde", "sitzung": wahl_s()}
        elif art == "AgentAbfrage":
            e = {"t": t, "art": "AgentAbfrage", "worker": r.choice(["w-a", "w-b", "w-z"]),
                 "zustand": r.choice(["laeuft", "wartet", "fertig", "unbekannt", "x"]), "quelle": r.choice([None, "q", 5])}
        elif art == "Nachricht":
            e = {"t": t, "art": "Nachricht", "sitzung": wahl_s(), "text": "hi",
                 "herkunft": r.choice([{"art": "sitzung", "sitzung": "s1"}, {"art": "mensch", "betrachter": wahl_b()},
                                       {"art": "x"}, None])}
        elif art == "Anfrage":
            aid = f"a{r.randint(1, 3)}"
            e = {"t": t, "art": "Anfrage", "id": aid, "sitzung": wahl_s(), "werkzeug": "w", "vorschau": "v",
                 "frist": t + r.choice([-1, 0, 30, 500])}
            anfragen.append(aid)
        elif art == "Freigabe":
            bid = wahl_b()
            e = {"t": t, "art": "Freigabe", "betrachter": bid,
                 "anfragen": r.choice([[f"a{r.randint(1, 3)}"], [], ["a1", "a2"]]),
                 "nachweis": r.choice(["w1", "w3", "w4", "w5", None])}
        elif art == "Nachweis":
            e = {"t": t, "art": "Nachweis", "nachweis": r.choice(["w1", "w9", "w8"]), "person": r.choice(["n1", "adm", "q"])}
        elif art == "RouteWaehlen":
            bid = wahl_b()
            e = {"t": t, "art": "RouteWaehlen", "betrachter": bid, "sitzung": wahl_s(bid),
                 "route": r.choice(["r-frei", "r-g", "r-e", "r-x"])}
        else:
            e = {"t": t, "art": "Unbekannt", "sitzung": wahl_s()}
        e = fehler(e)
        if art == "Freigabe" and "entscheidung" not in e:
            # R-L6 point 1: `entscheidung` is required; added after `fehler` so the random sequence stays the same.
            e["entscheidung"] = "erteilen"
        ereignisse.append(e)
    t += r.choice([2000, 5000])
    ereignisse.append({"t": t, "art": "Takt"})
    return {"name": f"g_{seed}", "regeln": [], "beschreibung": "random case, expected from the reference",
            "anfang": anfang, "ereignisse": ereignisse}


# ---------------------------------------------------------------- cursor cases (R15)

def mit_cursor(fall: dict, seed: int) -> dict:
    """A random case of the older kind with cursors added: most outputs carry one (some wrong in form or outside the
    size), some sessions start with one. A second generator keeps the older seeds as they were."""
    r = random.Random(seed * 7 + 1)
    for s in fall["anfang"]["sitzungen"]:
        if r.random() < 0.3:
            sp, ze = s["groesse"]
            s["cursor"] = [r.randint(0, ze - 1), r.randint(0, sp - 1), r.choice([True, False])]
        elif r.random() < 0.04:
            s["cursor"] = r.choice([None, [0, 0], [0, 0, 1], [-1, 0, True], [0, 0, True, 0], "x", [0.0, 0, True]])
    for e in fall["ereignisse"]:
        if e.get("art") == "Ausgabe" and r.random() < 0.7:
            k = r.random()
            if k < 0.05:
                e["cursor"] = r.choice([None, [0, 0], [0, 0, 1], [0, 0, "ja"], [-1, 0, True], [0, -1, True], [1.5, 0, True],
                                        [True, 0, True], "00t", [0, 0, True, 1], [99, 0, True], [0, 99, True]])
            else:
                e["cursor"] = [r.choice([0, 0, 1, 2, 3, 5, 6]), r.choice([0, 0, 1, 4, 5, 19, 79, 80]), r.random() < 0.7]
    fall["name"] = f"c_{seed}"
    fall["regeln"] = ["R15"]
    fall["beschreibung"] = "random case with cursors, expected from the reference"
    return fall


# ---------------------------------------------------------------- scenario cases (R14)

def basis(regeln=None, sitzungen=None):
    return {"personen": {"n1": {"rolle": "nutzer", "art": "mensch"}, "n2": {"rolle": "nutzer", "art": "mensch"},
                         "adm": {"rolle": "admin", "art": "mensch"}},
            "nachweise": {}, "routen": [{"name": "r-frei", "status": "frei"}],
            "sitzungen": sitzungen or [
                {"id": "s1", "titel": "eins", "route": "r-frei", "kundenraum": "ra", "person": "n1", "groesse": [10, 4],
                 "zeilen": ["a", "b"]},
                {"id": "s2", "titel": "zwei", "route": "r-frei", "kundenraum": "ra", "person": "n2", "groesse": [10, 4]}],
            "worker": [], "regeln": regeln or {}}


def v(t, b, p, raum="ra"):
    return {"t": t, "art": "Verbinden", "betrachter": b, "person": p, "raum": raum}


def abo(t, b, s="s1", an=True):
    e = {"t": t, "art": "Abo", "betrachter": b, "sitzung": s}
    if an:
        e.update(spalten=10, zeilen=4)
    else:
        e["an"] = False
    return e


def aus(t, stand, zeilen, s="s1"):
    return {"t": t, "art": "Ausgabe", "sitzung": s, "stand": stand, "zeilen": {str(k): x for k, x in zeilen.items()}}


def sock(t, b, n):
    return {"t": t, "art": "Socket", "betrachter": b, "ausstehend": n}


def takt(t):
    return {"t": t, "art": "Takt"}


def einst(t, b, k, ebene, w):
    return {"t": t, "art": "EinstellungSetzen", "betrachter": b, "schluessel": k, "ebene": ebene, "wert": w}


def szenarien():
    f = {}
    f["h_r14_verfall_im_bild"] = ("Expiry in the middle of an image: the report comes between two changes, the due dispatch expires; "
        "a synchronised run ends over the limit", basis(), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(1, "b1"), abo(1, "b2"),
        aus(10, 1, {0: "x"}), aus(12, 2, {1: "y"}), sock(14, "b1", 20000), aus(20, 3, {2: "z"}), takt(26),
        aus(30, 4, {3: "w"}), takt(42), sock(50, "b1", 16384), takt(60),
        {"t": 70, "art": "SyncBeginn", "sitzung": "s1"}, aus(71, 5, {0: "p"}), sock(72, "b1", 16385),
        {"t": 75, "art": "SyncEnde", "sitzung": "s1"}, sock(90, "b1", 100), takt(200)])
    f["h_r14_zwei_grenzen"] = ("Two viewers with different limits; a company limit set later; the user limit stays when stricter", basis(
        regeln={"rueckstau_max": {"nutzer": {"n1": 100}}}), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), v(0, "b3", "adm"), abo(1, "b1"), abo(1, "b2"), abo(1, "b3"),
        sock(2, "b1", 500), sock(2, "b2", 500), sock(2, "b3", 500),
        aus(100, 1, {0: "x"}), aus(200, 2, {1: "y"}),
        einst(250, "b2", "rueckstau_max", "nutzer", 400), aus(300, 3, {2: "z"}),
        einst(350, "b3", "rueckstau_max", "firma", 450), aus(400, 4, {3: "w"}),
        sock(500, "b1", 100), sock(500, "b2", 400), sock(500, "b3", 450), takt(600)])
    f["h_r14_nie_unter_grenze"] = ("A viewer that never comes under the limit: the count grows, the backlog per session stays bounded", basis(), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(1, "b1"), abo(1, "b2"), abo(1, "b1", "s2"), abo(1, "b2", "s2"),
        sock(2, "b1", 70000),
        aus(10, 1, {0: "a"}), aus(30, 2, {1: "b"}), aus(50, 3, {2: "c"}, "s2"), aus(70, 4, {3: "d"}),
        aus(90, 5, {0: "e"}), aus(110, 6, {0: "f"}, "s2"), aus(130, 7, {1: "g"}), aus(150, 8, {1: "h"}),
        sock(160, "b1", 80000), takt(300)])
    f["h_r14_abzug_groesse"] = ("A full image is never held back: a size change clears the backlog of a viewer over the limit", basis(), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(1, "b1"), abo(1, "b2"),
        sock(2, "b1", 30000), aus(10, 1, {0: "a"}), aus(30, 2, {1: "b"}), aus(50, 3, {2: "c"}),
        {"t": 60, "art": "Text", "betrachter": "b1", "sitzung": "s1", "text": "x"},
        {"t": 70, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 12, "zeilen": 3},
        {"t": 80, "art": "Text", "betrachter": "b1", "sitzung": "s1", "text": "y"},
        sock(90, "b1", 20000), aus(100, 4, {0: "d"}), sock(110, "b1", 10), takt(300)])
    f["h_r14_abzug_abo"] = ("Subscribing again gives a full image over the limit and clears the backlog; leaving clears it too", basis(), [
        v(0, "b1", "n1"), abo(1, "b1"), abo(1, "b1", "s2"), sock(2, "b1", 30000),
        aus(10, 1, {0: "a"}), aus(30, 2, {1: "b"}, "s2"), aus(50, 3, {2: "c"}), aus(70, 4, {3: "d"}, "s2"),
        abo(80, "b1"), takt(90), abo(100, "b1", "s2", False), sock(110, "b1", 0), takt(300)])
    f["h_r14_verbinden_neu"] = ("Reconnecting starts at 0 and 0; a room change keeps the count and the report but not the backlog", basis(), [
        v(0, "b1", "n1"), abo(1, "b1"), sock(2, "b1", 30000), aus(10, 1, {0: "a"}), aus(30, 2, {1: "b"}), takt(50),
        {"t": 60, "art": "RaumWechsel", "betrachter": "b1", "raum": "rb"}, takt(70),
        {"t": 80, "art": "Trennen", "betrachter": "b1"}, v(90, "b1", "n1"), abo(91, "b1"), aus(100, 3, {0: "c"}),
        takt(200)])
    f["h_r14_nachholen_timer_nach_versand"] = (
        "The catch-up waits for the distance; a dispatch of the session reaches the viewer first; the timer then finds nothing to do",
        basis(), [
        v(0, "b1", "n1"), v(0, "b3", "adm"), abo(0, "b1"), aus(0, 1, {0: "a"}), sock(2, "b1", 30000),
        aus(3, 2, {1: "b"}), takt(16), aus(17, 3, {2: "c"}), einst(17, "b3", "bildrate_max", "firma", 10),
        sock(18, "b1", 0), takt(32), takt(100), takt(300)])
    f["h_r14_nachholen_abstand"] = ("A catch-up inside the distance sets a timer at its end; a second report moves it; leaving ends it", basis(
        regeln={"bildrate_max": {"firma": 10}}), [
        v(0, "b1", "n1"), v(0, "b2", "n1"), abo(0, "b1"), abo(0, "b2"), aus(0, 1, {0: "a"}),
        sock(1, "b1", 30000), sock(1, "b2", 30000), aus(2, 2, {1: "b"}), takt(100), sock(101, "b1", 0),
        sock(102, "b1", 0), takt(150), sock(160, "b2", 0), {"t": 170, "art": "Trennen", "betrachter": "b2"},
        takt(400)])
    f["h_r14_nachholen_bild_ohne_versand"] = ("The catch-up carries the backlog and the lines not yet sent; the session's own dispatch follows later", basis(
        regeln={"bildrate_max": {"firma": 20}}), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(0, "b1"), abo(0, "b2"), aus(0, 1, {0: "a"}), sock(1, "b1", 30000),
        aus(50, 2, {1: "b"}), aus(60, 3, {2: "c"}), aus(120, 4, {3: "d"}), aus(125, 5, {0: "e"}),
        sock(130, "b1", 100), takt(200), takt(300)])
    f["h_r14_socket_ungueltig"] = ("Socket reports that are not valid", basis(), [
        v(0, "b1", "n1"),
        {"t": 1, "art": "Socket", "betrachter": "nobody", "ausstehend": 5}, {"t": 2, "art": "Socket", "betrachter": "b1"},
        {"t": 3, "art": "Socket", "betrachter": "b1", "ausstehend": -1}, {"t": 4, "art": "Socket", "betrachter": "b1", "ausstehend": "5"},
        {"t": 5, "art": "Socket", "betrachter": "b1", "ausstehend": 5.5}, {"t": 6, "art": "Socket", "betrachter": "b1", "ausstehend": True},
        {"t": 7, "art": "Socket", "betrachter": "b1", "ausstehend": None}, {"t": 8, "art": "Socket", "ausstehend": 5},
        {"t": 9, "art": "Socket", "betrachter": "b1", "ausstehend": 0}, {"t": 10, "art": "Socket", "betrachter": "b1", "ausstehend": 9223372036854775807},
        takt(20)])
    f["h_r14_grenze_eins"] = ("The smallest limit is 1; 0 is not a valid value; exactly the limit passes", basis(), [
        v(0, "b1", "n1"), abo(0, "b1"), aus(0, 1, {0: "a"}),
        einst(1, "b1", "rueckstau_max", "nutzer", 0), einst(2, "b1", "rueckstau_max", "nutzer", 1),
        sock(3, "b1", 1), aus(20, 2, {1: "b"}), sock(25, "b1", 2), aus(40, 3, {2: "c"}), sock(45, "b1", 1),
        takt(100), einst(110, "b1", "rueckstau_max", "nutzer", 1), takt(200)])
    f["h_r14_mehrere_sitzungen"] = ("One report catches a viewer up in every session it is behind in", basis(), [
        v(0, "b1", "n1"), abo(0, "b1"), abo(0, "b1", "s2"), aus(0, 1, {0: "a"}), aus(0, 1, {0: "z"}, "s2"),
        sock(1, "b1", 30000), aus(20, 2, {1: "b"}), aus(25, 2, {1: "y"}, "s2"), takt(40), sock(60, "b1", 16384),
        takt(200)])
    f["h_r14_sync_nachholen"] = ("A report under the limit during a synchronised run catches up with the released stand", basis(), [
        v(0, "b1", "n1"), abo(0, "b1"), aus(0, 1, {0: "a"}), sock(1, "b1", 30000), aus(20, 2, {1: "b"}),
        {"t": 40, "art": "SyncBeginn", "sitzung": "s1"}, aus(41, 3, {2: "c"}), sock(42, "b1", 0),
        {"t": 50, "art": "SyncEnde", "sitzung": "s1"}, takt(100)])
    f["h_r14_invers_nachholen"] = ("A change of the inverse mode expires like a line and the catch-up carries the current value", basis(), [
        v(0, "b1", "n1"), abo(0, "b1"), aus(0, 1, {0: "a"}), sock(1, "b1", 30000),
        {"t": 20, "art": "Modus", "sitzung": "s1", "invers": True, "stand": 2}, sock(30, "b1", 0), takt(100)])
    for nr, spaet in (("a", 229), ("b", 230)):
        f[f"h_r14_abstand_genau_{nr}"] = (
            "The catch-up inside the distance waits (one ms short) and exactly one distance after the last change it goes at once",
            basis(regeln={"bildrate_max": {"firma": 10}}), [
            v(0, "b1", "n1"), abo(0, "b1"), aus(0, 1, {0: "a"}), sock(1, "b1", 20000), aus(2, 2, {1: "b"}), takt(100),
            sock(130, "b1", 0), sock(131, "b1", 20000), aus(140, 3, {2: "c"}), takt(200), sock(spaet, "b1", 0),
            takt(231), takt(400)])
    f["h_r14_versand_wartet_nach_nachholen"] = (
        "After a catch-up a dispatch of the session inside the distance waits for the viewer; the timer then brings the lines",
        basis(regeln={"bildrate_max": {"firma": 10}}), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(0, "b1"), abo(0, "b2"), aus(0, 1, {0: "a"}), sock(1, "b1", 20000),
        aus(2, 2, {1: "b"}), takt(100), sock(130, "b1", 0), aus(140, 3, {2: "c"}), takt(200), takt(229), takt(230),
        takt(400)])
    f["h_r14_gleiche_zeit"] = ("Several reports and dispatches at the same time", basis(), [
        v(0, "b1", "n1"), abo(0, "b1"), aus(0, 1, {0: "a"}), sock(0, "b1", 30000), aus(0, 2, {1: "b"}), sock(0, "b1", 0),
        aus(16, 3, {2: "c"}), sock(16, "b1", 30000), aus(32, 4, {3: "d"}), sock(32, "b1", 16384), takt(64)])
    # R15 cursor
    g = [{"id": "s1", "titel": "eins", "route": "r-frei", "kundenraum": "ra", "person": "n1", "groesse": [10, 4],
          "zeilen": ["a", "b"]}]

    def cur(t, stand, c, zeilen=None, s="s1"):
        e = aus(t, stand, zeilen or {}, s)
        e["cursor"] = c
        return e

    f["h_r15_gleicher_cursor_sendet"] = ("The same cursor again is still an output: stand moves, a change without lines goes out", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [1, 2, True]), cur(40, 2, [1, 2, True]), aus(80, 3, {}), takt(200)])
    f["h_r15_sichtbar_wechsel"] = ("Only the visibility changes; a missing cursor field keeps the cursor of the core", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [0, 0, True]), cur(40, 2, [0, 0, False]), aus(80, 3, {1: "x"}),
        {"t": 100, "art": "Abo", "betrachter": "b1", "sitzung": "s1", "spalten": 10, "zeilen": 4}, takt(200)])
    f["h_r15_rand"] = ("The last row and column are valid, one more is not; a zero-sized start takes no cursor", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [3, 9, True]), cur(40, 2, [4, 9, True]), cur(80, 3, [3, 10, True]),
        cur(120, 4, [-1, 0, True]), cur(160, 5, [0, -1, True]), cur(200, 6, [0, 0, True]), takt(300)])
    f["h_r15_ungueltig_alter_stand"] = ("An invalid cursor with an old stand is event invalid, not stand old; a valid one with an old stand is stand old", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 5, [0, 0, True]), cur(40, 5, [9, 9, True]), cur(60, 5, [0, 1, True]),
        cur(80, 2, "x"), cur(90, 2, [0, 1, True]), takt(200)])
    f["h_r15_sync_nur_cursor"] = ("A synchronised run with only cursor steps: the latest cursor goes out at the end, nothing before; "
        "with a Modus of equal invers the cursor step still goes out", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [0, 0, True]),
        {"t": 100, "art": "SyncBeginn", "sitzung": "s1"}, cur(101, 2, [1, 1, True]), cur(102, 3, [2, 2, True]),
        {"t": 110, "art": "SyncEnde", "sitzung": "s1"}, takt(130),
        {"t": 200, "art": "SyncBeginn", "sitzung": "s1"}, cur(201, 4, [2, 2, True]),
        {"t": 202, "art": "Modus", "sitzung": "s1", "invers": False, "stand": 5},
        {"t": 210, "art": "SyncEnde", "sitzung": "s1"}, takt(230),
        {"t": 300, "art": "SyncBeginn", "sitzung": "s1"}, cur(301, 6, [2, 2, True]),
        {"t": 302, "art": "Modus", "sitzung": "s1", "invers": False, "stand": 7},
        {"t": 310, "art": "SyncEnde", "sitzung": "s1"}, takt(400)])
    f["h_r15_sync_zeitgeber_klemmen"] = ("A cursor held in a synchronised run is clamped against the size that holds at the release", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [0, 0, True]),
        {"t": 100, "art": "SyncBeginn", "sitzung": "s1"}, cur(101, 2, [3, 9, True]),
        {"t": 102, "art": "Text", "betrachter": "b1", "sitzung": "s1", "text": "x"},
        {"t": 103, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 5, "zeilen": 2},
        {"t": 104, "art": "Text", "betrachter": "b1", "sitzung": "s1", "text": "y"}, takt(300)])
    f["h_r15_groesse_klemmt_einzeln"] = ("A size change clamps row and column each, and only when they lie outside", basis(sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1"), cur(10, 1, [3, 2, False]),
        {"t": 20, "art": "Text", "betrachter": "b1", "sitzung": "s1", "text": "x"},
        {"t": 30, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 10, "zeilen": 2},
        {"t": 40, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 2, "zeilen": 2},
        {"t": 50, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 6, "zeilen": 1},
        {"t": 60, "art": "Groesse", "betrachter": "b1", "sitzung": "s1", "spalten": 6, "zeilen": 5}, takt(100)])
    f["h_r15_deckel_sturm"] = ("A storm of pure cursor steps at a rate limit of 10: one change per distance, the latest cursor in the last", basis(
        regeln={"bildrate_max": {"firma": 10}}, sitzungen=g), [
        v(0, "b1", "n1"), abo(1, "b1")] + [cur(10 + 7 * i, 1 + i, [i % 4, i % 10, True]) for i in range(40)] + [takt(400), takt(1000)])
    f["h_r15_nachholen_nur_cursor"] = ("A viewer over the limit misses only cursor steps; the catch-up is a change without lines", basis(sitzungen=g), [
        v(0, "b1", "n1"), v(0, "b2", "n2"), abo(1, "b1"), abo(1, "b2"), cur(10, 1, [0, 0, True]), sock(11, "b1", 30000),
        cur(40, 2, [1, 1, True]), cur(80, 3, [2, 2, True]), takt(100), sock(150, "b1", 100), takt(300)])
    f["h_r15_zustand_neubau"] = ("The cursor of the end state goes into a new core as the start: the first full image has it", basis(sitzungen=[
        dict(g[0], cursor=[3, 9, False])]), [v(0, "b1", "n1"), abo(1, "b1"), takt(10), cur(20, 1, [0, 0, True]), takt(100)])
    return f


# ---------------------------------------------------------------- main

def schreiben(pfad: Path, wert):
    pfad.write_text(dumps(wert), encoding="utf-8")


def main(argv):
    if len(argv) != 3:
        print(__doc__)
        return 64
    paket, tests = Path(argv[1]), Path(argv[2])
    ref = Ref(referenz(paket / "referenz" / "zk_ref.py"))
    faelle, erwartet = tests / "faelle", tests / "erwartet"
    faelle.mkdir(exist_ok=True)
    erwartet.mkdir(exist_ok=True)
    # outside cases, compacted
    for p in sorted((paket / "aussentests" / "faelle").glob("*.json")):
        schreiben(faelle / p.name, json.loads(p.read_text(encoding="utf-8")))
    # random cases
    abgewichen = []
    for seed in range(2000, 2200):
        fall = zufall_fall(seed, True)
        try:
            ref.erwartung(fall)
        except Exception:
            fall = zufall_fall(seed, False)
            abgewichen.append(seed)
        schreiben(faelle / f"g_{seed}.json", fall)
    # random cases with cursors (R15)
    for seed in range(3000, 3150):
        schreiben(faelle / f"c_{seed}.json", mit_cursor(zufall_fall(seed, True), seed))
    # scenarios
    for name, (text, anfang, ereignisse) in szenarien().items():
        schreiben(faelle / f"{name}.json", {"name": name, "regeln": ["R15" if name.startswith("h_r15") else "R14"], "beschreibung": text, "anfang": anfang,
                                              "ereignisse": ereignisse})
    # expectations of every case
    n = 0
    for p in sorted(faelle.glob("*.json")):
        text = p.read_text(encoding="utf-8")
        try:
            fall = json.loads(text)
        except ValueError:
            erg = {"exit": 2, "ausgabe": None}
        else:
            erg = ref.erwartung(fall)
        schreiben(erwartet / p.name, erg)
        n += 1
    print(f"{n} cases; random cases without errors in the event fields: {abgewichen}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
