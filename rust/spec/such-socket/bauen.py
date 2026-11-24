#!/usr/bin/env python3
"""Baut die sichtbaren Fälle von Vertrag 4 (Such-Socket je Sitzung): `sichtbar/faelle` und `sichtbar/erwartet`.

Die Rangliste ohne Tor kommt aus der Referenz des Vertrags `rangkern` (Python-Vorbild, sicherer Starter). Das Tor
rechnet dieses Skript nach Vertrag 3 (`darf`, Gründe in fester Reihenfolge, Stand 20.11.2026). Ändert sich Vertrag 3,
wird `darf` unten angepasst und der Satz neu gebaut.

Aufruf (eine Python-Umgebung, in der `braincli` importierbar ist):
    REF_SICHER=1 <python> -I -B bauen.py <ordner mit rangkern_ref.py>
"""
from __future__ import annotations

import contextlib
import io
import json
import os
import re
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent
SCHLUESSEL = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
SITZUNG = "s-test-1"
HEUTE = "2026-11-20"

EINSTELLUNGEN = {
    "fassung": 1,
    "klassen": ["oeffentlich", "intern", "kundendaten"],
    "vorgabestufe": "kundendaten",
    "routen": [
        {"schluessel": "cloud", "art": "cloud", "klasse": "intern"},
        {"schluessel": "lokal", "art": "lokal", "klasse": "kundendaten"},
    ],
    "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []}],
    "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
}

FEHLT = object()


# --- Tor nach Vertrag 3 -------------------------------------------------------------------------------------------
def darf(einst: dict, klasse, kunde, route_key: str) -> tuple[bool, str, str | None]:
    klassen = einst["klassen"]
    route = next(r for r in einst["routen"] if r["schluessel"] == route_key)
    if klasse is None:
        klasse = einst.get("vorgabestufe", klassen[-1])
    if not isinstance(klasse, str) or klasse not in klassen:
        return False, "einheit-ohne-klasse", None
    pos, hoechste = klassen.index(klasse), len(klassen) - 1
    if kunde is not None:
        pos = hoechste
    wirksam = klassen[pos]
    if pos > klassen.index(route["klasse"]):
        return False, "klasse-ueber-route", wirksam
    if pos == hoechste:
        if kunde is None:
            return False, "kunde-fehlt", wirksam
        eintrag = next((k for k in einst["kunden"] if k["schluessel"] == kunde), None)
        if eintrag is None:
            return False, "kunde-ohne-eintrag", wirksam
        if route_key in eintrag.get("verboten", []):
            return False, "route-fuer-kunde-verboten", wirksam
        if route_key not in eintrag.get("erlaubt", []):
            return False, "route-fuer-kunde-nicht-erlaubt", wirksam
        freigaben = einst["kundendaten_an_cloud"]["freigaben"]
        if route["art"] == "cloud" and {"route": route_key, "kunde": kunde} not in freigaben:
            return False, "kundendaten-an-cloud", wirksam
    return True, "frei", wirksam


def eingaben(fm: dict) -> tuple[object, object, str | None]:
    """(klasse für darf, kunde für darf, kunde fürs Protokoll) aus dem Frontmatter, wie Vertrag 4 es festlegt."""
    klasse = fm.get("datenklasse")
    kunde = fm.get("kunde")
    if kunde is not None and not isinstance(kunde, str):
        kunde = ""  # vorhanden, aber kein Text: darf sperrt mit kunde-ohne-eintrag
    log_kunde = kunde if isinstance(kunde, str) and SCHLUESSEL.match(kunde) else None
    return klasse, kunde, log_kunde


# --- Notizen ------------------------------------------------------------------------------------------------------
def notiz(title: str, body: str, **fm) -> dict:
    return {"fm": {"title": title, "type": "note", **fm}, "body": body}


def text_von(n: dict) -> str:
    zeilen = ["---"]
    for k, v in n["fm"].items():
        zeilen.append(f"{k}: {json.dumps(v, ensure_ascii=False)}")
    zeilen += ["---", "", n["body"], ""]
    return "\n".join(zeilen)


def grund_vault() -> dict:
    return {
        "projekte/lieferplan.md": notiz("Lieferplan Lager", "Der Lieferplan regelt die Termine im Lager.",
                                        datenklasse="intern"),
        "kunden/nord/lieferplan-qkx1.md": notiz(
            "Lieferplan QKX1", "Lieferplan QKX1: Lieferplan und Preise des Kunden. Lieferplan QKX1 bleibt intern.",
            datenklasse="kundendaten", kunde="k-nord"),
        "handbuch/lieferplan-lesen.md": notiz("Lieferplan lesen", "Wie man einen Lieferplan liest.",
                                              datenklasse="oeffentlich"),
        "eingang/lieferplan-qkx2.md": notiz("Lieferplan QKX2", "Lieferplan QKX2 kam ohne Klasse an."),
        "sonst/wetter.md": notiz("Wetter", "Sonne und Regen im Herbst.", datenklasse="intern"),
    }


# --- Referenz -----------------------------------------------------------------------------------------------------
REF = None


def liste_ohne_tor(notes: dict, extra: dict, frage: str) -> dict:
    with tempfile.TemporaryDirectory(prefix="ss-bau-") as d:
        v = Path(d) / "vault"
        for rel, n in notes.items():
            p = v / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text_von(n), encoding="utf-8")
        for rel, t in extra.items():
            p = v / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(t, encoding="utf-8")
        roh = io.BytesIO()
        out = io.TextIOWrapper(roh, encoding="utf-8")
        with contextlib.redirect_stdout(out):
            code = REF.main(["search", str(v), "-k", "50", "--today", HEUTE, "--", frage])
            out.flush()
        text = roh.getvalue().decode("utf-8")
        assert code == 0, (frage, code, text)
        return json.loads(text)


def widersprueche(extra: dict) -> list[tuple[str, str]]:
    roh = extra.get("_meta/state/contradictions.json")
    if not roh:
        return []
    paare = []
    for f in json.loads(roh)["findings"].values():
        if f.get("status") == "open":
            paare.append((f["note_a"]["rel"], f["note_b"]["rel"]))
    return paare


def suche(notes, extra, route, req, protokoll_schreibbar) -> tuple[dict, list[dict]]:
    k = req.get("k", 5)
    roh = liste_ohne_tor(notes, extra, req["query"])
    L = roh["hits"]

    def urteil(rel):
        klasse, kunde, log_kunde = eingaben(notes[rel]["fm"])
        ok, grund, wirksam = darf(EINSTELLUNGEN, klasse, kunde, route)
        return ok, grund, wirksam, log_kunde

    urteile = [urteil(h["rel"]) for h in L]
    erlaubt = [h for h, u in zip(L, urteile) if u[0]][:k]
    gruppen: dict[tuple, int] = {}
    for h, u in zip(L[:k], urteile[:k]):
        if not u[0]:
            schl = (u[2], u[3], u[1])
            gruppen[schl] = gruppen.get(schl, 0) + 1
    gesperrt = sum(gruppen.values())
    if gesperrt and not protokoll_schreibbar:
        antwort = {"error": "protokoll"}
        if "id" in req:
            antwort["id"] = req["id"]
        return antwort, [], []
    felder = {}
    volle = []
    paare = widersprueche(extra)
    for h in erlaubt:
        pruef = {}
        voll = dict(h)
        if "abgeloest_durch" in h:
            rest = [r for r in h["abgeloest_durch"] if r in notes and urteil(r)[0]]
            if rest != h["abgeloest_durch"]:
                pruef["abgeloest_durch"] = rest or None
            if rest:
                voll["abgeloest_durch"] = rest
            else:
                voll.pop("abgeloest_durch")
        if h.get("contradiction"):
            partner = [b for a, b in paare if a == h["rel"]] + [a for a, b in paare if b == h["rel"]]
            if partner and not any(p in notes and urteil(p)[0] for p in partner):
                pruef["contradiction"] = None
                voll.pop("contradiction")
        if pruef:
            felder[h["rel"]] = pruef
        volle.append(voll)
    antwort = {"fallback": roh["fallback"], "rels": [h["rel"] for h in erlaubt], "gesperrt": gesperrt}
    if felder:
        antwort["pruefe_felder"] = felder
    if "id" in req:
        antwort["id"] = req["id"]
    zeilen = [{"aktionsname": "ausgabe.gesperrt", "ergebnis": "gesperrt", "fassung": 2, "route": route,
               "sitzung": SITZUNG, "klasse": kl, "kunde": ku, "grund": gr, "anzahl": n}
              for (kl, ku, gr), n in gruppen.items()]
    return antwort, zeilen, volle


def zaehle(wert, marke: str) -> int:
    """Wie oft `marke` in den Texten einer Antwort steht (Marker sind ASCII und stehen nie in Schlüsseln)."""
    if isinstance(wert, str):
        return wert.count(marke)
    if isinstance(wert, dict):
        return sum(zaehle(v, marke) for v in wert.values())
    if isinstance(wert, list):
        return sum(zaehle(v, marke) for v in wert)
    return 0


# --- Fälle --------------------------------------------------------------------------------------------------------
def args(route="cloud", **ueber) -> list[str]:
    werte = {"--socket": "{sock}", "--route": route, "--sitzung": SITZUNG,
             "--einstellungen": "{root}/aux/einstellungen.json", "--protokoll": "{root}/protokoll"}
    werte.update(ueber)
    a = ["sitzung", "{root}/vault"]
    for opt, wert in werte.items():
        if wert is not FEHLT:
            a += [opt, wert]
    return a


def S(query, **f):
    return {"request": {"op": "search", "query": query, **f}}


BAD = {"error": "bad_request"}
FAELLE: dict[str, dict] = {}


def fall(name, description, steps, route="cloud", notes=None, extra=None, verboten=(), start="bereit", argv=None,
         einstellungen=EINSTELLUNGEN, einstellungen_roh=None, vorher=None, protokoll_ordner=True, rechte="0600",
         marker=None, dirs=()):
    FAELLE[name] = dict(description=description, steps=steps, route=route,
                        notes=grund_vault() if notes is None else notes, extra=extra or {}, verboten=list(verboten),
                        start=start, argv=argv or args(route), einstellungen=einstellungen,
                        einstellungen_roh=einstellungen_roh, vorher=vorher or {}, protokoll_ordner=protokoll_ordner,
                        rechte=rechte, marker=marker, dirs=list(dirs))


fall("p-ping", "ping mit id als Zahl und als Text", [
    {"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}},
    {"request": {"op": "ping", "id": "a"}, "antwort": {"ok": True, "id": "a"}},
    {"request": {"op": "ping"}, "antwort": {"ok": True}}])

fall("p-route-in-anfrage", "Eine Route in der Anfrage ist ungültig; die Route kommt nur aus dem Start", [
    {"request": {"op": "search", "query": "Lieferplan", "route": "lokal", "id": 1}, "antwort": dict(BAD, id=1)},
    S("Lieferplan", id=2)], verboten=["qkx1", "qkx2"])

fall("p-felder-fremd", "Felder für Kunde, Sitzung, Klasse, Datum, Vektor und Vault sind in der Anfrage ungültig", [
    *[{"request": {"op": "search", "query": "Lieferplan", feld: wert}, "antwort": BAD} for feld, wert in [
        ("kunde", "k-nord"), ("sitzung", "s-andere"), ("klasse", "kundendaten"), ("datenklasse", "kundendaten"),
        ("today", HEUTE), ("query_vector", [0.1, 0.2]), ("vault", "/")]],
    {"request": {"op": "ping"}, "antwort": {"ok": True}}], verboten=["qkx1", "qkx2"])

fall("p-quit", "quit ist über den Socket ungültig; der Prozess läuft weiter", [
    {"request": {"op": "quit", "id": 7}, "antwort": dict(BAD, id=7)},
    {"request": {"op": "ping"}, "antwort": {"ok": True}}])

fall("p-k-grenzen", "k zwischen 1 und 50 als ganze Zahl; sonst ungültig", [
    {"request": {"op": "search", "query": "Lieferplan", "k": 0}, "antwort": BAD},
    {"request": {"op": "search", "query": "Lieferplan", "k": 51}, "antwort": BAD},
    {"request": {"op": "search", "query": "Lieferplan", "k": "5"}, "antwort": BAD},
    {"line": "{\"op\": \"search\", \"query\": \"Lieferplan\", \"k\": 5.0}", "antwort": BAD},
    S("Lieferplan", k=50)], verboten=["qkx1", "qkx2"])

fall("p-zeilen-ungueltig", "Zeilen, die kein gültiges Anfrageobjekt sind", [
    {"line": "kein json", "antwort": BAD},
    {"line": "[]", "antwort": BAD},
    {"line": "", "antwort": BAD},
    {"line": "{\"op\": \"ping\", \"op\": \"ping\"}", "antwort": BAD},
    {"request": {"op": "ping"}, "antwort": {"ok": True}}])

fall("t-cloud-sperrt", "Cloud-Route: Kundennotiz und Notiz ohne Klasse kommen nicht an, nur ihre Zahl",
     [S("Lieferplan", id=1)], verboten=["qkx1", "qkx2"])

fall("t-lokal-liefert", "Lokale Route: die Kundennotiz kommt an; die Notiz ohne Klasse (Vorgabestufe, ohne Kunde) "
     "nicht", [S("Lieferplan", id=1)], route="lokal", verboten=["qkx2"])

fall("t-tor-vor-schnitt", "k = 1: die gesperrte Spitze belegt keinen Platz; der erste erlaubte Treffer kommt",
     [S("Lieferplan", k=1)], verboten=["qkx1", "qkx2"])

fall("t-praefix", "Die Treffer für k sind der Anfang der Treffer für k + 1",
     [S("Lieferplan", k=n) for n in (1, 2, 3, 4)], verboten=["qkx1", "qkx2"])

fall("t-leer", "Keine passende Notiz: keine Treffer, nichts gesperrt, keine Protokollzeile",
     [S("Zzzkeinwortdafuer")], verboten=["qkx1", "qkx2"])

n_angebot = {
    "projekte/angebot.md": notiz("Angebot Lager", "Das Angebot für das Lager.", datenklasse="intern"),
    "projekte/angebot-qkx3.md": notiz("Angebot QKX3", "Angebot QKX3 Angebot mit Rabatt.", datenklasse="intern",
                                      kunde="k-nord"),
    "sonst/wetter.md": notiz("Wetter", "Sonne und Regen im Herbst.", datenklasse="intern"),
}
fall("t-intern-mit-kunde", "Eine Notiz der Klasse intern mit Kunde gilt als Kundendaten und kommt nicht in die Cloud",
     [S("Angebot")], notes=n_angebot, verboten=["qkx3"])

n_sued = {
    "kunden/sued/plan-qkx4.md": notiz("Plan QKX4", "Wartungsplan QKX4 Wartungsplan.", datenklasse="kundendaten",
                                      kunde="k-sued"),
    "projekte/wartungsplan.md": notiz("Wartungsplan", "Der Wartungsplan der Halle.", datenklasse="intern"),
}
fall("t-kunde-ohne-eintrag", "Kunde ohne Eintrag in den Einstellungen bekommt auch lokal keine Route",
     [S("Wartungsplan")], route="lokal", notes=n_sued, verboten=["qkx4"])

n_klasse = {
    "a/geheim-qkx6.md": notiz("Bauplan QKX6", "Bauplan QKX6 Bauplan.", datenklasse="geheim"),
    "a/liste-qkx7.md": notiz("Bauplan QKX7", "Bauplan QKX7 als Liste.", datenklasse=["intern"]),
    "a/bauplan.md": notiz("Bauplan", "Der Bauplan der Halle.", datenklasse="intern"),
}
fall("t-klasse-ungueltig", "Unbekannte Klasse und eine Klasse, die kein Text ist, sperren auch lokal",
     [S("Bauplan")], route="lokal", notes=n_klasse, verboten=["qkx6", "qkx7"])

n_kunde_liste = {
    "a/notiz-qkx8.md": notiz("Messplan QKX8", "Messplan QKX8 Messplan.", datenklasse="intern", kunde=["k-nord"]),
    "a/messplan.md": notiz("Messplan", "Der Messplan der Halle.", datenklasse="intern"),
}
fall("t-kunde-kein-text", "Ein Feld kunde, das kein Text ist, sperrt; im Protokoll steht kunde null",
     [S("Messplan")], notes=n_kunde_liste, verboten=["qkx8"])

n_nachfolger = {
    "projekte/regel-alt.md": notiz("Regel alt", "Die Regel Torzeit gilt bis Herbst. Torzeit.", datenklasse="intern",
                                   status="superseded", **{"superseded-by": ["[[regel-qkx5]]"]}),
    "kunden/nord/regel-qkx5.md": notiz("Regel QKX5", "Nachfolgetext des Kunden QKX5.", datenklasse="kundendaten",
                                       kunde="k-nord"),
    "sonst/wetter.md": notiz("Wetter", "Sonne und Regen im Herbst.", datenklasse="intern"),
}
fall("t-nachfolger-gesperrt", "Ein gesperrter Nachfolger erscheint weder als Treffer noch in abgeloest_durch",
     [S("Torzeit")], notes=n_nachfolger, verboten=["qkx5"])

fall("t-widerspruch-gesperrt", "Ein Widerspruch nur mit einer gesperrten Notiz wird nicht angezeigt",
     [S("Lieferplan")], extra={"_meta/state/contradictions.json": json.dumps({"findings": {"w1": {
         "status": "open", "note_a": {"rel": "projekte/lieferplan.md"},
         "note_b": {"rel": "kunden/nord/lieferplan-qkx1.md"}}}})}, verboten=["qkx1", "qkx2"])

fall("z-klasse-geaendert", "Die Klasse gilt im Augenblick der Anfrage: nach dem Umklassieren kommt die Notiz an", [
    S("Lieferplan", id=1),
    {"write": {"kunden/nord/lieferplan-qkx1.md": notiz(
        "Lieferplan QKX1", "Lieferplan QKX1: Lieferplan und Preise des Kunden. Lieferplan QKX1 bleibt intern.",
        datenklasse="oeffentlich")}},
    S("Lieferplan", id=2)], verboten=["qkx2"])

fall("x-protokoll-gesperrt", "Lässt sich die Protokollzeile nicht schreiben, kommt ein Fehler und kein Treffer", [
    {"chmod": {"protokoll": "000"}},
    S("Lieferplan", id=1),
    S("Wetter", id=2)], verboten=["qkx1", "qkx2"])

fall("v-zwei-verbindungen", "Eine zweite Verbindung zum selben Socket hat dieselbe Route", [
    S("Lieferplan", id=1), {"verbindung": "neu"}, S("Lieferplan", id=2)], verboten=["qkx1", "qkx2"])

fall("e-einstellungen-fehlen", "Fehlt die Einstellungsdatei, startet der Prozess nicht", [], start={"exit": 78},
     einstellungen=None)
fall("e-einstellungen-ungueltig", "Ungültige Einstellungsdatei: kein Start", [], start={"exit": 78},
     einstellungen=None, einstellungen_roh="{")
fall("e-route-unbekannt", "Route nicht in den Einstellungen: kein Start", [], start={"exit": 78},
     argv=args("satellit"))
fall("e-route-form", "Route nicht in Schlüsselform: Aufruffehler", [], start={"exit": 64}, argv=args("Cloud"))
fall("e-sitzung-fehlt", "Ohne --sitzung: Aufruffehler", [], start={"exit": 64}, argv=args(**{"--sitzung": FEHLT}))
fall("e-kurzform", "Unbekannte Option: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--rout", "cloud"])
fall("e-protokoll-fehlt", "Protokollordner fehlt: kein Start", [], start={"exit": 78}, protokoll_ordner=False)
fall("v-socket-rechte-0666", "Mit --socket-rechte 0666 hat der Socket die Rechte 0666 (eigene Kennung des Agenten)", [
    {"request": {"op": "ping"}, "antwort": {"ok": True}}], argv=args() + ["--socket-rechte", "0666"], rechte="0666")
fall("e-socket-rechte-ungueltig", "--socket-rechte kennt nur 0600 und 0666", [], start={"exit": 64},
     argv=args() + ["--socket-rechte", "0644"])
fall("e-socket-vorhanden", "Am Socketpfad liegt schon eine Datei: kein Start, die Datei bleibt unverändert", [],
     start={"exit": 74}, vorher={"s.sock": "fremd\n"})


n_pumpe = {
    "a/pumpe-mi1a.md": notiz("Pumpe MI-1A", "Die Pumpe MI-1A läuft im Keller.", datenklasse="intern"),
    "k/pumpe-mk1b.md": notiz("Pumpe MK-1B", "Pumpe MK-1B des Kunden, Pumpe MK-1B.", datenklasse="kundendaten", kunde="k-nord"),
}
MARKER_ARGS = ["--probelauf", "--marker", "MI-1A,MK-1B", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"]
fall("v-marker-cloud", "Zählung der Marker: auf cloud kommt MI-1A an, MK-1B nie; ping und ungültige Anfragen zählen nicht",
     [{"request": {"op": "ping"}, "antwort": {"ok": True}}, S("Pumpe", id=1), S("Pumpe", k=1),
      {"request": {"op": "search", "query": "Pumpe", "route": "lokal"}, "antwort": BAD}],
     notes=n_pumpe, verboten=["MK-1B"], argv=args() + MARKER_ARGS, marker=["MI-1A", "MK-1B"], dirs=["zaehl"])
fall("v-marker-lokal", "Zählung der Marker auf lokal: beide Notizen kommen an, beide Marker zählen",
     [S("Pumpe")], route="lokal", notes=n_pumpe, argv=args("lokal") + MARKER_ARGS, marker=["MI-1A", "MK-1B"],
     dirs=["zaehl"])
fall("x-zaehlung-gesperrt", "Lässt sich die Zähldatei nicht schreiben, kommt ein Fehler und kein Treffer",
     [{"chmod": {"zaehl": "000"}}, S("Pumpe", id=1)], notes=n_pumpe, verboten=["MK-1B", "MI-1A"],
     argv=args() + MARKER_ARGS, marker=["MI-1A", "MK-1B"], dirs=["zaehl"])
fall("e-marker-ohne-datei", "--marker ohne --marker-zaehlung: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--probelauf", "--marker", "MI-1A"])
fall("e-zaehlung-ohne-marker", "--marker-zaehlung ohne --marker: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--probelauf", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"], dirs=["zaehl"])
fall("e-marker-form", "Ein Marker mit Kleinbuchstaben oder ein leerer Eintrag: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--probelauf", "--marker", "MI-1A,,mk-1b", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"],
     dirs=["zaehl"])
fall("e-marker-ohne-probelauf", "--marker und --marker-zaehlung ohne --probelauf: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--marker", "MI-1A", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"], dirs=["zaehl"])
fall("e-probelauf-allein", "--probelauf ohne die Zähloptionen: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--probelauf"])
fall("e-zaehlung-ordner-fehlt", "Der Ordner der Zähldatei fehlt: kein Start", [], start={"exit": 78},
     argv=args() + ["--probelauf", "--marker", "MI-1A", "--marker-zaehlung", "{root}/fehlt/zaehlung.json"])


def bauen() -> None:
    for sub in ("faelle", "erwartet"):
        (HIER / "sichtbar" / sub).mkdir(parents=True, exist_ok=True)
    for name, f in FAELLE.items():
        notes = {rel: json.loads(json.dumps(n)) for rel, n in f["notes"].items()}
        files = {"vault/" + rel: text_von(n) for rel, n in notes.items()}
        files.update({"vault/" + rel: t for rel, t in f["extra"].items()})
        files.update(f["vorher"])
        if f["einstellungen"] is not None:
            files["aux/einstellungen.json"] = json.dumps(f["einstellungen"], ensure_ascii=False, indent=1) + "\n"
        if f["einstellungen_roh"] is not None:
            files["aux/einstellungen.json"] = f["einstellungen_roh"]
        steps, antworten, protokoll = [], [], []
        schreibbar = True
        zaehl = {m: 0 for m in (f["marker"] or [])}
        zaehl_antworten = 0
        if f["marker"]:
            protokoll.append({"aktionsname": "probelauf.zaehlung", "anzahl": len(f["marker"]), "ergebnis": "an",
                              "fassung": 2, "programm": "suche", "sitzung": SITZUNG})
        zaehl_schreibbar = True
        for st in f["steps"]:
            if "write" in st:
                neu = {}
                for rel, n in st["write"].items():
                    notes[rel] = n
                    neu["vault/" + rel] = text_von(n)
                steps.append({"write": neu})
            elif "chmod" in st:
                if "protokoll" in st["chmod"]:
                    schreibbar = False
                if "zaehl" in st["chmod"]:
                    zaehl_schreibbar = False
                steps.append(st)
            elif "verbindung" in st:
                steps.append(st)
            elif "antwort" in st:
                steps.append({k: v for k, v in st.items() if k != "antwort"})
                antworten.append(st["antwort"])
            else:
                steps.append(st)
                a, z, volle = suche(notes, f["extra"], f["route"], st["request"], schreibbar)
                if f["marker"] and not zaehl_schreibbar:
                    a, z, volle = ({"error": "protokoll", **({"id": st["request"]["id"]} if "id" in st["request"] else {})}, [], [])
                if f["marker"] and zaehl_schreibbar:
                    zaehl_antworten += 1
                    for m in zaehl:
                        zaehl[m] += zaehle(volle, m)
                antworten.append(a)
                protokoll += z
        fall_json = {"description": f["description"], "files": files, "dirs": ["home"] +
                     (["protokoll"] if f["protokoll_ordner"] else []) + f["dirs"], "args": f["argv"],
                     "steps": steps, "verboten": f["verboten"]}
        erwartet = {"start": f["start"]}
        if f["start"] == "bereit":
            erwartet.update({"socket_rechte": f["rechte"], "responses": antworten, "protokoll": protokoll,
                             "ende": {"exit": 0}})
            if f["marker"]:
                erwartet["stderr_zeile"] = "such.probelauf: Marker werden gezählt"
                erwartet["marker_zaehlung"] = {"datei": "zaehl/zaehlung.json", "inhalt": {
                    "antworten": zaehl_antworten, "marker": {m: zaehl[m] for m in f["marker"]}, "sitzung": SITZUNG}}
        (HIER / "sichtbar" / "faelle" / f"{name}.json").write_text(
            json.dumps(fall_json, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "sichtbar" / "erwartet" / f"{name}.json").write_text(
            json.dumps(erwartet, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        # Alles außer der Zähldatei: Sie nennt die Marker absichtlich als Schlüssel.
        ohne_zaehlung = {k: v for k, v in erwartet.items() if k != "marker_zaehlung"}
        for t in (json.dumps(ohne_zaehlung, ensure_ascii=False).lower(),
                  json.dumps(antworten, ensure_ascii=False).lower()):
            for m in f["verboten"]:
                assert m.lower() not in t, (name, m)
    print(f"{len(FAELLE)} Fälle gebaut")


if __name__ == "__main__":
    ref_dir = Path(sys.argv[1]).resolve()
    assert os.environ.get("REF_SICHER") == "1", "REF_SICHER=1 fehlt"
    sys.path.insert(0, str(ref_dir))
    import rangkern_ref as REF  # noqa: E402
    globals()["REF"] = REF
    bauen()
