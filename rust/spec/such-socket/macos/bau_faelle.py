#!/usr/bin/env python3
"""Fallbauer für den Nachtrag „Such-Socket unter macOS“ zu Vertrag 4. Nur Standardbibliothek.

Aufruf: python3 bau_faelle.py            schreibt faelle/, erwartet/ und mcp_faelle.json neu
        python3 bau_faelle.py --pruefe   baut im Speicher, prüft die Form und vergleicht mit der Platte; ändert nichts
Exit 0: Form gut (und bei --pruefe gleich). Exit 1: Formfehler oder Abweichung.

Quellen: ../sichtbar/ (Fälle von Vertrag 4), ../mcp/faelle.json (MCP-Fälle von Vertrag 4) und die Fälle des Nachtrags
unten (README Abschnitt 7). Bei gleichem Stand ist die Ausgabe Byte für Byte gleich.

Platzhalter in den Fällen: {root} ist der Ordner des Falls, {sock} der Socket (`<root>/s/such.sock`, Ordner `s` mit 0700).
"""
from __future__ import annotations

import copy
import json
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
V4 = HIER.parent

ORTE = {"mac", "mac-profil", "mac-zweites-konto", "bau"}
PEERS = {"eigene", "fremd", "unmessbar"}
SCHRITTE = {"request", "line", "write", "chmod", "verbindung", "profil"}
PROFIL_SCHRITTE = {"eigener-socket", "fremde-sitzung", "prozess"}
# Aufbau vor dem Start (Schlüssel -> Art des Wertes)
AUFBAU = {"socket_bytes": int, "socket_relativ": bool, "socket_ordner_modus": str, "socket_ordner_verweis": bool,
          "socket_verweis_ins_leere": bool, "socket_ordner_fremd": bool}
NACHHER = {"ordner_modus": str, "ziel_ohne_socket": bool, "ziel_leer": bool, "verweis_bleibt": bool}
GRUENDE = {"such.kennung_unmessbar", "such.kennung_fremd"}
# Vertrag-4-Fälle, die unter macOS entfallen (README Abschnitt 7, „Übernommen“)
WEG_V4 = {"v-socket-rechte-0666": "unter macOS ist --socket-rechte 0666 ein Aufruffehler (mac-a-rechte-0666)"}
SITZUNG = "s-test-1"
ROUTE = "cloud"
PING = {"op": "ping", "id": 1}


def ab(grund: str) -> dict:
    """Protokollzeile von Stufe 2 ohne `vorher` und `zeit` (wie bei Windows)."""
    return {"aktionsname": "kanal.abgewiesen", "ergebnis": "abgewiesen", "fassung": 2, "grund": grund, "kanal": "such",
            "route": ROUTE, "sitzung": SITZUNG}


def stufen(grund: str, anzahl: int) -> dict:
    """Soll beider Stufen von Abschnitt 4. Stufe 1 gilt jetzt, Stufe 2 erst nach einem spaeteren Zug."""
    meldung = f"such.abgewiesen: {grund}"
    return {"1": {"stderr_zeilen": [meldung] * anzahl, "protokoll": []},
            "2": {"stderr_zeilen": [], "protokoll": [ab(grund)] * anzahl}}


def lade(pfad: Path) -> dict:
    return json.loads(pfad.read_text(encoding="utf-8"))


def basis() -> dict:
    """Vault, Einstellungen und Aufruf aus p-ping von Vertrag 4."""
    fall = lade(V4 / "sichtbar/faelle/p-ping.json")
    return {"files": fall["files"], "dirs": fall["dirs"], "args": fall["args"], "verboten": []}


def ersetze_arg(args: list[str], option: str, wert: str | None) -> list[str]:
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
        if p.stem in WEG_V4 or p.stem.startswith("r-") or nur_v4(lade(p), lade(V4 / "sichtbar/erwartet" / p.name)):
            # r-*: Rückfall der Einbettung (Attrappe, Datenbank); unter macOS fährt sie pruefe_macos.py --vertrag4
            # direkt aus ../sichtbar, unter Windows schreibt der Such-Prozess die Zeile noch nicht.
            continue
        fall = lade(p)
        erw = lade(V4 / "sichtbar/erwartet" / p.name)
        # der Socketordner `s` gehört dem Läufer; eine vorgelegte Datei am Socketpfad liegt darin
        files = {("s/such.sock" if rel == "s.sock" else rel): text for rel, text in fall["files"].items()}
        neu = {"beschreibung": fall["description"], "herkunft": f"vertrag4:{p.stem}", "orte": ["mac"], "files": files,
               "dirs": fall["dirs"], "args": fall["args"], "steps": fall["steps"], "verboten": fall["verboten"]}
        aus[f"v4-{p.stem}"] = (neu, erw)
    return aus


def mac_faelle(v4: dict[str, tuple[dict, dict]]) -> dict[str, tuple[dict, dict]]:
    aus: dict[str, tuple[dict, dict]] = {}
    a = basis()["args"]

    def start(name, beschr, args, exit_, orte=("mac",), nachher=None, **mehr):
        f = basis() | {"beschreibung": beschr, "herkunft": "macos", "orte": list(orte), "args": args, "steps": []}
        f |= mehr
        e = {"start": {"exit": exit_}}
        if nachher:
            e["nachher"] = nachher
        aus[name] = (f, e)

    def lauf(name, beschr, steps, erw_teil, orte=("mac",), **mehr):
        f = basis() | {"beschreibung": beschr, "herkunft": "macos", "orte": list(orte), "steps": steps}
        f |= mehr
        aus[name] = (f, {"start": "bereit", "socket_rechte": "0600"} | erw_teil)

    # ---- Aufruf
    start("mac-a-rechte-0666", "--socket-rechte 0666 ist unter macOS ein Aufruffehler; nichts wird angelegt",
          a + ["--socket-rechte", "0666"], 64)
    start("mac-a-pfad-relativ", "Ein relativer Socketpfad ist ein Aufruffehler", ersetze_arg(a, "--socket", "such.sock"),
          64, aufbau=[{"socket_relativ": True}])
    start("mac-a-pfad-104", "Ein Socketpfad mit 104 Byte ist ein Aufruffehler (sun_path fasst 104 Byte mit dem NUL)",
          a, 64, aufbau=[{"socket_bytes": 104}])
    lauf("mac-a-pfad-103", "Ein Socketpfad mit genau 103 Byte wird angenommen; die Suche antwortet",
         [{"request": PING}, {"request": {"op": "ping", "id": 2}}],
         {"responses": [{"ok": True, "id": 1}, {"ok": True, "id": 2}], "protokoll": [],
          "ende": {"exit": 0, "socket_weg": True, "ordner_bleibt": True}}, aufbau=[{"socket_bytes": 103}])
    # ---- Ordner und Socket
    start("mac-a-ordner-0755", "Elternordner mit 0755: Exit 74, der Ordner bleibt unverändert", a, 74,
          aufbau=[{"socket_ordner_modus": "0755"}], nachher={"ordner_modus": "0755"})
    start("mac-a-ordner-verweis", "Der Elternordner ist ein Verweis auf einen Ordner 0700: Exit 74, der Verweis bleibt",
          a, 74, aufbau=[{"socket_ordner_verweis": True}], nachher={"verweis_bleibt": True, "ziel_leer": True})
    start("mac-a-ordner-fremd", "Der Elternordner gehört einer anderen Kennung: Exit 74", a, 74,
          orte=("mac-zweites-konto",), aufbau=[{"socket_ordner_fremd": True}])
    start("mac-a-verweis-ins-leere", "Am Socketpfad liegt ein Verweis ins Leere: Exit 74, am Ziel entsteht kein Socket", a,
          74, aufbau=[{"socket_verweis_ins_leere": True}], nachher={"verweis_bleibt": True, "ziel_ohne_socket": True})
    # ---- Peer
    cloud_f, cloud_e = v4["v4-t-cloud-sperrt"]
    lauf("mac-p-gleiche-kennung", "Ping und eine Suche unter derselben Kennung: Antworten wie unter Linux, keine Abweisung",
         [{"request": PING}] + cloud_f["steps"],
         {"responses": [{"ok": True, "id": 1}] + cloud_e["responses"], "protokoll": cloud_e["protokoll"],
          "ohne_aktionsname": ["kanal.abgewiesen"], "stderr_ohne_praefix": "such.abgewiesen:",
          "ende": {"exit": 0, "socket_weg": True, "ordner_bleibt": True}}, verboten=cloud_f["verboten"])
    ping2 = {"op": "ping", "id": 2}
    abgewiesen_erw = {"responses": [{"abgewiesen": True}, {"ok": True, "id": 2}], "geltend": 2,
                      "ende": {"exit": 0, "socket_weg": True, "ordner_bleibt": True}}
    lauf("mac-p-fremde-kennung",
         "Ein zweites Konto verbindet (Rechte absichtlich offen): kein Byte, Ende der Verbindung; danach geht es weiter",
         [{"peer": "fremd", "request": PING}, {"peer": "eigene", "request": ping2}],
         abgewiesen_erw | {"stufen": stufen("such.kennung_fremd", 1)}, orte=("mac-zweites-konto", "bau"))
    lauf("mac-p-unmessbar", "getpeereid scheitert: kein Byte, Abweisung mit dem Grund such.kennung_unmessbar",
         [{"peer": "unmessbar", "request": PING}, {"peer": "eigene", "request": ping2}],
         abgewiesen_erw | {"stufen": stufen("such.kennung_unmessbar", 1)}, orte=("bau",))
    lauf("mac-p-zeile-scheitert",
         "Nur Stufe 2: Abweisung bei nicht schreibbarem Protokoll: kein Byte, Meldung ohne Inhalt, der Prozess läuft weiter",
         [{"chmod": {"protokoll": "000"}}, {"peer": "fremd", "request": PING}, {"peer": "eigene", "request": ping2}],
         abgewiesen_erw | {"geltend": 2,
                           "stufen": {"2": {"stderr_zeilen": [], "stderr_ohne_inhalt": True, "protokoll": []}}},
         orte=("bau",))
    # ---- Ende
    lauf("mac-e-sigterm", "SIGTERM: Exit 0, der Socket ist weg, der Ordner bleibt", [],
         {"responses": [], "protokoll": [], "ende": {"exit": 0, "socket_weg": True, "ordner_bleibt": True}})
    # ---- Profil des Einschlusses
    lauf("mac-profil-eigener", "Der Agent im Profil fragt den eigenen Socket und bekommt eine Antwort (U1)",
         [{"profil": "eigener-socket", "request": PING}],
         {"responses": [{"ok": True, "id": 1}], "protokoll": [], "ende": {"exit": 0, "socket_weg": True,
                                                                          "ordner_bleibt": True}}, orte=("mac-profil",))
    lauf("mac-profil-fremde-sitzung",
         "Der Agent im Profil verbindet den Socket einer anderen Sitzung und bekommt EPERM (U2)",
         [{"profil": "fremde-sitzung"}],
         {"responses": [{"errno": "EPERM"}], "protokoll": [], "ende": {"exit": 0, "socket_weg": True,
                                                                       "ordner_bleibt": True}}, orte=("mac-profil",))
    # Ohne lldb (Entscheidung vom 22.11.2026): Den Task-Port sperrt macOS für Prozesse derselben Kennung ohne Debug-Recht selbst, mit und
    # ohne Profil (P4, Entscheidung vom 22.11.2026: kern=5); eine Prüfung im Profil wäre blind. Sie steht als mac-s-ohne-get-task-allow.
    lauf("mac-profil-prozess",
         "Der Agent im Profil sendet SIGTERM an den Such-Prozess: EPERM (Signal 0 von außen gelingt), die Suche antwortet weiter",
         [{"profil": "prozess"}, {"request": PING}],
         {"responses": [{"kill": "scheitert"}, {"ok": True, "id": 1}], "protokoll": [],
          "ende": {"exit": 0, "socket_weg": True, "ordner_bleibt": True}}, orte=("mac-profil",))
    return aus


def mcp_faelle() -> dict:
    daten = lade(V4 / "mcp/faelle.json")
    faelle = []
    for f in daten["faelle"]:
        g = copy.deepcopy(f)
        g["herkunft"] = f"vertrag4:{f['name']}"
        g["name"] = "v4-" + f["name"]
        if "start_exit" not in g:
            g["argv"] = ["--socket", "{sock}"]
        g["orte"] = ["mac"]
        faelle.append(g)
    aufruf = {"sende": {"jsonrpc": "2.0", "id": 21, "method": "tools/call",
                        "params": {"name": "brain_suche", "arguments": {"query": "x"}}}}
    antwort = "{\"fallback\":true,\"hits\":[],\"gesperrt\":0}"

    def e(name, beschr, argv, orte=("mac",)):
        faelle.append({"name": name, "beschreibung": beschr, "herkunft": "macos", "argv": argv, "start_exit": 64,
                       "orte": list(orte)})

    e("mac-m-ohne-socket", "Unter macOS ist --socket Pflicht: ohne Angabe Exit 64 ohne Ausgabe", [])
    e("mac-m-pfad-relativ", "Ein relativer Pfad: Exit 64 ohne Ausgabe", ["--socket", "such.sock"])
    e("mac-m-pfad-104", "Ein Pfad mit 104 Byte: Exit 64 ohne Ausgabe", ["--socket", "{sock104}"])
    faelle.append({"name": "mac-m-pfad-103", "herkunft": "macos", "orte": ["mac"],
                   "beschreibung": "Ein Pfad mit genau 103 Byte wird angenommen; der Aufruf kommt durch",
                   "argv": ["--socket", "{sock103}"], "socket_antworten": [antwort],
                   "schritte": [aufruf | {"erwarte": {"id": 21, "werkzeug_text": antwort, "isError": False}}],
                   "socket_zeilen": [{"op": "search", "query": "x"}]})
    faelle.append({"name": "mac-m-server-fremd", "herkunft": "macos", "orte": ["mac-zweites-konto", "bau"],
                   "beschreibung": "Hat der Server eine andere euid, sendet such-mcp kein Byte: Suche nicht erreichbar",
                   "argv": ["--socket", "{sock}"], "standin": "fremde-euid", "socket_antworten": [antwort],
                   "schritte": [aufruf | {"erwarte": {"id": 21, "werkzeug_text": "Suche nicht erreichbar", "isError": True}}],
                   "socket_zeilen": []})
    return {"werkzeug": daten["werkzeug"], "faelle": faelle}


# Rechte, die ein ausgeliefertes Programm nie trägt (Entscheidung vom 22.11.2026). An `get-task-allow` hängt, ob ein Prozess derselben
# Kennung den Task-Port bekommt (Nachtrag Abschnitt 5).
VERBOTENE_RECHTE = ["com.apple.security.get-task-allow"]
SIGNATUR_PROGRAMME = {"rangkern": "RANGKERN_BIN"}


def signatur_faelle() -> dict:
    """Statische Fälle an der Signatur: kein Prozess, nur `codesign -d --entitlements - --xml <programm>`."""
    faelle = []
    for prog in SIGNATUR_PROGRAMME:
        faelle.append({"name": f"mac-s-{prog}-ohne-get-task-allow", "herkunft": "macos", "orte": ["mac"], "programm": prog,
                       "beschreibung": f"{prog} trägt keines der verbotenen Rechte; eine Kopie, die get-task-allow trägt, "
                                       "macht dieselbe Prüfung rot (Gegenprobe)",
                       "verbotene_rechte": VERBOTENE_RECHTE})
    return {"faelle": faelle}


def form_signatur(daten: dict) -> list[str]:
    f: list[str] = []
    for g in daten["faelle"]:
        if set(g) != {"name", "herkunft", "orte", "programm", "beschreibung", "verbotene_rechte"}:
            f.append(f"{g.get('name')}: Felder {sorted(g)}")
        if g.get("programm") not in SIGNATUR_PROGRAMME or g.get("orte") != ["mac"] or not g.get("verbotene_rechte"):
            f.append(f"{g.get('name')}: programm, orte oder Rechte")
    return f


# ---------------------------------------------------------------- Formprüfung

def form_such(name: str, fall: dict, erw: dict) -> list[str]:
    f: list[str] = []
    if not re.fullmatch(r"(v4|mac)-[a-z0-9-]+", name):
        f.append(f"{name}: Name")
    pflicht = {"beschreibung", "herkunft", "orte", "files", "dirs", "args", "steps", "verboten"}
    if not pflicht <= set(fall) or set(fall) - pflicht - {"aufbau"}:
        f.append(f"{name}: Felder {sorted(fall)}")
        return f
    orte = set(fall["orte"])
    if not orte or not orte <= ORTE:
        f.append(f"{name}: orte {fall['orte']}")
    if "{pipe}" in json.dumps(fall) or "run/brain" in json.dumps(fall):
        f.append(f"{name}: Pfad aus Linux oder Windows")
    if "--socket-rechte" in fall["args"] and fall["args"][fall["args"].index("--socket-rechte") + 1] == "0666" \
            and name != "mac-a-rechte-0666":
        f.append(f"{name}: 0666 gibt es unter macOS nur im Fall mac-a-rechte-0666")
    for a in fall.get("aufbau", []):
        if len(a) != 1 or next(iter(a)) not in AUFBAU or not isinstance(next(iter(a.values())), AUFBAU[next(iter(a))]):
            f.append(f"{name}: Aufbau {a}")
        if "socket_ordner_fremd" in a and "mac-zweites-konto" not in orte:
            f.append(f"{name}: fremder Ordner braucht mac-zweites-konto")
    for rel in fall["files"]:
        if rel == "s.sock":
            f.append(f"{name}: Datei s.sock statt s/such.sock")
    anfragen = 0
    abgewiesen = 0
    for st in fall["steps"]:
        art = set(st) - {"peer"}
        if "profil" in art:
            art -= {"request"}
        if len(art) != 1 or not art <= SCHRITTE:
            f.append(f"{name}: Schritt {st}")
            continue
        peer = st.get("peer", "eigene")
        if peer not in PEERS:
            f.append(f"{name}: Peer {peer}")
        if peer != "eigene":
            abgewiesen += 1
            if "mac" in orte:
                f.append(f"{name}: Peer {peer} kann der Fall mit Ort mac nicht bilden")
        if "request" in st or "line" in st or "profil" in st:
            anfragen += 1
        if "profil" in st:
            if st["profil"] not in PROFIL_SCHRITTE or orte != {"mac-profil"}:
                f.append(f"{name}: profil {st['profil']}")
        if "chmod" in st and not set(st["chmod"]) <= {"protokoll", "zaehl"}:
            f.append(f"{name}: chmod {st['chmod']}")
    if erw["start"] == "bereit":
        erlaubt = {"start", "socket_rechte", "responses", "protokoll", "ende", "marker_zaehlung", "stderr_zeile", "stufen",
                   "geltend", "ohne_aktionsname", "stderr_ohne_praefix"}
        if set(erw) - erlaubt:
            f.append(f"{name}: Erwartung {sorted(erw)}")
        if len(erw["responses"]) != anfragen:
            f.append(f"{name}: {anfragen} Anfragen, {len(erw['responses'])} Antworten erwartet")
        if erw["socket_rechte"] != "0600":
            f.append(f"{name}: Rechte {erw['socket_rechte']}")
        if abgewiesen:
            if sum(1 for r in erw["responses"] if r == {"abgewiesen": True}) != abgewiesen:
                f.append(f"{name}: {abgewiesen} Abweisungen, andere Zahl der Antworten")
            for s, inhalt in erw.get("stufen", {}).items():
                for z in inhalt["protokoll"]:
                    if set(z) != {"aktionsname", "ergebnis", "fassung", "grund", "kanal", "route", "sitzung"} \
                            or z["grund"] not in GRUENDE:
                        f.append(f"{name}: Zeile {z}")
                if name != "mac-p-zeile-scheitert":
                    n = len(inhalt["stderr_zeilen"]) if s == "1" else len(inhalt["protokoll"])
                    if n != abgewiesen:
                        f.append(f"{name}: Stufe {s} nennt {n} statt {abgewiesen} Abweisungen")
                if s == "1" and inhalt["protokoll"]:
                    f.append(f"{name}: Stufe 1 schreibt keine Zeile")
            if erw.get("geltend") not in (1, 2, None) or str(erw.get("geltend")) not in erw.get("stufen", {}) and erw.get("geltend") is not None:
                f.append(f"{name}: geltend {erw.get('geltend')}")
            if "protokoll" in erw:
                f.append(f"{name}: protokoll statt stufen")
        else:
            if "stufen" in erw or "protokoll" not in erw:
                f.append(f"{name}: ohne Abweisung gilt protokoll, nicht stufen")
        for z in erw.get("protokoll", []):
            if z["aktionsname"] == "kanal.abgewiesen" and "ohne_aktionsname" not in erw:
                f.append(f"{name}: Zeile kanal.abgewiesen ohne Abweisung")
    else:
        if set(erw) - {"start", "nachher"} or not isinstance(erw["start"].get("exit"), int):
            f.append(f"{name}: Start {erw}")
        for k, v in erw.get("nachher", {}).items():
            if k not in NACHHER or not isinstance(v, NACHHER[k]):
                f.append(f"{name}: nachher {k}")
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
        if "start_exit" not in g and "{sock}" not in " ".join(g.get("argv", [])) and "{sock103}" not in g.get("argv", []):
            f.append(f"{n}: argv ohne Socketpfad")
        if "{pipe}" in json.dumps(g) or "run/brain" in json.dumps(g):
            f.append(f"{n}: Pfad aus Linux oder Windows")
    return f


def bauen() -> tuple[dict[str, bytes], list[str]]:
    dateien: dict[str, bytes] = {}
    fehler: list[str] = []
    v4 = v4_faelle()
    alle = v4 | mac_faelle(v4)
    for name, (fall, erw) in sorted(alle.items()):
        fehler += form_such(name, fall, erw)
        dateien[f"faelle/{name}.json"] = (json.dumps(fall, ensure_ascii=False, indent=1) + "\n").encode()
        dateien[f"erwartet/{name}.json"] = (json.dumps(erw, ensure_ascii=False, indent=1) + "\n").encode()
    mcp = mcp_faelle()
    fehler += form_mcp(mcp)
    dateien["mcp_faelle.json"] = (json.dumps(mcp, ensure_ascii=False, indent=1) + "\n").encode()
    sig = signatur_faelle()
    fehler += form_signatur(sig)
    dateien["signatur_faelle.json"] = (json.dumps(sig, ensure_ascii=False, indent=1) + "\n").encode()
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
        da |= {n for n in ("mcp_faelle.json", "signatur_faelle.json") if (HIER / n).exists()}
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
    sig = len(json.loads(dateien["signatur_faelle.json"])["faelle"])
    print(f"{such} Fälle für den Such-Prozess, {mcp} für such-mcp, {sig} an der Signatur, {len(fehler)} Fehler")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
