#!/usr/bin/env python3
"""Erzeugt die sichtbaren Fälle des Halter-Vertrags: faelle/<name>.json und erwartet/<name>.json.

Aufruf: python3 -I rust/spec/halter/bau-faelle.py
Die Signaturen der Scheine entstehen aus zwei festen Beispielsamen (unten). Sie sind Testschlüssel ohne
Wert außerhalb dieser Fälle; kein echter Schlüssel der Stelle steht im Repo.
"""
import base64
import copy
import hashlib
import json
import shutil
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
sys.path.insert(0, str(HIER))
import werkzeug as w  # noqa: E402

STELLE = hashlib.sha256(b"halter-vertrag-beispiel-stelle").digest()
FREMD = hashlib.sha256(b"halter-vertrag-beispiel-fremd").digest()
KID = "stelle-mcp-1"
SID_A = "S-1-5-21-1000-1000-1000-1001"
SID_B = "S-1-5-21-1000-1000-1000-1002"
PERSON_A = "ps_" + "a1" * 16
PERSON_B = "ps_" + "b2" * 16
LAUF = "l-beispiel"
JETZT = 1795000000000
START = JETZT - 3600000
VORSCHAU = "c3" * 32

LAGE = {
    "aufrufer": {"rolle": "hub", "konto": SID_A, "vertreter": False},
    "jetzt_ms": JETZT,
    "start_ms": START,
    "kids": {KID: base64.b64encode(w.oeffentlich(STELLE)).decode()},
    "benutzt": [],
    "nachweis_gueltig": True,
    "konfig": {
        "ziele": {"cloud-beispiel": {"pfade": ["POST /v1/messages"], "schluessel": "modell.cloud-beispiel"}},
        "server": {"tickets": {"schluessel": "mcp.tickets", "freigabe": True,
                               "werkzeuge": {"update": "schreibend", "get": "lesend"}},
                   "notizen": {"schluessel": "mcp.notizen", "freigabe": False,
                               "werkzeuge": {"anlegen": "schreibend"}}},
    },
    "geheimnisse": [
        {"name": "modell.cloud-beispiel", "konto": None, "person": None},
        {"name": "mcp.tickets", "konto": SID_A, "person": PERSON_A},
        {"name": "mcp.notizen", "konto": SID_A, "person": PERSON_A},
        {"name": "mcp.wiki", "konto": SID_B, "person": PERSON_B},
    ],
}
LAGE["konfig"]["server"]["wiki"] = {"schluessel": "mcp.wiki", "freigabe": True, "werkzeuge": {"seite": "lesend"}}

FAELLE = {}


def lage(rolle="hub", **aenderung):
    l = copy.deepcopy(LAGE)
    l["aufrufer"]["rolle"] = rolle
    for k, v in aenderung.items():
        l[k] = v
    return l


KONTO_ZIEL = "konto-beispiel"
KONTO_PFADE = ["GET /copilot_internal/*", "GET /responses", "POST /chat/completions"]


def lage_konto(rolle="bruecke", konto=SID_A, strom=None):
    """Lage mit einem Modellziel, dessen Geheimnis eine Anmeldung der Person ist (Nachtrag B05)."""
    l = lage(rolle)
    l["aufrufer"]["konto"] = konto
    l["konfig"]["ziele"][KONTO_ZIEL] = {"pfade": KONTO_PFADE, "schluessel": "anmeldung." + KONTO_ZIEL}
    l["geheimnisse"].append({"name": "anmeldung." + KONTO_ZIEL, "konto": SID_A, "person": None})
    if strom is not None:
        l["strom"] = strom
    return l


def zeile(obj):
    return w.kanonisch(obj)


def fall(name, beschreibung, text=None, *, roh=None, code=None, lage_=None, form="anfrage", erzeugt=None):
    f = {"beschreibung": beschreibung, "form": form}
    if erzeugt is not None:
        f["erzeugt"] = erzeugt
    elif roh is not None:
        f["b64"] = base64.b64encode(roh).decode()
    else:
        f["text"] = text
    if form == "anfrage":
        f["lage"] = lage_ if lage_ is not None else lage()
    erw = {"gueltig": code is None}
    if code is not None:
        erw["code"] = code
    assert name not in FAELLE, name
    FAELLE[name] = (f, erw)


def schein(daten_aend=None, *, schluessel=STELLE, nach_signatur=None, argumente=None, vorschau=VORSCHAU,
           person=PERSON_A, werkzeug="update"):
    argumente = {"id": "t-1", "status": "erledigt"} if argumente is None else argumente
    daten = {"ablauf": JETZT + 30000, "anfrage": "mf_" + "0d" * 16, "art": "mcp-schein",
             "ausgestellt": JETZT - 10000,
             "digest": w.digest("tickets", werkzeug, argumente, person, vorschau),
             "einmal": "5e" * 16, "fassung": 1, "kid": KID, "lauf": LAUF}
    daten.update(daten_aend or {})
    sig = w.signieren(schluessel, w.kanonisch_bytes(daten))
    daten = dict(daten, **(nach_signatur or {}))
    return {"daten": daten, "signatur": base64.b64encode(sig).decode()}


def mcp(werkzeug="update", argumente=None, schein_=..., server="tickets", person=PERSON_A, vorschau=VORSCHAU,
        lauf=LAUF, **zusatz):
    a = {"argumente": {"id": "t-1", "status": "erledigt"} if argumente is None else argumente,
         "fassung": 1, "id": 7, "lauf": lauf, "op": "mcp.aufruf", "person": person, "server": server,
         "vorschau_sha256": vorschau, "werkzeug": werkzeug}
    if schein_ is ...:
        schein_ = schein() if werkzeug == "update" else None
    if schein_ is not None:
        a["schein"] = schein_
    a.update(zusatz)
    return a


def vorhanden(**zusatz):
    a = {"fassung": 1, "id": "q-1", "name": "mcp.tickets", "op": "schluessel.vorhanden"}
    a.update(zusatz)
    return a


def modell(**zusatz):
    a = {"abfrage": None, "fassung": 1, "id": "m-1",
         "koerper_b64": base64.b64encode(b'{"model":"claude-sonnet-5-5"}').decode(),
         "kopf": {"anthropic-version": "2023-06-01", "content-type": "application/json"},
         "methode": "POST", "op": "modell.aufruf", "pfad": "/v1/messages", "ziel": "cloud-beispiel"}
    a.update(zusatz)
    return a


def konto_modell(**zusatz):
    a = modell(ziel=KONTO_ZIEL, methode="GET", pfad="/copilot_internal/user", koerper_b64="",
               kopf={"accept": "application/json"})
    a.update(zusatz)
    return a


def strom(**zusatz):
    a = {"abfrage": None, "fassung": 1, "id": "m-2",
         "kopf": {"sec-websocket-key": "dGhlIHNhbXBsZSBub25jZQ==", "sec-websocket-version": "13"},
         "op": "modell.strom", "pfad": "/responses", "ziel": KONTO_ZIEL}
    a.update(zusatz)
    return a


def suche(**zusatz):
    a = {"fassung": 1, "id": "u-1", "op": "modell.suche", "pfade": ["/srv/beispiel/arbeit"], "ziel": KONTO_ZIEL}
    a.update(zusatz)
    return a


def setzen(**zusatz):
    a = {"fassung": 1, "id": "s-1", "nachweis": "nw-beispiel", "name": "mcp.tickets", "op": "schluessel.setzen",
         "wert_b64": base64.b64encode(b"beispielwert").decode()}
    a.update(zusatz)
    return a


def ohne(obj, feld):
    obj = dict(obj)
    del obj[feld]
    return obj


def bauen():
    # ---- gültig ----
    fall("a-vorhanden-hub", "Hub fragt, ob sein Token vorhanden ist", zeile(vorhanden()))
    fall("a-vorhanden-oberflaeche", "Oberfläche fragt nach einem Schlüssel", zeile(vorhanden()), lage_=lage("oberflaeche"))
    fall("a-vorhanden-id-zahl", "id als ganze Zahl", zeile(vorhanden(id=9007199254740991)))
    fall("a-modell-gueltig", "Brücke ruft das Modell über ein bekanntes Ziel", zeile(modell()), lage_=lage("bruecke"))
    fall("a-mcp-lesend", "lesender MCP-Aufruf ohne Schein", zeile(mcp("get", {"id": "t-1"}, None)))
    fall("a-mcp-schreibend", "schreibender Aufruf mit gültigem Schein", zeile(mcp()))
    fall("a-mcp-schein-letzte-ms", "Schein in der letzten Millisekunde vor dem Ablauf",
         zeile(mcp()), lage_=lage(jetzt_ms=JETZT + 29999))
    fall("a-mcp-schein-frist-24h", "Schein mit der längsten Einlösefrist von 86400000 ms",
         zeile(mcp(schein_=schein({"ablauf": JETZT - 10000 + 86400000}))))
    fall("a-mcp-freigabe-aus", "Server mit Option Freigabe aus: Schreibaufruf ohne Schein",
         zeile(mcp("anlegen", {"titel": "x"}, None, server="notizen")))
    fall("a-mcp-unicode-argument", "Argument mit Umlaut und Steuerzeichen in kanonischer Form",
         zeile(mcp("get", {"id": "t-1", "notiz": "Grüße\n\u0001"}, None)))
    fall("a-setzen", "Oberfläche übergibt einen Token mit Nachweis", zeile(setzen()), lage_=lage("oberflaeche"))
    fall("a-loeschen", "Oberfläche löscht einen Token mit Nachweis",
         zeile({"fassung": 1, "id": "s-2", "nachweis": "nw-beispiel", "name": "mcp.tickets", "op": "schluessel.loeschen"}),
         lage_=lage("oberflaeche"))

    # ---- Stufe 1 und 2 ----
    fall("s01-zu-gross", "Zeile ein Byte über 32 MiB", code="halter.zu_gross",
         erzeugt={"vorne": '{"a":"', "fuellbyte": "x", "hinten": '"}', "laenge": 33554433})
    fall("s01-genau-grenze-json", "Zeile genau 32 MiB, aber kein gültiges Objekt der Form (Grenze ist inklusiv)",
         code="halter.json_ungueltig",
         erzeugt={"vorne": '{"a":"', "fuellbyte": "x", "hinten": '"', "laenge": 33554432})
    fall("s02-peer", "Aufrufer ohne Rolle (Peer-Prüfung gescheitert)", zeile(vorhanden()), code="halter.peer",
         lage_=lage(None))
    # ---- Stufe 3 ----
    fall("s03-bom", "BOM vor dem Objekt", roh=b"\xef\xbb\xbf" + zeile(vorhanden()).encode(), code="halter.json_ungueltig")
    fall("s03-doppelt", "doppelter Schlüssel",
         '{"fassung":1,"id":"q-1","name":"mcp.tickets","name":"mcp.wiki","op":"schluessel.vorhanden"}',
         code="halter.json_ungueltig")
    fall("s03-liste", "oberste Ebene ist eine Liste", "[1]", code="halter.json_ungueltig")
    fall("s03-utf8", "ungültiges UTF-8", roh=b'{"a":"\xff"}', code="halter.json_ungueltig")
    fall("s03-nan", "NaN ist kein JSON", '{"a":NaN}', code="halter.json_ungueltig")
    # ---- Stufe 4 ----
    fall("s04-leerzeichen", "Leerzeichen nach dem Doppelpunkt",
         '{"fassung": 1,"id":"q-1","name":"mcp.tickets","op":"schluessel.vorhanden"}', code="halter.nicht_kanonisch")
    fall("s04-ordnung", "Schlüssel nicht sortiert",
         '{"id":"q-1","fassung":1,"name":"mcp.tickets","op":"schluessel.vorhanden"}', code="halter.nicht_kanonisch")
    fall("s04-eins-punkt-null", "fassung als 1.0",
         '{"fassung":1.0,"id":"q-1","name":"mcp.tickets","op":"schluessel.vorhanden"}', code="halter.nicht_kanonisch")
    fall("s04-escape", "unnötiges Escape \\u0061",
         '{"fassung":1,"id":"q-1","name":"mcp.tick\\u0065ts","op":"schluessel.vorhanden"}', code="halter.nicht_kanonisch")
    # ---- Stufe 5 bis 7 ----
    fall("s05-fassung-2", "fassung 2", zeile(vorhanden(fassung=2)), code="halter.fassung_unbekannt")
    fall("s05-fassung-fehlt", "fassung fehlt", zeile(ohne(vorhanden(), "fassung")), code="halter.fassung_unbekannt")
    fall("s05-fassung-text", "fassung als Text", zeile(vorhanden(fassung="1")), code="halter.fassung_unbekannt")
    fall("s06-op-quit", "op quit gibt es nicht", zeile(vorhanden(op="quit")), code="halter.op_unbekannt")
    fall("s06-op-fehlt", "op fehlt", zeile(ohne(vorhanden(), "op")), code="halter.op_unbekannt")
    fall("s06-op-schluessel-lesen", "eine Op zum Auslesen eines Schlüssels gibt es nicht",
         zeile(vorhanden(op="schluessel.lesen")), code="halter.op_unbekannt")
    fall("s07-hub-modell", "Hub darf kein Modell rufen", zeile(modell()), code="halter.rolle")
    fall("s07-bruecke-mcp", "Brücke darf kein MCP ausführen", zeile(mcp()), code="halter.rolle", lage_=lage("bruecke"))
    fall("s07-hub-setzen", "Hub darf keinen Token setzen", zeile(setzen()), code="halter.rolle")
    # ---- Stufe 8 bis 12 (Schema) ----
    fall("s08-vorhanden-wert", "Anfrage verlangt den Wert mit", zeile(vorhanden(wert=True)), code="halter.schluessel_unbekannt")
    fall("s08-mcp-digest", "Hub schickt einen eigenen digest mit", zeile(mcp(digest="00" * 32)),
         code="halter.schluessel_unbekannt")
    fall("s08-modell-url", "Brücke schickt eine Adresse mit", zeile(modell(url="https://beispiel.invalid/v1")),
         code="halter.schluessel_unbekannt", lage_=lage("bruecke"))
    fall("s08-schein-zusatz", "Schein mit weiterem Feld",
         zeile(mcp(schein_=dict(schein(), notiz="x"))), code="halter.schluessel_unbekannt")
    fall("s09-name-fehlt", "name fehlt", zeile(ohne(vorhanden(), "name")), code="halter.schluessel_fehlt")
    fall("s09-id-fehlt", "id fehlt", zeile(ohne(vorhanden(), "id")), code="halter.schluessel_fehlt")
    fall("s10-id-null", "id null", zeile(vorhanden(id=None)), code="halter.typ_falsch")
    fall("s10-argumente-liste", "argumente als Liste", zeile(mcp("get", [1], None)), code="halter.typ_falsch")
    fall("s11-name-ohne-art", "Schlüsselname ohne Art", zeile(vorhanden(name="tickets")), code="halter.wert_form")
    fall("s11-name-gross", "Schlüsselname mit Großbuchstaben", zeile(vorhanden(name="mcp.Tickets")), code="halter.wert_form")
    fall("s11-pfad-query", "Pfad mit Abfrageteil", zeile(modell(pfad="/v1/messages?beta=true")), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-pfad-relativ", "Pfad ohne führenden Schrägstrich", zeile(modell(pfad="v1/messages")), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-person", "Person nicht als Pseudonym", zeile(mcp("get", {"id": "t-1"}, None, person="person-x")),
         code="halter.wert_form")
    fall("s11-b64", "Körper kein Base64", zeile(modell(koerper_b64="nicht base64")), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-kopf-crlf", "Kopfwert mit Zeilenumbruch",
         zeile(modell(kopf={"content-type": "application/json\r\nx-api-key: a"})), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s12-kopf-authorization", "Brücke setzt Authorization selbst",
         zeile(modell(kopf={"authorization": "Bearer x", "content-type": "application/json"})),
         code="halter.kopf_verboten", lage_=lage("bruecke"))
    fall("s12-kopf-host", "Brücke setzt Host", zeile(modell(kopf={"host": "beispiel.invalid"})),
         code="halter.kopf_verboten", lage_=lage("bruecke"))
    # ---- Stufe 13 bis 18 ----
    fall("s13-setzen-modell", "Oberfläche setzt einen Maschinenschlüssel", zeile(setzen(name="modell.cloud-beispiel")),
         code="halter.bereich", lage_=lage("oberflaeche"))
    fall("s14-ziel-unbekannt", "unbekanntes Ziel", zeile(modell(ziel="cloud-fremd")), code="halter.ziel_unbekannt",
         lage_=lage("bruecke"))
    fall("s14-server-unbekannt", "unbekannter MCP-Server", zeile(mcp("get", {}, None, server="fremd")),
         code="halter.ziel_unbekannt")
    fall("s15-pfad-verboten", "Pfad nicht in der Liste des Ziels", zeile(modell(pfad="/v1/files")),
         code="halter.pfad_verboten", lage_=lage("bruecke"))
    fall("s15-methode-verboten", "Methode nicht in der Liste des Ziels",
         zeile(modell(methode="GET", koerper_b64="")), code="halter.pfad_verboten", lage_=lage("bruecke"))
    fall("s16-fremder-token", "Token gibt es nur für ein anderes Konto", zeile(mcp("seite", {}, None, server="wiki")),
         code="halter.kein_schluessel")
    fall("s16-kein-modell-schluessel", "Maschinenschlüssel fehlt",
         zeile(modell()), code="halter.kein_schluessel",
         lage_=lage("bruecke", geheimnisse=[g for g in LAGE["geheimnisse"] if g["konto"] is not None]))
    fall("s16-person-ohne-token", "für die genannte Person gibt es keinen Token dieses Servers",
         zeile(mcp(person=PERSON_B, schein_=schein(person=PERSON_B))), code="halter.kein_schluessel")
    fall("s17-person-falsch", "Hub einer Person nennt eine andere Person, deren Token es gibt",
         zeile(mcp("seite", {}, None, server="wiki", person=PERSON_B)), code="halter.person")
    vertreter = lage()
    vertreter["aufrufer"] = {"rolle": "hub", "konto": "S-1-5-80-1000-1000-1000-1000-1000", "vertreter": True}
    fall("a-mcp-vertreter", "Hub unter eigener Dienst-SID (Windows) handelt für die genannte Person",
         zeile(mcp("seite", {}, None, server="wiki", person=PERSON_B)), lage_=vertreter)
    fall("s18-schein-fehlt", "Schreibaufruf ohne Schein", zeile(mcp(schein_=None)), code="halter.schein_fehlt")
    fall("s18-werkzeug-ohne-klasse", "Werkzeug ohne Klasse gilt als schreibend",
         zeile(mcp("loeschen", {"id": "t-1"}, None)), code="halter.schein_fehlt")
    fall("s18-schein-unerwartet", "lesender Aufruf mit Schein", zeile(mcp("get", {"id": "t-1"}, schein())),
         code="halter.schein_unerwartet")
    # ---- Schein, Stufen 19 bis 26 ----
    fall("s19-kid-unbekannt", "Schlüsselkennung unbekannt (etwa der Ticket-Schlüssel)",
         zeile(mcp(schein_=schein({"kid": "stelle-ticket-1"}))), code="halter.schein_kid")
    fall("s20-fremder-schluessel", "richtige kid, fremder Schlüssel",
         zeile(mcp(schein_=schein(schluessel=FREMD))), code="halter.schein_signatur")
    fall("s20-ablauf-verlaengert", "Ablauf nach der Signatur verlängert",
         zeile(mcp(schein_=schein(nach_signatur={"ablauf": JETZT + 60000}))), code="halter.schein_signatur")
    fall("s21-art-ticket", "gültig signiert, aber art ticket",
         zeile(mcp(schein_=schein({"art": "ticket"}))), code="halter.schein_art")
    fall("s21-fassung-2", "gültig signiert, aber fassung 2",
         zeile(mcp(schein_=schein({"fassung": 2}))), code="halter.schein_art")
    fall("s22-lauf-fremd", "Schein für einen anderen Lauf",
         zeile(mcp(schein_=schein({"lauf": "l-fremd"}))), code="halter.schein_lauf")
    fall("s23-argument-geaendert", "Hub ändert nach der Freigabe ein Argument",
         zeile(mcp(argumente={"id": "t-2", "status": "erledigt"})), code="halter.schein_digest")
    fall("s23-vorschau-geaendert", "andere Vorschau als freigegeben",
         zeile(mcp(vorschau="d4" * 32)), code="halter.schein_digest")
    fall("s23-werkzeug-getauscht", "anderes schreibendes Werkzeug mit dem Schein von update",
         zeile(mcp("anlegen", schein_=schein(), server="tickets")), code="halter.schein_digest")
    fall("s24-frist-zu-lang", "Ablauf mehr als 86400000 ms nach Ausstellung",
         zeile(mcp(schein_=schein({"ablauf": JETZT - 10000 + 86400001}))), code="halter.schein_zeit")
    fall("s24-zukunft", "ausgestellt mehr als 5 s in der Zukunft",
         zeile(mcp(schein_=schein({"ausgestellt": JETZT + 5001, "ablauf": JETZT + 35001}))), code="halter.schein_zeit")
    fall("s25-abgelaufen", "jetzt gleich Ablauf", zeile(mcp()), code="halter.schein_abgelaufen",
         lage_=lage(jetzt_ms=JETZT + 30000))
    fall("s25-vor-start", "ausgestellt vor dem Start des Halters", zeile(mcp()), code="halter.schein_abgelaufen",
         lage_=lage(start_ms=JETZT - 9999))
    fall("s26-benutzt", "dieselbe Einmal-Kennung schon eingelöst", zeile(mcp()), code="halter.schein_benutzt",
         lage_=lage(benutzt=["5e" * 16]))
    fall("s27-nachweis", "Nachweis ungültig", zeile(setzen()), code="halter.nachweis",
         lage_=lage("oberflaeche", nachweis_gueltig=False))

    # ---- Nachtrag B05: Anmeldung, Abfrageteil, Teilbäume, Strom, Suche ----
    k = lage_konto()
    fall("a-modell-abfrage", "Abfrageteil im eigenen Feld", zeile(modell(abfrage="api-version=2025-01-01&x=%20a")),
         lage_=lage("bruecke"))
    fall("a-modell-anmeldung-teilbaum", "Anmeldung der Person, Pfad im Teilbaum /copilot_internal/*",
         zeile(konto_modell()), lage_=k)
    fall("a-modell-anmeldung-chat", "Anmeldung der Person, genauer Pfad POST /chat/completions",
         zeile(konto_modell(methode="POST", pfad="/chat/completions", koerper_b64=base64.b64encode(b"{}").decode())),
         lage_=k)
    fall("a-strom", "Upgrade auf WebSocket an GET /responses", zeile(strom()), lage_=k)
    fall("a-strom-teil", "Bytes des Harness im offenen Strom",
         zeile({"daten_b64": base64.b64encode(b"\x81\x85abcd").decode(), "fassung": 1, "id": "m-2", "op": "strom.teil"}),
         lage_=lage_konto(strom="m-2"))
    fall("a-strom-ende", "Brücke beendet den offenen Strom", zeile({"fassung": 1, "id": "m-2", "op": "strom.ende"}),
         lage_=lage_konto(strom="m-2"))
    fall("a-suche", "Suche nach dem Schlüssel eines Ziels nach dem Lauf", zeile(suche()), lage_=k)
    fall("a-setzen-anmeldung", "Oberfläche setzt eine Anmeldung (Bereich person)",
         zeile(setzen(name="anmeldung." + KONTO_ZIEL)), lage_=lage("oberflaeche"))
    fall("s07-hub-strom", "Hub darf keinen Strom öffnen", zeile(strom()), code="halter.rolle", lage_=lage_konto("hub"))
    fall("s07-hub-suche", "Hub darf nicht suchen", zeile(suche()), code="halter.rolle", lage_=lage_konto("hub"))
    fall("s07a-teil-ohne-strom", "strom.teil ohne offenen Strom",
         zeile({"daten_b64": "AAAA", "fassung": 1, "id": "m-2", "op": "strom.teil"}), code="halter.strom", lage_=k)
    fall("s07a-teil-fremde-id", "strom.teil mit der id eines anderen Stroms",
         zeile({"daten_b64": "AAAA", "fassung": 1, "id": "m-3", "op": "strom.teil"}), code="halter.strom",
         lage_=lage_konto(strom="m-2"))
    fall("s07a-aufruf-bei-offenem-strom", "andere Op, solange der Strom offen ist", zeile(konto_modell()),
         code="halter.strom", lage_=lage_konto(strom="m-2"))
    fall("s09-abfrage-fehlt", "modell.aufruf ohne Feld abfrage", zeile(ohne(modell(), "abfrage")),
         code="halter.schluessel_fehlt", lage_=lage("bruecke"))
    fall("s11-abfrage-leer", "leere Abfrage statt null", zeile(modell(abfrage="")), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-abfrage-raute", "Abfrage mit Fragment", zeile(modell(abfrage="a=1#x")), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-abfrage-zu-lang", "Abfrage mit 1025 Zeichen", zeile(modell(abfrage="a" * 1025)), code="halter.wert_form",
         lage_=lage("bruecke"))
    fall("s11-pfad-prozent", "Pfad mit kodiertem Punkt", zeile(konto_modell(pfad="/copilot_internal/%2e%2e/user")),
         code="halter.wert_form", lage_=k)
    fall("s11-suche-relativ", "Suchpfad nicht absolut", zeile(suche(pfade=["srv/beispiel"])), code="halter.wert_form",
         lage_=k)
    fall("s11-suche-punktpunkt", "Suchpfad mit Segment ..", zeile(suche(pfade=["/srv/beispiel/../x"])),
         code="halter.wert_form", lage_=k)
    fall("s11-suche-17", "17 Suchpfade", zeile(suche(pfade=["/srv/beispiel/%d" % i for i in range(17)])),
         code="halter.wert_form", lage_=k)
    fall("s11-name-anmeldung-gross", "Name der Art anmeldung mit Großbuchstaben",
         zeile(vorhanden(name="anmeldung.Konto")), code="halter.wert_form")
    fall("s08-suche-muster", "Suche mit eigenem Muster", zeile(suche(muster="marke-x")), code="halter.schluessel_unbekannt",
         lage_=k)
    fall("s08-strom-methode", "modell.strom mit Feld methode", zeile(strom(methode="GET")),
         code="halter.schluessel_unbekannt", lage_=k)
    fall("s12-kopf-accept-encoding", "Brücke setzt accept-encoding selbst",
         zeile(modell(kopf={"accept-encoding": "gzip"})), code="halter.kopf_verboten", lage_=lage("bruecke"))
    fall("s12-strom-erweiterung", "Upgrade mit Erweiterungskopf",
         zeile(strom(kopf={"sec-websocket-extensions": "permessage-deflate", "sec-websocket-version": "13"})),
         code="halter.kopf_verboten", lage_=k)
    fall("s14-suche-ziel-unbekannt", "Suche für ein unbekanntes Ziel", zeile(suche(ziel="cloud-fremd")),
         code="halter.ziel_unbekannt", lage_=k)
    fall("s15-pfad-punktpunkt", "Teilbaum per .. verlassen", zeile(konto_modell(pfad="/copilot_internal/../user/repos")),
         code="halter.pfad_verboten", lage_=k)
    fall("s15-pfad-punkt", "Segment . im Teilbaum", zeile(konto_modell(pfad="/copilot_internal/./user")),
         code="halter.pfad_verboten", lage_=k)
    fall("s15-pfad-doppelstrich", "doppelter Schrägstrich im Teilbaum", zeile(konto_modell(pfad="/copilot_internal//user")),
         code="halter.pfad_verboten", lage_=k)
    fall("s15-teilbaum-praefix", "Präfix ohne Schrägstrich deckt der Teilbaum nicht",
         zeile(konto_modell(pfad="/copilot_internalx")), code="halter.pfad_verboten", lage_=k)
    fall("s15-genau-unterpfad", "genauer Pfad deckt keinen Unterpfad",
         zeile(konto_modell(methode="POST", pfad="/chat/completions/x")), code="halter.pfad_verboten", lage_=k)
    fall("s15-strom-nur-post", "Strom auf einem Pfad, der nur für POST erlaubt ist", zeile(strom(pfad="/chat/completions")),
         code="halter.pfad_verboten", lage_=k)
    fall("s16-anmeldung-anderes-konto", "Anmeldung gehört einem anderen Konto als dem der Brücke", zeile(konto_modell()),
         code="halter.kein_schluessel", lage_=lage_konto(konto=SID_B))
    fall("s16-suche-ohne-anmeldung", "Suche ohne Anmeldung des eigenen Kontos", zeile(suche()),
         code="halter.kein_schluessel", lage_=lage_konto(konto=SID_B))

    # ---- Antworten: was herausgehen darf ----
    def antwort(name, beschreibung, obj, gueltig=True):
        fall(name, beschreibung, zeile(obj), form="antwort", code=None if gueltig else "antwort.form")

    antwort("o-vorhanden", "Antwort auf vorhanden", {"art": "antwort", "fassung": 1, "id": "q-1", "ok": True, "vorhanden": True})
    antwort("o-fehler", "Fehlerantwort mit festem Grund",
            {"art": "antwort", "code": "halter.schein_benutzt", "fassung": 1, "grund": "Schein schon benutzt", "id": 7, "ok": False})
    antwort("o-fehler-ohne-id", "Fehlerantwort auf unlesbare Zeile",
            {"art": "antwort", "code": "halter.json_ungueltig", "fassung": 1, "grund": "kein gueltiges JSON", "id": None, "ok": False})
    antwort("o-mcp-fehler", "Fehler des Firmensystems nur als Zahl",
            {"art": "antwort", "code": "halter.mcp_fehler", "fassung": 1, "grund": "MCP-Server meldet Fehler", "id": 7,
             "mcp_code": -32603, "ok": False})
    antwort("o-mcp-ergebnis", "Ergebnis eines MCP-Aufrufs",
            {"art": "antwort", "ergebnis": {"content": [{"text": "ok", "type": "text"}]}, "fassung": 1, "id": 7, "ok": True})
    antwort("o-beginn", "Beginn einer Modellantwort",
            {"art": "beginn", "fassung": 1, "id": "m-1", "kopf": {"content-type": "text/event-stream"}, "status": 200})
    antwort("o-teil", "Teil einer Modellantwort",
            {"art": "teil", "daten_b64": base64.b64encode(b"event: ping\n\n").decode(), "fassung": 1, "id": "m-1"})
    antwort("o-ende", "Ende einer Modellantwort", {"art": "ende", "fassung": 1, "id": "m-1", "ok": True})
    antwort("o-ende-leck", "Abbruch wegen Leck",
            {"art": "ende", "code": "halter.leck_gesperrt", "fassung": 1, "grund": "Antwort enthielt einen Schluessel",
             "id": "m-1", "ok": False})
    antwort("o-suche", "Ergebnis der Suche nur als Zahlen",
            {"art": "antwort", "fassung": 1, "id": "u-1", "ok": True,
             "suche": {"dateien": 12, "nicht_lesbar": 0, "treffer": 0, "uebersprungen": 1}})
    antwort("o-beginn-101", "Beginn eines Stroms mit Status 101",
            {"art": "beginn", "fassung": 1, "id": "m-2", "kopf": {"sec-websocket-accept": "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="},
             "status": 101})
    antwort("o-ende-komprimiert", "Abbruch wegen komprimierter Antwort",
            {"art": "ende", "code": "halter.ziel_komprimiert", "fassung": 1, "grund": "Antwort komprimiert", "id": "m-1",
             "ok": False})
    antwort("o-suche-mit-pfad", "Suche nennt einen Pfad",
            {"art": "antwort", "fassung": 1, "id": "u-1", "ok": True,
             "suche": {"dateien": 1, "nicht_lesbar": 0, "pfad": "/srv/beispiel/a", "treffer": 1, "uebersprungen": 0}},
            False)
    antwort("o-beginn-upgrade", "Beginn reicht Upgrade weiter",
            {"art": "beginn", "fassung": 1, "id": "m-2", "kopf": {"upgrade": "websocket"}, "status": 101}, False)
    antwort("o-mit-wert", "Antwort mit dem Wert des Schlüssels",
            {"art": "antwort", "fassung": 1, "id": "q-1", "ok": True, "vorhanden": True, "wert": "beispielwert"}, False)
    antwort("o-fehler-meldung", "Fehler mit freiem Text des Firmensystems",
            {"art": "antwort", "code": "halter.mcp_fehler", "fassung": 1, "grund": "MCP-Server meldet Fehler", "id": 7,
             "meldung": "token abc ungueltig", "ok": False}, False)
    antwort("o-beginn-set-cookie", "Beginn reicht Set-Cookie weiter",
            {"art": "beginn", "fassung": 1, "id": "m-1", "kopf": {"set-cookie": "a=b"}, "status": 200}, False)
    antwort("o-fehler-grund-umbruch", "Grund mit Zeilenumbruch",
            {"art": "antwort", "code": "halter.intern", "fassung": 1, "grund": "a\nb", "id": 7, "ok": False}, False)


def main():
    bauen()
    for ordner in ("faelle", "erwartet"):
        ziel = HIER / ordner
        if ziel.exists():
            shutil.rmtree(ziel)  # frei herstellbar aus diesem Skript
        ziel.mkdir()
    for name, (f, erw) in sorted(FAELLE.items()):
        (HIER / "faelle" / f"{name}.json").write_text(json.dumps(f, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (HIER / "erwartet" / f"{name}.json").write_text(json.dumps(erw, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{len(FAELLE)} Fälle geschrieben")


if __name__ == "__main__":
    main()
