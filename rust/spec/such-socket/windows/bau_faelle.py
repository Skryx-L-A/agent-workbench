#!/usr/bin/env python3
"""Fallbauer für den Nachtrag „Such-Pipe unter Windows“ zu Vertrag 4. Nur Standardbibliothek.

Aufruf: python3 bau_faelle.py            schreibt faelle/, erwartet/ und mcp_faelle.json neu
        python3 bau_faelle.py --pruefe   baut im Speicher, prüft die Form und vergleicht mit der Platte; ändert nichts
Exit 0: Form gut (und bei --pruefe gleich). Exit 1: Formfehler oder Abweichung.

Quellen: ../sichtbar/ (Fälle von Vertrag 4), ../mcp/faelle.json (MCP-Fälle von Vertrag 4) und die Windows-Fälle unten.
Bei gleichem Stand ist die Ausgabe Byte für Byte gleich.
"""
from __future__ import annotations

import copy
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
V4 = HIER.parent

ORTE = {"attrappe", "windows", "vm-konto", "vm-container"}
PEERS = {"agent", "person", "andere-sitzung", "ohne-lauf", "dienst-stumm", "zeit-falsch", "anderer-container",
         "fremdes-konto"}
# Kennungen, die unter Windows einen Client im AppContainer brauchen, und die mit zweitem Konto
PEER_ORT = {"agent": "vm-container", "andere-sitzung": "vm-container", "ohne-lauf": "vm-container",
            "dienst-stumm": "vm-container", "anderer-container": "vm-container", "fremdes-konto": "vm-konto",
            "person": "windows", "zeit-falsch": None}
# nur die Attrappe kann diese Kennung bilden; gegen Windows braucht es erzwungene PID-Wiederverwendung (offener Punkt des Vertrags)
NUR_ATTRAPPE_PEERS = {"zeit-falsch"}
# DACL-Ablehnungen bildet die Attrappe nicht nach: das tut unter Windows der Kern
NICHT_ATTRAPPE_PEERS = {"anderer-container", "fremdes-konto"}
SCHRITTE = {"request", "line", "write", "chmod", "verbindung", "zugleich", "stumm", "sicherheit", "probe"}
# feste Gründe von peerkanal mit Vorsilbe pk. (Entscheidung vom 20.11.2026) und die drei Gründe des Such-Prozesses
GRUENDE = {"pk.hallo_fehlt", "pk.hallo_falsch", "pk.messung_gescheitert", "pk.prozess_nicht_offen", "pk.job_unbekannt",
           "pk.prozess_beendet", "pk.startzeit_weicht_ab", "pk.pidversion_weicht_ab", "pk.eingeschraenkt",
           "pk.sid_nicht_erlaubt", "pk.appcontainer_falsch", "pk.uid_nicht_erlaubt", "pk.gid_falsch",
           "such.dienst_stumm", "such.lauf_unbekannt", "such.sitzung_fremd"}
WEG_V4 = {"v-socket-rechte-0666": "unter Windows ist --socket-rechte ein Aufruffehler (w-e-socket-rechte)",
          "e-socket-vorhanden": "unter Windows ersetzt durch w-e-pipe-belegt"}
PAKET = "{paket_sid}"
SITZUNG = "s-test-1"
ROUTE = "cloud"


def ab(grund: str) -> dict:
    return {"aktionsname": "kanal.abgewiesen", "ergebnis": "abgewiesen", "fassung": 2, "grund": grund, "kanal": "such",
            "route": ROUTE, "sitzung": SITZUNG}


def basis() -> dict:
    """Vault, Einstellungen und Aufruf aus p-ping von Vertrag 4, umgestellt auf die Pipe."""
    fall = json.loads((V4 / "sichtbar/faelle/p-ping.json").read_text(encoding="utf-8"))
    return {"files": fall["files"], "dirs": fall["dirs"], "args": umstellen(fall["args"]), "verboten": []}


def umstellen(args: list[str]) -> list[str]:
    """{sock} wird {pipe}; direkt dahinter kommt --paket-sid."""
    neu = []
    for a in args:
        neu.append("{pipe}" if a == "{sock}" else a)
        if a == "{sock}":
            neu += ["--paket-sid", PAKET]
    if "{pipe}" not in neu:
        neu += ["--paket-sid", PAKET]
    return neu


def ersetze_arg(args: list[str], option: str, wert: str | None) -> list[str]:
    """Setzt den Wert einer Option; None entfernt Option und Wert."""
    i = args.index(option)
    return args[:i] + args[i + 2:] if wert is None else args[:i + 1] + [wert] + args[i + 2:]


# Erweiterungen von ../pruefe.py vom 23.11.2026, die dieser Läufer nicht kennt. Solche Fälle übernimmt der Bauer nicht; sie
# laufen unter macOS über `pruefe_macos.py --vertrag4` direkt aus ../sichtbar (wie r-*).
V4_SCHRITT_GRUNDFORM = {"request", "line", "write", "chmod", "verbindung"}
V4_ERWEITERT = {"env_extra", "vorher_rechte", "ende_signal"}


def nur_v4(fall: dict, erw: dict) -> bool:
    """True, wenn der Fall eine Erweiterung von ../pruefe.py braucht (Schritt, Feld des Falls oder der Erwartung)."""
    if V4_ERWEITERT & set(fall) or "nachher_fehlt" in erw:
        return True
    return any(not set(s) <= V4_SCHRITT_GRUNDFORM | {"antwort"} or s.get("ohne_lf") for s in fall["steps"])


def v4_faelle() -> dict[str, tuple[dict, dict]]:
    aus: dict[str, tuple[dict, dict]] = {}
    for p in sorted((V4 / "sichtbar/faelle").glob("*.json")):
        if p.stem in WEG_V4 or p.stem.startswith("r-") or nur_v4(json.loads(p.read_text(encoding="utf-8")), json.loads(
                (V4 / "sichtbar/erwartet" / p.name).read_text(encoding="utf-8"))):
            # r-*: Rückfall der Einbettung (Attrappe, Datenbank); unter macOS fährt sie pruefe_macos.py --vertrag4
            # direkt aus ../sichtbar, unter Windows schreibt der Such-Prozess die Zeile noch nicht.
            continue
        fall = json.loads(p.read_text(encoding="utf-8"))
        erw = json.loads((V4 / "sichtbar/erwartet" / p.name).read_text(encoding="utf-8"))
        neu = {"beschreibung": fall["description"], "herkunft": f"vertrag4:{p.stem}", "files": fall["files"],
               "dirs": fall["dirs"], "args": umstellen(fall["args"]), "steps": fall["steps"],
               "verboten": fall["verboten"]}
        verbindet = any("request" in s or "line" in s for s in fall["steps"])
        orte = ["attrappe", "vm-container"] if verbindet else ["attrappe", "windows"]
        if any("chmod" in s for s in fall["steps"]):
            orte = ["attrappe"]
            neu["hinweis"] = "chmod; unter Windows über eine ACL des Protokollordners nachzubauen (offen)"
        neu["orte"] = orte
        e = {"start": erw["start"]}
        if erw["start"] == "bereit":
            e |= {"responses": erw["responses"], "protokoll": erw["protokoll"],
                  "ende": {"pipe_weg": True, "exit_attrappe": erw["ende"]["exit"]}}
            for k in ("marker_zaehlung", "stderr_zeile"):
                if k in erw:
                    e[k] = erw[k]
        aus[f"v4-{p.stem}"] = (neu, e)
    return aus


def w_faelle() -> dict[str, tuple[dict, dict]]:
    aus: dict[str, tuple[dict, dict]] = {}
    hex32 = "0123456789abcdef" * 2

    def start(name, beschr, args, exit_, orte=("attrappe", "windows"), **mehr):
        f = basis() | {"beschreibung": beschr, "herkunft": "windows", "orte": list(orte), "args": args, "steps": []}
        f |= mehr
        aus[name] = (f, {"start": {"exit": exit_}})

    def lauf(name, beschr, steps, responses, protokoll, orte, **mehr):
        f = basis() | {"beschreibung": beschr, "herkunft": "windows", "orte": list(orte), "steps": steps}
        e = {"start": "bereit", "responses": responses, "protokoll": protokoll,
             "ende": {"pipe_weg": True, "exit_attrappe": 0}}
        for k in ("sicherheit",):
            if k in mehr:
                e[k] = mehr.pop(k)
        f |= mehr
        aus[name] = (f, e)

    a = basis()["args"]
    start("w-e-paket-sid-fehlt", "Ohne --paket-sid startet der Prozess unter Windows nicht", ersetze_arg(a, "--paket-sid", None), 64)
    start("w-e-paket-sid-nutzer", "Eine Nutzer-SID statt einer AppContainer-SID ist ein Aufruffehler",
          ersetze_arg(a, "--paket-sid", "S-1-5-21-1000000001-1000000002-1000000003-1001"), 64)
    start("w-e-paket-sid-null-vorn", "Eine Zahl mit führender Null in der SID ist ein Aufruffehler",
          ersetze_arg(a, "--paket-sid", "S-1-15-2-01-2-3-4-5-6-7"), 64)
    start("w-e-paket-sid-zu-gross", "Eine Zahl über 4294967295 in der SID ist ein Aufruffehler",
          ersetze_arg(a, "--paket-sid", "S-1-15-2-4294967296-2-3-4-5-6-7"), 64)
    start("w-e-paket-sid-sechs", "Sechs statt sieben Zahlen nach S-1-15-2 sind ein Aufruffehler",
          ersetze_arg(a, "--paket-sid", "S-1-15-2-1-2-3-4-5-6"), 64)
    start("w-e-paket-sid-doppelt", "--paket-sid zweimal ist ein Aufruffehler", a + ["--paket-sid", PAKET], 64)
    start("w-e-pipe-dateipfad", "Ein Dateipfad statt eines Pipe-Namens ist ein Aufruffehler",
          ersetze_arg(a, "--socket", "{root}/s.sock"), 64)
    start("w-e-pipe-fern", "Ein ferner Pipe-Name ist ein Aufruffehler",
          ersetze_arg(a, "--socket", "\\\\host\\pipe\\wb-such-" + hex32), 64)
    start("w-e-pipe-gross", "Großbuchstaben im Zufallsteil sind ein Aufruffehler",
          ersetze_arg(a, "--socket", "\\\\.\\pipe\\wb-such-" + hex32.upper()), 64)
    start("w-e-pipe-kurz", "31 statt 32 Zeichen im Zufallsteil sind ein Aufruffehler",
          ersetze_arg(a, "--socket", "\\\\.\\pipe\\wb-such-" + hex32[:31]), 64)
    start("w-e-pipe-fremder-praefix", "Ein Name ohne das Präfix wb-such- ist ein Aufruffehler",
          ersetze_arg(a, "--socket", "\\\\.\\pipe\\wb-plattform"), 64)
    start("w-e-socket-rechte", "--socket-rechte ist unter Windows ein Aufruffehler, auch mit 0600",
          a + ["--socket-rechte", "0600"], 64)
    start("w-e-pipe-belegt", "Hält schon ein anderer Server den Namen, endet der Prozess mit 74; die fremde Pipe bleibt",
          a, 74, vorbelegen=True)
    start("w-e-reihenfolge", "Eine unbekannte Route geht vor dem belegten Namen: 78, nicht 74",
          ersetze_arg(a, "--route", "unbekannt"), 78, vorbelegen=True)

    ping = {"op": "ping", "id": 1}
    lauf("w-zugleich-vier", "Vier Verbindungen zugleich bekommen je ihre Antwort",
         [{"zugleich": 4, "request": ping}], [{"ok": True, "id": 1}] * 4, [], ["attrappe", "vm-container"])
    lauf("w-peer-person", "Die Person außerhalb des AppContainers wird abgewiesen, zweimal, mit je einer Zeile",
         [{"peer": "person", "request": ping}, {"verbindung": "neu"}, {"peer": "person", "request": {"op": "ping", "id": 2}}],
         [{"abgewiesen": True}, {"abgewiesen": True}], [ab("pk.appcontainer_falsch"), ab("pk.appcontainer_falsch")],
         ["attrappe", "windows"])
    lauf("w-peer-person-dann-agent", "Nach der Abweisung der Person bedient der Prozess den Agenten",
         [{"peer": "person", "request": ping}, {"peer": "agent", "request": {"op": "ping", "id": 2}}],
         [{"abgewiesen": True}, {"ok": True, "id": 2}], [ab("pk.appcontainer_falsch")], ["attrappe", "vm-container"])
    lauf("w-peer-andere-sitzung",
         "Ein Lauf derselben AppContainer-SID aus einer anderen Sitzung wird abgewiesen und bekommt keinen Treffer",
         [{"peer": "andere-sitzung", "request": {"op": "search", "query": "Lieferplan", "id": 1}},
          {"peer": "agent", "request": {"op": "ping", "id": 2}}],
         [{"abgewiesen": True}, {"ok": True, "id": 2}], [ab("such.sitzung_fremd")], ["attrappe", "vm-container"],
         verboten=["qkx1", "qkx2", "lieferplan"])
    lauf("w-peer-ohne-lauf", "Ein Prozess mit der Paket-SID, aber ohne Job eines Laufs, wird abgewiesen",
         [{"peer": "ohne-lauf", "request": ping}], [{"abgewiesen": True}], [ab("such.lauf_unbekannt")],
         ["attrappe", "vm-container"])
    lauf("w-peer-dienst-stumm", "Antwortet der Plattformdienst nicht, wird auch der Agent abgewiesen (fail closed)",
         [{"peer": "dienst-stumm", "request": ping}], [{"abgewiesen": True}], [ab("such.dienst_stumm")],
         ["attrappe", "vm-container"])
    lauf("w-peer-zeit-falsch", "Nennt der Dienst eine andere Startzeit als die Messung (PID wiederverwendet), wird abgewiesen",
         [{"peer": "zeit-falsch", "request": ping}], [{"abgewiesen": True}], [ab("such.lauf_unbekannt")], ["attrappe"],
         hinweis="unter Windows nur mit erzwungener PID-Wiederverwendung prüfbar (offener Punkt des Vertrags)")
    lauf("w-peer-anderer-container", "Ein AppContainer eines anderen Projekts scheitert an der DACL (Fehler 5), ohne Zeile",
         [{"peer": "anderer-container", "request": ping}, {"peer": "agent", "request": {"op": "ping", "id": 2}}],
         [{"oeffnen_fehler": 5}, {"ok": True, "id": 2}], [], ["vm-container"])
    lauf("w-peer-fremdes-konto", "Ein zweites Konto scheitert an der DACL (Fehler 5), ohne Zeile; die Pipe bleibt",
         [{"peer": "fremdes-konto", "request": ping}, {"verbindung": "neu"}, {"peer": "person", "request": ping}],
         [{"oeffnen_fehler": 5}, {"abgewiesen": True}], [ab("pk.appcontainer_falsch")], ["vm-konto"])
    lauf("w-hallo-fehlt", "Ohne Hallo trennt der Prozess nach 2 s ohne ein Byte, mit Zeile; danach geht es weiter",
         [{"peer": "person", "stumm": True}, {"peer": "person", "request": ping}],
         [{"abgewiesen": True}, {"abgewiesen": True}], [ab("pk.hallo_fehlt"), ab("pk.appcontainer_falsch")],
         ["attrappe", "windows"])
    lauf("w-hallo-falsch", "Ein falsches Hallo wird vor jeder Messung abgewiesen, auch beim Agenten",
         [{"peer": "agent", "hallo": "pk2", "request": ping}, {"peer": "agent", "request": {"op": "ping", "id": 2}}],
         [{"abgewiesen": True}, {"ok": True, "id": 2}], [ab("pk.hallo_falsch")], ["attrappe", "vm-container"])
    lauf("w-hallo-falsch-person", "Ein falsches Hallo ergibt pk.hallo_falsch, bevor die Person als fremd gilt",
         [{"peer": "person", "hallo": "PK1", "request": ping}], [{"abgewiesen": True}], [ab("pk.hallo_falsch")],
         ["attrappe", "windows"])
    lauf("w-protokoll-gesperrt", "Ist der Protokollordner nicht schreibbar, bleibt die Abweisung; der Prozess läuft weiter",
         [{"chmod": {"protokoll": "000"}}, {"peer": "person", "request": ping}, {"verbindung": "neu"},
          {"peer": "person", "request": ping}],
         [{"abgewiesen": True}, {"abgewiesen": True}], [], ["attrappe"],
         hinweis="chmod; unter Windows über eine ACL des Protokollordners nachzubauen (offen)")
    lauf("w-pipe-sicherheit", "Besitzer, DACL und Label der Pipe haben genau die Form aus Abschnitt 3; das Lesen der "
         "Beschreibung ist eine Verbindung ohne Hallo",
         [{"sicherheit": True}], [], [ab("pk.hallo_fehlt")], ["windows"],
         sicherheit={"besitzer": "{eigene_sid}", "dacl_flags": "P",
                     "aces": [["A", "0x12019f", "{eigene_sid}"], ["A", "0x12019b", PAKET]],
                     "label": ["ML", "NW", "S-1-16-4096"]})
    lauf("w-instanz-agent", "Der Agent legt keine Instanz unter dem Namen an (Fehler 5)",
         [{"probe": "instanz"}], [{"probe_fehler": 5}], [], ["vm-container"])
    lauf("w-prozess-gesperrt", "Der Agent öffnet den Such-Prozess nicht als Prozess (Fehler 5)",
         [{"probe": "prozess"}], [{"probe_fehler": 5}], [], ["vm-container"])
    lauf("w-ende", "Nach dem Ende des Prozesses ist der Name frei", [], [], [], ["attrappe", "windows"])
    return aus


def mcp_faelle() -> dict:
    daten = json.loads((V4 / "mcp/faelle.json").read_text(encoding="utf-8"))
    hex32 = "0123456789abcdef" * 2
    faelle = []
    for f in daten["faelle"]:
        g = copy.deepcopy(f)
        g["herkunft"] = f"vertrag4:{f['name']}"
        g["name"] = "v4-" + f["name"]
        if "start_exit" not in g:
            g["argv"] = ["--socket", "{pipe}"]
        g["orte"] = ["attrappe", "windows"]
        faelle.append(g)

    def e(name, beschr, argv):
        faelle.append({"name": name, "beschreibung": beschr, "herkunft": "windows", "argv": argv, "start_exit": 64,
                       "orte": ["attrappe", "windows"]})

    e("wm-ohne-socket", "Unter Windows ist --socket Pflicht: ohne Angabe Exit 64", [])
    e("wm-name-dateipfad", "Ein Dateipfad statt eines Pipe-Namens: Exit 64", ["--socket", "/run/brain/such.sock"])
    e("wm-name-fern", "Ein ferner Pipe-Name: Exit 64", ["--socket", "\\\\host\\pipe\\wb-such-" + hex32])
    e("wm-name-gross", "Großbuchstaben im Zufallsteil: Exit 64", ["--socket", "\\\\.\\pipe\\wb-such-" + hex32.upper()])
    e("wm-socket-doppelt", "--socket zweimal: Exit 64", ["--socket", "{pipe}", "--socket", "{pipe}"])
    aufruf = {"sende": {"jsonrpc": "2.0", "id": 21, "method": "tools/call",
                        "params": {"name": "brain_suche", "arguments": {"query": "x"}}}}
    faelle.append({"name": "wm-fremder-besitzer", "herkunft": "windows", "orte": ["vm-konto"],
                   "beschreibung": "Gehört die Pipe einem anderen Konto, sendet such-mcp nichts: Suche nicht erreichbar",
                   "argv": ["--socket", "{pipe}"], "standin": "fremdes-konto",
                   "socket_antworten": ["{\"fallback\":true,\"hits\":[],\"gesperrt\":0}"],
                   "schritte": [aufruf | {"erwarte": {"id": 21, "werkzeug_text": "Suche nicht erreichbar", "isError": True}}],
                   "socket_zeilen": []})
    faelle.append({"name": "wm-server-ohne-hallo", "herkunft": "windows", "orte": ["attrappe", "windows"],
                   "beschreibung": "Schließt der Server nach dem Hallo ohne Antwort, sendet such-mcp keine Anfrage",
                   "argv": ["--socket", "{pipe}"], "hallo_antwort": False,
                   "schritte": [aufruf | {"erwarte": {"id": 21, "werkzeug_text": "Suche nicht erreichbar", "isError": True}}],
                   "socket_zeilen": []})
    faelle.append({"name": "wm-identifikation", "herkunft": "windows", "orte": ["windows"],
                   "beschreibung": "such-mcp verbindet mit Stufe Identifikation; der Server kann nicht in seinem Namen handeln",
                   "argv": ["--socket", "{pipe}"], "stufe_messen": True,
                   "socket_antworten": ["{\"fallback\":true,\"hits\":[],\"gesperrt\":0}"],
                   "schritte": [aufruf | {"erwarte": {"id": 21, "werkzeug_text": "{\"fallback\":true,\"hits\":[],\"gesperrt\":0}",
                                                      "isError": False}}],
                   "socket_zeilen": [{"op": "search", "query": "x"}], "stufe": "identifikation"})
    return {"werkzeug": daten["werkzeug"], "faelle": faelle}


# ---------------------------------------------------------------- Formprüfung

def form_such(name: str, fall: dict, erw: dict) -> list[str]:
    f: list[str] = []
    if not re.fullmatch(r"(v4|w)-[a-z0-9-]+", name):
        f.append(f"{name}: Name")
    pflicht = {"beschreibung", "herkunft", "orte", "files", "dirs", "args", "steps", "verboten"}
    if not pflicht <= set(fall) or set(fall) - pflicht - {"hinweis", "vorbelegen"}:
        f.append(f"{name}: Felder {sorted(fall)}")
        return f
    orte = set(fall["orte"])
    if not orte or not orte <= ORTE:
        f.append(f"{name}: orte {fall['orte']}")
    a = fall["args"]
    if a.count("--paket-sid") > 1 and name != "w-e-paket-sid-doppelt":
        f.append(f"{name}: --paket-sid doppelt")
    if "{sock}" in a:
        f.append(f"{name}: {{sock}} statt {{pipe}}")
    anfragen = 0
    for st in fall["steps"]:
        art = set(st) - {"peer", "hallo"}
        if "zugleich" in art:
            art -= {"request"}
        if len(art) != 1 or not art <= SCHRITTE:
            f.append(f"{name}: Schritt {st}")
            continue
        peer = st.get("peer", "agent")
        if peer not in PEERS:
            f.append(f"{name}: Peer {peer}")
        if "request" in st or "line" in st or "stumm" in st:
            anfragen += st.get("zugleich", 1)
            if peer in NICHT_ATTRAPPE_PEERS and "attrappe" in orte:
                f.append(f"{name}: {peer} kann die Attrappe nicht bilden")
            if peer in NUR_ATTRAPPE_PEERS and orte != {"attrappe"}:
                f.append(f"{name}: {peer} gibt es nur in der Attrappe")
            ort = PEER_ORT[peer]
            if ort and ort != "windows" and orte - {"attrappe"} and ort not in orte:
                f.append(f"{name}: Peer {peer} braucht Ort {ort}")
        if "zugleich" in st:
            if "request" not in st or not 1 <= st["zugleich"] <= 4:
                f.append(f"{name}: zugleich")
        if "probe" in st:
            anfragen += 1
            if st["probe"] not in ("instanz", "prozess") or orte != {"vm-container"}:
                f.append(f"{name}: probe {st['probe']}")
        if "hallo" in st and ("request" not in st or st["hallo"] == "pk1"):
            f.append(f"{name}: hallo nur mit request und nur abweichend")
        if "chmod" in st and orte != {"attrappe"}:
            f.append(f"{name}: chmod nur in der Attrappe")
        if "sicherheit" in st and orte != {"windows"}:
            f.append(f"{name}: {art} nur unter Windows")
    if erw["start"] == "bereit":
        if set(erw) - {"start", "responses", "protokoll", "ende", "sicherheit", "marker_zaehlung", "stderr_zeile"}:
            f.append(f"{name}: Erwartung {sorted(erw)}")
        if len(erw["responses"]) != anfragen:
            f.append(f"{name}: {anfragen} Anfragen, {len(erw['responses'])} Antworten erwartet")
        for z in erw["protokoll"]:
            if z["aktionsname"] == "kanal.abgewiesen":
                if set(z) != {"aktionsname", "ergebnis", "fassung", "grund", "kanal", "route", "sitzung"} or z["grund"] not in GRUENDE:
                    f.append(f"{name}: Zeile {z}")
        abgewiesen = sum(1 for r in erw["responses"] if r == {"abgewiesen": True})
        abgewiesen += sum(1 for st in fall["steps"] if "sicherheit" in st)   # Verbindung ohne Hallo
        zeilen = sum(1 for z in erw["protokoll"] if z["aktionsname"] == "kanal.abgewiesen")
        if name != "w-protokoll-gesperrt" and zeilen != abgewiesen:
            f.append(f"{name}: {abgewiesen} Abweisungen, {zeilen} Zeilen")
    else:
        if set(erw) != {"start"} or not isinstance(erw["start"].get("exit"), int):
            f.append(f"{name}: Start {erw}")
        if fall["steps"]:
            f.append(f"{name}: Schritte trotz Startfehler")
    return f


def form_mcp(daten: dict) -> list[str]:
    f: list[str] = []
    namen = set()
    for g in daten["faelle"]:
        n = g["name"]
        if n in namen:
            f.append(f"{n}: doppelt")
        namen.add(n)
        if not set(g.get("orte", [])) <= ORTE or not g.get("orte"):
            f.append(f"{n}: orte")
        if "start_exit" not in g and "{pipe}" not in g.get("argv", []):
            f.append(f"{n}: argv ohne {{pipe}}")
        if "{sock}" in json.dumps(g):
            f.append(f"{n}: {{sock}}")
    return f


def bauen() -> tuple[dict[str, bytes], list[str]]:
    dateien: dict[str, bytes] = {}
    fehler: list[str] = []
    alle = v4_faelle() | w_faelle()
    for name, (fall, erw) in sorted(alle.items()):
        fehler += form_such(name, fall, erw)
        dateien[f"faelle/{name}.json"] = (json.dumps(fall, ensure_ascii=False, indent=1) + "\n").encode()
        dateien[f"erwartet/{name}.json"] = (json.dumps(erw, ensure_ascii=False, indent=1) + "\n").encode()
    mcp = mcp_faelle()
    fehler += form_mcp(mcp)
    dateien["mcp_faelle.json"] = (json.dumps(mcp, ensure_ascii=False, indent=1) + "\n").encode()
    return dateien, fehler


def main() -> int:
    pruefe = sys.argv[1:] == ["--pruefe"]
    if sys.argv[1:] not in ([], ["--pruefe"]):
        print("Aufruf: bau_faelle.py [--pruefe]", file=sys.stderr)
        return 2
    dateien, fehler = bauen()
    for z in fehler:
        print("FORM  " + z)
    if pruefe:
        da = {p.relative_to(HIER).as_posix() for d in ("faelle", "erwartet") for p in (HIER / d).glob("*.json")}
        da |= {"mcp_faelle.json"} if (HIER / "mcp_faelle.json").exists() else set()
        stand = []
        for rel in sorted(set(dateien) | da):
            p = HIER / rel
            if rel not in dateien:
                stand.append(f"{rel}: liegt da, wird nicht gebaut")
            elif not p.exists() or p.read_bytes() != dateien[rel]:
                stand.append(f"{rel}: weicht vom Bau ab")
        for z in stand:
            print("STAND " + z)
        fehler += stand
    else:
        for d in ("faelle", "erwartet"):
            (HIER / d).mkdir(exist_ok=True)
            for p in (HIER / d).glob("*.json"):
                if f"{d}/{p.name}" not in dateien:
                    p.unlink()
        for rel, inhalt in dateien.items():
            (HIER / rel).write_bytes(inhalt)
    such = sum(1 for k in dateien if k.startswith("faelle/"))
    mcp = len(json.loads(dateien["mcp_faelle.json"])["faelle"])
    print(f"{such} Fälle für den Such-Prozess, {mcp} für such-mcp, {len(fehler)} Fehler")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
