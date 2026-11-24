#!/usr/bin/env python3
"""Baut die sichtbaren Fälle von Vertrag 4 (Such-Socket je Sitzung): `sichtbar/faelle` und `sichtbar/erwartet`.

Die Rangliste ohne Tor kommt aus der Referenz des Vertrags `rangkern` (Python-Vorbild, sicherer Starter). Das Tor
rechnet dieses Skript nach Vertrag 3 (`darf`, Gründe in fester Reihenfolge, Stand 20.11.2026). Ändert sich Vertrag 3,
wird `darf` unten angepasst und der Satz neu gebaut.

Aufruf (eine Python-Umgebung, in der `braincli` importierbar ist):
    REF_SICHER=1 <python> -I -B bauen.py <ordner mit rangkern_ref.py> [--pruefe]
Mit `--pruefe` baut es im Speicher und vergleicht Byte für Byte mit `sichtbar/`; es ändert nichts. Exit 1 bei Abweichung.

Die Fälle `r-*` (Rückfall der Einbettung, README) tragen je Suche die Angabe `_emb`, die nicht in den Fall kommt: `an`
(die Attrappe liefert den Vektor [1.0, 0.0]; die Liste kommt aus der Referenz mit `--vectors`, hybrid), `leer` (keine Notiz
der Datenbank steht im Korpus; Referenz mit `--vectors`, Ollama wird nicht gefragt) oder ein Grund (`keine_verbindung`,
`frist`, `antwort_ungueltig`: lexikalisch, dazu die Zeile `suche.rueckfall` nach den Zeilen „Ausgabe gesperrt“, wenn die
Antwort keinen Fehler trägt und das Protokoll schreibbar ist).
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
# Rückfall der Einbettung: der Vektor der Attrappe für jede Frage, und die drei Gründe (Vertrag 4).
ANFRAGEVEKTOR = [1.0, 0.0]
GRUENDE = ("keine_verbindung", "frist", "antwort_ungueltig")


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


def liste_ohne_tor(notes: dict, extra: dict, frage: str, vektoren: dict | None = None) -> dict:
    with tempfile.TemporaryDirectory(prefix="ss-bau-") as d:
        v = Path(d) / "vault"
        mit = []
        if vektoren is not None:
            datei = Path(d) / "vektoren.json"
            datei.write_text(json.dumps({"query": ANFRAGEVEKTOR, "notes": vektoren}), encoding="utf-8")
            mit = ["--vectors", str(datei)]
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
            code = REF.main(["search", str(v), "-k", "50", "--today", HEUTE] + mit + ["--", frage])
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


def suche(notes, extra, route, req, protokoll_schreibbar, vektoren=None) -> tuple[dict, list[dict]]:
    k = req.get("k", 5)
    roh = liste_ohne_tor(notes, extra, req["query"], vektoren)
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
            # README „Suche mit Tor“, Punkt 5: `status` bleibt, auch wenn das Tor `abgeloest_durch` kürzt (seit 23.11.2026).
            pruef["status"] = h.get("status")
            if rest:
                voll["abgeloest_durch"] = rest
            else:
                voll.pop("abgeloest_durch")
        if h.get("contradiction"):
            partner = [b for a, b in paare if a == h["rel"]] + [a for a, b in paare if b == h["rel"]]
            if partner and not any(p in notes and urteil(p)[0] for p in partner):
                pruef["contradiction"] = None
                voll.pop("contradiction")
            elif partner:
                # mindestens ein erlaubter Partner: das Feld bleibt unverändert (seit 23.11.2026)
                pruef["contradiction"] = h["contradiction"]
        if pruef:
            felder[h["rel"]] = pruef
        volle.append(voll)
    if gesperrt and not protokoll_schreibbar:
        # Klarstellung 5: Die Zeile scheitert nach der Zählung. Gezählt wird `volle`, die Inhaltsantwort, die ohne den
        # Fehler gesendet worden wäre (bis 23.11.2026 gab der Bauer hier [] zurück und zählte 0, W4).
        antwort = {"error": "protokoll"}
        if "id" in req:
            antwort["id"] = req["id"]
        return antwort, [], volle
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
         marker=None, dirs=(), dbs=None, einbettung=None, einbettung_aufrufe=None, stderr_leer=False, protokoll_schreibbar=True,
         **mehr):
    # protokoll_schreibbar=False: Der Prozess kann von Anfang an keine Zeile anhängen (nur für die Erwartung, nicht im JSON).
    # mehr: Felder für den Läufer, unverändert in den Fall (`env_extra`, `vorher_rechte`, `ende_signal`) oder in die
    # Erwartung (`nachher_fehlt`); siehe Kopf von pruefe.py.
    assert set(mehr) <= {"env_extra", "vorher_rechte", "ende_signal", "nachher_fehlt"}, sorted(mehr)
    FAELLE[name] = dict(mehr=mehr, description=description, steps=steps, route=route,
                        notes=grund_vault() if notes is None else notes, extra=extra or {}, verboten=list(verboten),
                        start=start, argv=argv or args(route), einstellungen=einstellungen,
                        einstellungen_roh=einstellungen_roh, vorher=vorher or {}, protokoll_ordner=protokoll_ordner,
                        rechte=rechte, marker=marker, dirs=list(dirs), dbs=dbs, einbettung=einbettung,
                        einbettung_aufrufe=einbettung_aufrufe, stderr_leer=stderr_leer,
                        protokoll_schreibbar=protokoll_schreibbar)


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


# --- Fälle vom 23.11.2026 (Entwürfe der Vertragsprüfung) ----------------------------------------------------------
# Herkunft: Entwürfe der Vertragsprüfung vom 23.11.2026, Teil A und C. Soll nach dem Vertrag und den
# Entscheiden vom 23.11.2026 (W3: Zähldatei vor Socket, 78 vor 74).
ZAEHL_ARGS = ["--probelauf", "--marker", "MI-1A", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"]

# Reihenfolge der Startprüfungen (README „Aufruf“, Tabelle)
fall("e-reihenfolge-64-vor-78-und-74", "Aufruffehler, fehlende Einstellungen und belegter Socketpfad zugleich: Exit 64",
     [], start={"exit": 64}, einstellungen=None, vorher={"s.sock": "fremd\n"}, argv=args("Cloud"))
fall("e-reihenfolge-78-vor-74", "Fehlende Einstellungen und belegter Socketpfad zugleich: Exit 78, Datei bleibt", [],
     start={"exit": 78}, einstellungen=None, vorher={"s.sock": "fremd\n"})
fall("e-reihenfolge-zaehlung-vor-socket", "Ordner der Zähldatei fehlt und Socketpfad belegt: Exit 78 (Zähldatei vor dem "
     "Socket, W3)", [], start={"exit": 78}, vorher={"s.sock": "fremd\n"},
     argv=args() + ["--probelauf", "--marker", "MI-1A", "--marker-zaehlung", "{root}/fehlt/zaehlung.json"])
fall("e-zaehlung-weg-nach-74", "Probelauf und belegter Socketpfad: Exit 74, die Datei bleibt, eine angelegte Zähldatei "
     "ist wieder weg", [], start={"exit": 74}, vorher={"s.sock": "fremd\n"}, argv=args() + ZAEHL_ARGS, dirs=["zaehl"],
     nachher_fehlt=["zaehl/zaehlung.json"])
fall("e-probelauf-zeile-scheitert", "Probelauf, Protokollordner nur lesbar: die Zeile probelauf.zaehlung scheitert, Exit 78, "
     "kein Socket, keine Zeile, keine Zähldatei", [], start={"exit": 78}, argv=args() + ZAEHL_ARGS, dirs=["zaehl"],
     vorher_rechte={"protokoll": "0500"}, nachher_fehlt=["zaehl/zaehlung.json"])

# Aufruffehler 64 (Tabelle, Klarstellungen 1, 2, 11)
fall("e-option-doppelt-beide-formen", "--route und --route=… zugleich: Aufruffehler", [], start={"exit": 64},
     argv=args() + ["--route=lokal"])
fall("v-optionen-mit-gleich", "Alle Optionen in der Form --name=wert: normaler Start", [S("Lieferplan", id=1)],
     argv=["sitzung", "{root}/vault", "--socket={sock}", "--route=cloud", "--sitzung=" + SITZUNG,
           "--einstellungen={root}/aux/einstellungen.json", "--protokoll={root}/protokoll"], verboten=["qkx1", "qkx2"])
fall("e-kurzform-echt", "Echte Kurzform -r: Aufruffehler", [], start={"exit": 64}, argv=args() + ["-r", "cloud"])
fall("e-sitzung-form", "--sitzung nicht in Schlüsselform: Aufruffehler", [], start={"exit": 64},
     argv=args(**{"--sitzung": "S 1"}))
fall("e-vault-fehlt", "Der Vault fehlt: Aufruffehler", [], start={"exit": 64},
     argv=["sitzung"] + args()[2:])
for _opt in ("--socket", "--einstellungen", "--protokoll", "--route", "--sitzung"):
    fall(f"e-wert-leer{_opt[1:]}", f"{_opt} mit leerem Wert: 64 vor 78 und 74, die Datei am Socketpfad bleibt", [],
         start={"exit": 64}, einstellungen=None, vorher={"s.sock": "fremd\n"}, argv=args(**{_opt: ""}))
fall("e-marker-doppelt", "Ein Marker zweimal: Aufruffehler", [], start={"exit": 64}, dirs=["zaehl"],
     argv=args() + ["--probelauf", "--marker", "MI-1A,MI-1A", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"])
fall("e-marker-siebzehn", "17 Marker: Aufruffehler", [], start={"exit": 64}, dirs=["zaehl"],
     argv=args() + ["--probelauf", "--marker", ",".join(f"M-{i}" for i in range(17)),
                    "--marker-zaehlung", "{root}/zaehl/zaehlung.json"])
fall("e-marker-33-zeichen", "Ein Marker mit 33 Zeichen: Aufruffehler", [], start={"exit": 64}, dirs=["zaehl"],
     argv=args() + ["--probelauf", "--marker", "M" * 33, "--marker-zaehlung", "{root}/zaehl/zaehlung.json"])
fall("e-unterbefehl-unbekannt", "Unbekannter Unterbefehl: Aufruffehler 64 (seit 23.11.2026 für jeden Befehl)", [],
     start={"exit": 64}, argv=["tor", "{root}/vault"])
# 78 (Tabelle)
fall("e-protokoll-ist-datei", "--protokoll nennt eine Datei: kein Start", [], start={"exit": 78},
     argv=args(**{"--protokoll": "{root}/aux/einstellungen.json"}))

# Anfrageform (Felder, Klarstellungen 3 und 4, Zeilenregeln wie rangkern serve)
fall("p-felder-form", "query fehlt oder ist kein Text, space kein Text, ping mit query, id ungültig", [
    {"request": {"op": "search", "id": 1}, "antwort": dict(BAD, id=1)},
    {"request": {"op": "search", "query": 5, "id": 2}, "antwort": dict(BAD, id=2)},
    {"request": {"op": "search", "query": "Lieferplan", "space": 5, "id": 3}, "antwort": dict(BAD, id=3)},
    {"request": {"op": "ping", "query": "x", "id": 4}, "antwort": dict(BAD, id=4)},
    {"request": {"op": "ping", "id": 1.5}, "antwort": BAD},
    {"request": {"op": "ping", "id": True}, "antwort": BAD},
    {"request": {"op": "ping", "id": None}, "antwort": BAD},
    S("Lieferplan", space=None, id=5)], verboten=["qkx1", "qkx2"])
fall("p-zeile-ueber-1-mib", "Eine Zeile über 1 MiB: bad_request ohne id; danach antwortet die Verbindung weiter", [
    {"line_gross": {"anfang": "{\"id\":1,\"op\":\"ping\",\"x\":\"", "zeichen": "a", "anzahl": 1 << 20, "ende": "\"}"},
     "antwort": BAD},
    {"request": {"op": "ping", "id": 2}, "antwort": {"ok": True, "id": 2}}])
fall("p-einsames-surrogat", "Escape \\ud800 ohne zweite Hälfte: bad_request ohne id; ein gültiges Paar geht", [
    {"line": "{\"op\":\"ping\",\"id\":\"\\ud800\"}", "antwort": BAD},
    {"line": "{\"op\":\"ping\",\"id\":\"\\ud83d\\ude00\"}", "antwort": {"ok": True, "id": "\U0001F600"}}])
fall("p-utf8-ungueltig-und-halbe-zeile", "Eine Zeile mit dem Byte ff ist ungültig (ohne id); eine letzte Zeile ohne LF gilt "
     "als Anfrage (wie rangkern serve)", [
    {"line_hex": "7b226f70223a2270696e67222c226964223a22ff227d", "antwort": BAD},
    {"request": {"op": "ping", "id": 2}, "antwort": {"ok": True, "id": 2}},
    {"line": "{\"op\":\"ping\",\"id\":3}", "ohne_lf": True, "antwort": {"ok": True, "id": 3}}])
fall("p-pipeline", "Drei Anfragen in einem Stück: die Antworten kommen in der Reihenfolge der Anfragen", [
    {"zeilen": [{"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}}, S("Lieferplan", id=2),
                {"request": {"op": "ping", "id": 3}, "antwort": {"ok": True, "id": 3}}]}], verboten=["qkx1", "qkx2"])

# Tor: Eingaben aus dem Frontmatter
n_feldwerte = {
    "a/messplan.md": notiz("Messplan", "Der Messplan der Halle.", datenklasse="intern"),
    "a/null-qkx31.md": notiz("Messplan QKX31", "Messplan QKX31 Messplan.", datenklasse=None),
    "a/leer-qkx32.md": notiz("Messplan QKX32", "Messplan QKX32 Messplan.", datenklasse=""),
    "a/form-qkx33.md": notiz("Messplan QKX33", "Messplan QKX33 Messplan.", datenklasse="intern", kunde="K Nord"),
    "a/ohne-kunde.md": notiz("Messplan ohne Kunde", "Messplan, Feld kunde ohne Wert.", datenklasse="intern", kunde=None),
}
fall("t-feldwerte-lokal", "Lokal: datenklasse null und leer, kunde null und ohne Schlüsselform", [S("Messplan", k=10)],
     route="lokal", notes=n_feldwerte, verboten=["qkx31", "qkx32", "qkx33"])
n_class = {
    "a/plan-qkx34.md": notiz("Bauplan QKX34", "Bauplan QKX34 Bauplan.", datenklasse="kundendaten", kunde="k-nord",
                             **{"class": "knowledge"}),
    "a/bauplan.md": notiz("Bauplan", "Der Bauplan der Halle.", datenklasse="intern", **{"class": "source"}),
}
fall("t-class-feld", "class: knowledge gibt einer Kundennotiz auf cloud nichts frei", [S("Bauplan")], notes=n_class,
     verboten=["qkx34"])
n_nachfolger_gemischt = {
    "projekte/regel-alt.md": notiz("Regel alt", "Die Regel Torzeit gilt bis Herbst. Torzeit.", datenklasse="intern",
                                   status="superseded",
                                   **{"superseded-by": ["[[regel-qkx35]]", "[[regel-neu]]"]}),
    "kunden/nord/regel-qkx35.md": notiz("Regel QKX35", "Nachfolgetext QKX35.", datenklasse="kundendaten", kunde="k-nord"),
    "projekte/regel-neu.md": notiz("Regel neu", "Die neue Regel.", datenklasse="intern"),
}
fall("t-nachfolger-gemischt", "Ein gesperrter und ein erlaubter Nachfolger: nur der erlaubte steht in abgeloest_durch, "
     "status bleibt", [S("Torzeit")], notes=n_nachfolger_gemischt, verboten=["qkx35"])
fall("t-widerspruch-gemischt", "Ein Widerspruch mit gesperrter und einer mit erlaubter Partnernotiz: contradiction bleibt",
     [S("Lieferplan")], extra={"_meta/state/contradictions.json": json.dumps({"findings": {
         "w1": {"status": "open", "note_a": {"rel": "projekte/lieferplan.md"},
                "note_b": {"rel": "kunden/nord/lieferplan-qkx1.md"}},
         "w2": {"status": "open", "note_a": {"rel": "projekte/lieferplan.md"},
                "note_b": {"rel": "handbuch/lieferplan-lesen.md"}}}})}, verboten=["qkx1", "qkx2"])

# Änderung zwischen Anfragen, Einstellungen einmal beim Start
fall("z-klasse-verschaerft", "Nach dem Umklassieren auf kundendaten kommt eine vorher erlaubte Notiz nicht mehr an", [
    S("Lieferplan", id=1),
    {"write": {"projekte/lieferplan.md": notiz("Lieferplan Lager", "Der Lieferplan regelt die Termine im Lager.",
                                               datenklasse="kundendaten", kunde="k-nord")}},
    S("Lieferplan", id=2)], verboten=["qkx1", "qkx2"])
_offener = json.loads(json.dumps(EINSTELLUNGEN))
_offener["routen"][0]["klasse"] = "kundendaten"
fall("z-einstellungen-nach-start", "Die Einstellungen gelten aus dem Start: eine spätere Änderung, die cloud öffnet, wirkt "
     "nicht", [S("Lieferplan", id=1),
               {"write_datei": {"aux/einstellungen.json": json.dumps(_offener, ensure_ascii=False, indent=1) + "\n"}},
               S("Lieferplan", id=2)], verboten=["qkx1", "qkx2"])

# Zählung (Klarstellungen 5 und 6)
n_mm = {"a/pumpe-mmm.md": notiz("Pumpe MMM", "Pumpe MMMM im Keller.", datenklasse="intern")}
fall("v-marker-ueberlappend", "Marker MM in MMM und MMMM: von links ohne Überlappung gezählt", [S("Pumpe")],
     notes=n_mm, argv=args() + ["--probelauf", "--marker", "MM", "--marker-zaehlung", "{root}/zaehl/zaehlung.json"],
     marker=["MM"], dirs=["zaehl"])
fall("x-zaehlung-dann-protokoll-gesperrt", "Klarstellung 5: Zählung gelingt, die Zeile „Ausgabe gesperrt“ scheitert: "
     "Antwort protokoll, gezählt ist die Inhaltsantwort", [{"chmod": {"protokoll": "000"}}, S("Pumpe", id=1)],
     notes=n_pumpe, verboten=["MK-1B"], argv=args() + MARKER_ARGS, marker=["MI-1A", "MK-1B"], dirs=["zaehl"])

# Verbindungen, Ende, Umgebung
fall("v-siebzehnte-verbindung", "Klarstellung 13: die 17. gleichzeitige Verbindung wird ohne Byte geschlossen, danach ist "
     "wieder Platz", [{"grenze": 16}, {"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}}])
fall("x-leerlauf-schliesst", "Klarstellung 13: eine Verbindung ohne Byte wird nach 10 s ohne Antwort geschlossen, gezählt ab dem "
     "letzten Byte; danach nimmt der Prozess eine neue Verbindung an",
     [{"leerlauf": 10}, {"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}}])
fall("e-sigint", "SIGINT beendet den Prozess wie SIGTERM: Exit 0, Socket weg",
     [{"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}}], ende_signal="SIGINT")
fall("e-umgebung-ohne-wirkung", "Der Prozess liest keine Umgebungsvariable: Variablen für Route und Einbettung ändern "
     "nichts", [S("Lieferplan", id=1)], verboten=["qkx1", "qkx2"],
     env_extra={"ROUTE": "lokal", "BRAIN_ROUTE": "lokal", "RANGKERN_ROUTE": "lokal", "OLLAMA_HOST": "127.0.0.1:1"})


# --- Rückfall der Einbettung (README, Abschnitt „Rückfall der Einbettung“) -------------------------------------------
INDEX = "aux/index.db"
VEKTOR_TEXT = "[1.0, 0.0]"
DB_KORPUS = {INDEX: [[rel, VEKTOR_TEXT] for rel in grund_vault()]}
DB_RELIKT = {INDEX: [["weg/verschwunden.md", VEKTOR_TEXT]]}
EMB_ARGS = ["--index-db", "{root}/" + INDEX, "--embed", "{einbettung}", "--embed-timeout-ms", "300"]
LANG_MS = 2500  # länger als die Frist eines Versuchs (300 ms), kürzer als die Frist des Läufers je Antwort


def E(query, emb, **f):
    """Eine Suche mit der Angabe, wie die Einbettung für sie ausgeht (Kopf: `_emb`)."""
    return {"request": {"op": "search", "query": query, **f}, "_emb": emb}


fall("r-an", "Einbettung an: hybride Antwort mit fallback false, keine Zeile suche.rueckfall",
     [E("Wetter", "an", id=1), E("Lieferplan", "an", id=2)], dbs=DB_KORPUS, einbettung={},
     argv=args() + EMB_ARGS, verboten=["qkx1", "qkx2"], einbettung_aufrufe=2)
fall("r-aus", "Einbettung aus: je Suche eine Zeile mit keine_verbindung; ping und Fehlerantworten schreiben keine", [
    {"request": {"op": "ping", "id": 1}, "antwort": {"ok": True, "id": 1}},
    E("Wetter", "keine_verbindung", id=2),
    {"request": {"op": "search", "query": "Wetter", "route": "cloud", "id": 3}, "antwort": dict(BAD, id=3)},
    {"line": "kein json", "antwort": BAD},
    E("Wetter", "keine_verbindung", id=4)],
    dbs=DB_KORPUS, einbettung={"aus": True}, argv=args() + EMB_ARGS, einbettung_aufrufe=0, stderr_leer=True)
fall("r-frist", "Die Antwort kommt nach der Frist jedes Versuchs: Zeile mit frist, zwei Versuche",
     [E("Wetter", "frist", id=1)], dbs=DB_KORPUS, einbettung={"aufrufe": [{"verzoegerung_ms": LANG_MS}]},
     argv=args() + EMB_ARGS, einbettung_aufrufe=2)
fall("r-kaputt", "Kaputtes JSON in beiden Versuchen: Zeile mit antwort_ungueltig",
     [E("Wetter", "antwort_ungueltig", id=1)], dbs=DB_KORPUS, einbettung={"aufrufe": [{"koerper": "kaputt"}]},
     argv=args() + EMB_ARGS, einbettung_aufrufe=2)
fall("r-wieder-an", "Einbettung an, aus, wieder an: nur die Suche dazwischen schreibt eine Zeile", [
    E("Wetter", "an", id=1), {"einbettung_aus": True}, E("Wetter", "keine_verbindung", id=2),
    {"einbettung_an": True}, E("Wetter", "an", id=3)],
    dbs=DB_KORPUS, einbettung={}, argv=args() + EMB_ARGS, einbettung_aufrufe=2)
fall("r-frist-dann-abgewiesen", "Erster Versuch läuft in die Frist, der zweite wird abgewiesen: der letzte Versuch zählt",
     [E("Wetter", "keine_verbindung", id=1)], dbs=DB_KORPUS,
     einbettung={"aufrufe": [{"verzoegerung_ms": LANG_MS, "danach_aus": True}]}, argv=args() + EMB_ARGS,
     einbettung_aufrufe=1)
fall("r-ohne-embed", "Ohne --embed ist die Suche lexikalisch per Einstellung: fallback true, keine Zeile",
     [E("Wetter", None, id=1)], dbs=DB_KORPUS, argv=args() + ["--index-db", "{root}/" + INDEX])
fall("r-relikt", "Keine Notiz der Datenbank steht im Korpus: Ollama wird nicht gefragt, kein Rückfall, keine Zeile",
     [E("Wetter", "leer", id=1)], dbs=DB_RELIKT, einbettung={"aus": True}, argv=args() + EMB_ARGS,
     einbettung_aufrufe=0)
fall("r-sperre-und-rueckfall", "Gesperrter Treffer und Rückfall in einer Antwort: erst „Ausgabe gesperrt“, dann suche.rueckfall",
     [E("Lieferplan", "keine_verbindung", id=1)], dbs=DB_KORPUS, einbettung={"aus": True}, argv=args() + EMB_ARGS,
     verboten=["qkx1", "qkx2"], einbettung_aufrufe=0)
fall("r-protokoll-gesperrt", "Protokollordner nach dem Start nicht beschreibbar: Antwort gleich, keine Zeile, stderr leer; "
     "eine Sperre ergibt weiter {\"error\": \"protokoll\"}", [
    {"chmod": {"protokoll": "000"}}, E("Wetter", "keine_verbindung", id=1), E("Lieferplan", "keine_verbindung", id=2)],
    dbs=DB_KORPUS, einbettung={"aus": True}, argv=args() + EMB_ARGS, verboten=["qkx1", "qkx2"], einbettung_aufrufe=0,
    stderr_leer=True)
fall("r-vorgabe-ohne-lf", "Eine vorgegebene Monatsdatei endet ohne LF (Vertrag 5, 5.3: letzte_zeile_unvollstaendig): "
     "eine Sperre ergibt {\"error\": \"protokoll\"}, die Rückfallzeile fällt ohne Einfluss weg, die Datei bleibt unverändert", [
    E("Lieferplan", "keine_verbindung", id=1), E("Wetter", "keine_verbindung", id=2)],
    dbs=DB_KORPUS, einbettung={"aus": True}, argv=args() + EMB_ARGS, verboten=["qkx1", "qkx2"], einbettung_aufrufe=0,
    vorher={"protokoll/brain-2000-01.jsonl": '{"aktionsname":"vorgabe"}'}, protokoll_schreibbar=False)


def vektoren_von(dbs: dict | None) -> dict:
    return {rel: json.loads(text) for rel, text in (dbs or {}).get(INDEX, [])}


def dateien_bauen() -> dict[Path, str]:
    """Alle Dateien des Satzes als {Pfad: Text}, ohne zu schreiben."""
    dateien: dict[Path, str] = {}
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
        schreibbar = f["protokoll_schreibbar"]
        zaehl = {m: 0 for m in (f["marker"] or [])}
        zaehl_antworten = 0
        if f["marker"]:
            protokoll.append({"aktionsname": "probelauf.zaehlung", "anzahl": len(f["marker"]), "ergebnis": "an",
                              "fassung": 2, "programm": "suche", "sitzung": SITZUNG})
        zaehl_schreibbar = True

        def antwort_bauen(st) -> dict:
            """Eine Anfrage: Soll der Antwort, Protokollzeilen und Zählung wie bisher (für Schritte und `zeilen`)."""
            nonlocal zaehl_antworten
            if "antwort" in st:
                return st["antwort"]
            a, z, volle = suche(notes, f["extra"], f["route"], st["request"], schreibbar)
            if f["marker"] and not zaehl_schreibbar:
                a, z, volle = ({"error": "protokoll", **({"id": st["request"]["id"]} if "id" in st["request"] else {})}, [], [])
            if f["marker"] and zaehl_schreibbar:
                zaehl_antworten += 1
                for m in zaehl:
                    zaehl[m] += zaehle(volle, m)
            protokoll.extend(z)
            return a

        for st in f["steps"]:
            if "write_datei" in st:
                # eine Datei außerhalb des Vaults (etwa die Einstellungen) nach dem Start ändern
                steps.append({"write": dict(st["write_datei"])})
            elif "zeilen" in st:
                steps.append({"zeilen": [{k: v for k, v in s.items() if k != "antwort"} for s in st["zeilen"]]})
                antworten.extend(antwort_bauen(s) for s in st["zeilen"])
            elif "grenze" in st or "leerlauf" in st:
                steps.append(st)
            elif "write" in st:
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
            elif "verbindung" in st or "einbettung_aus" in st or "einbettung_an" in st:
                steps.append(st)
            elif "antwort" in st:
                steps.append({k: v for k, v in st.items() if k != "antwort"})
                antworten.append(antwort_bauen(st))
            elif "_emb" in st:
                emb = st["_emb"]
                steps.append({k: v for k, v in st.items() if k != "_emb"})
                vek = vektoren_von(f["dbs"]) if emb in ("an", "leer") else None
                a, z, _ = suche(notes, f["extra"], f["route"], st["request"], schreibbar, vek)
                if emb in GRUENDE and "error" not in a and schreibbar:
                    z.append({"aktionsname": "suche.rueckfall", "ergebnis": "lexikalisch", "fassung": 2,
                              "grund": emb, "route": f["route"], "sitzung": SITZUNG})
                antworten.append(a)
                protokoll += z
            else:
                steps.append(st)
                antworten.append(antwort_bauen(st))
        fall_json = {"description": f["description"], "files": files, "dirs": ["home"] +
                     (["protokoll"] if f["protokoll_ordner"] else []) + f["dirs"], "args": f["argv"],
                     "steps": steps, "verboten": f["verboten"]}
        if f["dbs"] is not None:
            fall_json["dbs"] = f["dbs"]
        if f["einbettung"] is not None:
            fall_json["einbettung"] = f["einbettung"]
        for feld in ("env_extra", "vorher_rechte", "ende_signal"):
            if feld in f["mehr"]:
                fall_json[feld] = f["mehr"][feld]
        erwartet = {"start": f["start"]}
        if "nachher_fehlt" in f["mehr"]:
            erwartet["nachher_fehlt"] = f["mehr"]["nachher_fehlt"]
        if f["start"] == "bereit":
            erwartet.update({"socket_rechte": f["rechte"], "responses": antworten, "protokoll": protokoll,
                             "ende": {"exit": 0}})
            if f["marker"]:
                erwartet["stderr_zeile"] = "such.probelauf: Marker werden gezählt"
                erwartet["marker_zaehlung"] = {"datei": "zaehl/zaehlung.json", "inhalt": {
                    "antworten": zaehl_antworten, "marker": {m: zaehl[m] for m in f["marker"]}, "sitzung": SITZUNG}}
            if f["einbettung_aufrufe"] is not None:
                erwartet["einbettung_aufrufe"] = f["einbettung_aufrufe"]
            if f["stderr_leer"]:
                erwartet["stderr_leer"] = True
        dateien[HIER / "sichtbar" / "faelle" / f"{name}.json"] = json.dumps(fall_json, ensure_ascii=False, indent=1) + "\n"
        dateien[HIER / "sichtbar" / "erwartet" / f"{name}.json"] = json.dumps(erwartet, ensure_ascii=False, indent=1) + "\n"
        # Alles außer der Zähldatei: Sie nennt die Marker absichtlich als Schlüssel.
        ohne_zaehlung = {k: v for k, v in erwartet.items() if k != "marker_zaehlung"}
        for t in (json.dumps(ohne_zaehlung, ensure_ascii=False).lower(),
                  json.dumps(antworten, ensure_ascii=False).lower()):
            for m in f["verboten"]:
                assert m.lower() not in t, (name, m)
    return dateien


def main(argv: list[str]) -> int:
    pruefe = "--pruefe" in argv
    rest = [a for a in argv if a != "--pruefe"]
    if len(rest) != 1:
        print("Aufruf: bauen.py <ordner mit rangkern_ref.py> [--pruefe]", file=sys.stderr)
        return 2
    assert os.environ.get("REF_SICHER") == "1", "REF_SICHER=1 fehlt"
    sys.path.insert(0, str(Path(rest[0]).resolve()))
    import rangkern_ref as REF  # noqa: E402
    globals()["REF"] = REF
    if not pruefe:
        bauen()
        return 0
    dateien = dateien_bauen()
    da = {p for sub in ("faelle", "erwartet") for p in (HIER / "sichtbar" / sub).glob("*.json")}
    abweichung = sorted(str(p.relative_to(HIER)) for p in set(dateien) | da
                        if p not in dateien or not p.is_file() or p.read_text(encoding="utf-8") != dateien[p])
    for p in abweichung:
        print(f"abweichend: {p}")
    print(f"{len(FAELLE)} Fälle gebaut, {len(abweichung)} Abweichungen von der Platte")
    return 1 if abweichung else 0


def bauen() -> None:
    """Schreibt den Satz nach `sichtbar/` (wie vor dem 23.11.2026)."""
    dateien = dateien_bauen()
    for sub in ("faelle", "erwartet"):
        (HIER / "sichtbar" / sub).mkdir(parents=True, exist_ok=True)
    for p, text in dateien.items():
        p.write_text(text, encoding="utf-8")
    print(f"{len(FAELLE)} Fälle gebaut")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
