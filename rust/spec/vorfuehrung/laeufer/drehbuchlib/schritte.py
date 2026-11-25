"""Die Schritte S1 bis S5 und die Ehrlichkeitspruefungen E1 bis E3 des Drehbuchs (`rust/spec/vorfuehrung/DREHBUCH.md`).

Jede Funktion `s1(L)` bis `e3(L)` fuehrt einen Schritt aus und gibt ein `Schritt` zurueck. Ein Schritt sammelt
Teilpruefungen (`haupt` fuer das Erwartete, `gegen` fuer die Gegenprobe). Gruen ist er nur, wenn jede Teilpruefung
besteht. Eine Messung, die in dieser Konfiguration nicht moeglich ist, macht ihn `offen`, nie gruen.
"""
from __future__ import annotations

import glob
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import time

from . import konfig, lauf, messen


ANGELEGT = []  # alle Schritte des laufenden Ablaufs in der Reihenfolge des Anlegens; ablauf.fuehre_aus leert die Liste zu Beginn


class Schritt:
    def __init__(self, kennung, name):
        self.kennung, self.name = kennung, name
        self.teile = []      # (art, name, ok)
        self.messwerte = {}  # Haupt
        self.gegenwerte = {}
        self.offen = []
        self.fehler = None
        self.code_rot = None    # eigener Code, wenn der Schritt rot ist (statt <schritt>.teil<n>)
        self.code_offen = None  # eigener Code, wenn der Schritt offen ist
        self.code_fest = None  # fester Fehlercode fuer die Ergebnisdatei, wenn der Schritt an einer bekannten Bedingung scheitert
        self.uebersprungen = False
        ANGELEGT.append(self)  # damit ablauf bei einer Ausnahme die schon vermerkten Teile behaelt (Abschluss 21.11.)

    def haupt(self, name, ok, **werte):
        self.teile.append(("haupt", name, bool(ok)))
        self.messwerte.update(werte)
        return bool(ok)

    def gegen(self, name, ok, **werte):
        self.teile.append(("gegen", name, bool(ok)))
        self.gegenwerte.update(werte)
        return bool(ok)

    def ergebnis(self):
        if self.uebersprungen:
            return "uebersprungen"
        if self.fehler or any(not ok for _, _, ok in self.teile):
            return "rot"
        return "offen" if self.offen else "gruen"

    def gegenprobe(self):
        g = [t for t in self.teile if t[0] == "gegen"]
        if self.uebersprungen:
            return "uebersprungen"
        if not g:
            return "keine"
        return "gruen" if all(ok for _, _, ok in g) else "rot"

    def als_dict(self):
        return {"schritt": self.kennung, "name": self.name, "ergebnis": self.ergebnis(), "gegenprobe": self.gegenprobe(),
                "messwerte": self.messwerte, "gegenprobe_messwerte": self.gegenwerte, "offen": self.offen,
                "teile": [{"art": a, "name": n, "ok": ok} for a, n, ok in self.teile], "fehler": self.fehler,
                "code_fest": self.code_fest, "code_rot": self.code_rot, "code_offen": self.code_offen}


def _kopf(kennung, was, code):
    from . import ablauf
    r = Schritt(kennung, ablauf.NAMEN[kennung])
    r.offen.append("%s.%s: %s" % (kennung.lower(), code, was))
    r.code_fest = "%s.%s" % (kennung.lower(), code)
    return r


def nur_attrappe(kennung, was):
    """Schritt, der im knappen Cloud-Lauf (echt) nicht laeuft: `offen` mit `<schritt>.nur_attrappe`, nie gruen. Gegen die Attrappen laeuft er voll."""
    return _kopf(kennung, was, "nur_attrappe")


def nur_cloud(kennung):
    """Schritt der lokalen Route im Messbetrieb `--nur-cloud`: `offen` mit `<schritt>.nur_cloud`, nie gruen."""
    return _kopf(kennung, "nicht gelaufen: Messbetrieb --nur-cloud, die lokale Route wird nicht geprueft", "nur_cloud")


BUENDEL_ZEILE = re.compile(r"^\W*([a-z][a-z0-9_]*)=(\d+) RC=(\d+)\s*$", re.M)
TOKENPRUEFUNGEN = ("token_env", "token_ps", "token_proc", "token_dateien", "token_gh", "token_secret")
# Positivkontrolle von E4-cloud: derselbe Weg wie `token_env` (die Umgebung der Cloud-Sitzung) zaehlt den PLATZHALTER, der dort stehen muss.
# Name der Vorlage in `befehle` und der Zeile im Buendel; keine Tokenpruefung (kein Teil im Ergebnis, nur ein Messwert).
KONTROLLE_ENV = "kontrolle_env"
BUENDEL_KONTROLLE = "e4c_kontrolle_platzhalter"
# Positivkontrolle des Wegs von `token_proc`: derselbe Lesepfad (die Startumgebung der Prozesse, Fehler nach /dev/null) zaehlt den
# PLATZHALTER in den Prozessen der Sitzung (die CLI traegt ihn dort). Erwartet >= 1, sonst blind.
KONTROLLE_PROC = "kontrolle_proc"
BUENDEL_KONTROLLE_PROC = "e4c_kontrolle_proc"


def ohne_tokenpraefix(text):
    """Schreibt `gho_` als `gh''o_`. In einfachen Anfuehrungszeichen setzt die Shell daraus dasselbe Wort zusammen; der Text des Befehls,
    den die CLI im Terminal zeigt, traegt das Praefix eines Tokens aber nie woertlich. Grund (Probe 2 der Aufnahme, 21.11.): Das Terminal
    bricht die lange Zeile um, und die Geheimnis-Pruefung des Videos wertet jedes `gho_` am Zeilenende und jeden zerrissenen Platzhalter
    zu Recht als moegliches Token. Nur fuer Werte, die in den Vorlagen in einfachen Anfuehrungszeichen stehen (`{muster}`, `{platzhalter}`
    der Tokenpruefungen)."""
    return text.replace("gho_", "gh''o_")


def cloud_buendel(L):
    """EIN Buendel Shell-Befehle fuer die Cloud-Sitzung: die Umgebungspruefung von S1 und die Tokenpruefungen von E4-cloud. Jeder Befehl
    gibt eine eigene Zeile `<name>=<zahl> RC=<n>` aus (nur Zahlen, nie ein Wert)."""
    k = L.k
    c, b = k["copilot"], k["befehle"]
    teile = [("s1_umgebung", b["s1_umgebung"]), ("s1_route_namen", b["s1_route_namen"]),
             ("s1_route_wert", _fmt(b["s1_route_wert"], route=L.cloud))]
    for n in TOKENPRUEFUNGEN:
        teile.append(("e4c_" + n, _fmt(b[n], muster=ohne_tokenpraefix(c["token_muster"]), platzhalter=ohne_tokenpraefix(c["platzhalter"]))))
    teile.append((BUENDEL_KONTROLLE, _fmt(b[KONTROLLE_ENV], platzhalter=ohne_tokenpraefix(c["platzhalter"]))))  # weitere Zeile, keine weitere Eingabe
    teile.append((BUENDEL_KONTROLLE_PROC, _fmt(b[KONTROLLE_PROC], platzhalter=ohne_tokenpraefix(c["platzhalter"]))))  # und noch eine Zeile
    return "; ".join('z=$(%s); echo "%s=$z RC=$?"' % (cmd, name) for name, cmd in teile)


def buendel_lesen(z):
    """Die Zeilen des Buendels aus der Ausgabe des Zugs: {name: (zahl, rc)}; bei doppelten Namen gilt die letzte."""
    return {m.group(1): (int(m.group(2)), int(m.group(3))) for m in BUENDEL_ZEILE.finditer(z.ausgabe())}


KINDER_WARTEZEIT_S = 0.5  # nach den Antworten 422 eine kurze feste Wartezeit, dann erst zaehlen


def _kinder_fenster(L, warte_s=0.0):
    """Stand fuer die Gegenprobe von S1 ohne Diagnose: (Menge der Sitzungskennungen laut Sitzungsliste oder None, Zahl der direkten Kinder des
    Kerns oder None). Die Kinder zaehlt der Laeufer nur aus `<bruecke.proc_wurzel>/[0-9]*/stat` unter der MainPID von `kern_einheit`; ohne
    `kern_einheit` oder bei MainPID 0 ist die Zahl None (offen)."""
    k = L.k
    if warte_s:
        time.sleep(warte_s)
    ids = None
    if hasattr(L.b, "sitzungs_ids"):
        try:
            ids = L.b.sitzungs_ids()
        except Exception:
            ids = None
    pid = messen.main_pid(k.get("kern_einheit"))
    return ids, messen.kinder_zahl(k["bruecke"].get("proc_wurzel", "/proc"), pid)


def _gegenprobe_ohne_diagnose(L, s, fenster0):
    """Zwei Teile, beide muessen halten: (a) die Menge der Sitzungskennungen ist nach den beiden abgelehnten Starts (ohne Route, unbekannte Route)
    gleich der vorher (eine Antwort 422 legt keine Sitzung an, und der Kern startet ein Kind nur fuer eine Sitzung); (b) die Zahl der direkten
    Kinder des Kerns ist gleich. Nicht messbar ist `offen` mit festem Text, nie gruen. Grenze: Ein Kind, das in der Wartezeit endet und keine
    Sitzung hinterlaesst, sieht Teil (b) nicht."""
    ids1, kinder1 = _kinder_fenster(L, KINDER_WARTEZEIT_S)
    ids0, kinder0 = fenster0
    s.gegenwerte.update(kontrolle_kinder=kinder0)  # Positivkontrolle: Kinder des Kerns, waehrend die Cloud-Sitzung laeuft (erwartet >= 1)
    neue_s = None if ids0 is None or ids1 is None else len(ids1 - ids0)
    neue_k = None if kinder0 is None or kinder1 is None else kinder1 - kinder0
    L.gegenprobe = {"weg": "sitzungen_und_kinder", "neue_sitzungen": neue_s, "neue_kinder": neue_k}
    s.gegenwerte.update(gegenprobe_weg="sitzungen_und_kinder", neue_sitzungen=neue_s, neue_kinder=neue_k)
    if neue_s is None:
        s.offen.append("gegenprobe.sitzungen: die Sitzungsliste der Bruecke ist nicht lesbar, Teil a nicht gemessen")
    else:
        s.gegen("Keine neue Sitzung in der Sitzungsliste fuer die abgelehnten Starts", ids1 == ids0)
    if neue_k is None:
        s.offen.append("gegenprobe.kinder: MainPID des Kerns oder /proc nicht lesbar, Teil b nicht gemessen")
    elif kinder0 == 0 and neue_k == 0:
        # Positivkontrolle: Die Cloud-Sitzung laeuft, also hat der Kern mindestens ein direktes Kind. Sieht der Messweg keins, ist er blind,
        # und "keine neuen Kinder" sagt nichts: fester Code, kein gruener Teil.
        s.offen.append("s1.kontrolle_blind: der Kern hat waehrend der Cloud-Sitzung kein direktes Kind, der Messweg der Gegenprobe sieht nichts")
        s.code_offen = "s1.kontrolle_blind"
    else:
        s.gegen("Keine neuen direkten Kinder des Kerns fuer die abgelehnten Starts", neue_k == 0)


def _text_ohne_rc(t):
    return "\n".join(z for z in (t or "").splitlines() if not re.match(r"^\W*RC=\d+\s*$", z)).strip()


def _fmt(vorlage, **w):
    """Setzt `{name}` ein, ohne andere geschweifte Klammern der Vorlage (etwa `%{http_code}`) anzufassen."""
    for k_, v in w.items():
        vorlage = vorlage.replace("{" + k_ + "}", str(v))
    return vorlage


def _mz(z, marker):
    """Zaehler im Mitschnitt des Modellwegs; None, wenn dieser Weg nicht mitgeschnitten wird (Option)."""
    return None if z.mit is None else messen.zaehle(z.mit, marker)


def _null(*werte):
    """Wahr, wenn jeder gemessene Wert 0 ist; nicht gemessene Werte (None) zaehlen nicht."""
    return all(w == 0 for w in werte if w is not None)


def _gesperrt(antwort):
    return antwort.get("gesperrt") if isinstance(antwort, dict) else None


# Werte aus Antworten der Bruecke, der Suche oder des Protokolls kommen nur als feste Form in die Messwerte: eine Zahl, ein Wahrheitswert,
# None oder ein fester Code. Alles andere ist `abweichend` (nie der Text selbst).
CODE_FORM = re.compile(r"^[a-z][a-z0-9._-]{0,63}$")


def _fest_zahl(x):
    """Zahl, Wahrheitswert oder None unveraendert, sonst `abweichend`."""
    return x if x is None or isinstance(x, (bool, int)) else "abweichend"


def _fest_code(x):
    """Fester Code (`^[a-z][a-z0-9._-]{0,63}$`) oder None unveraendert, sonst `abweichend`."""
    return x if x is None or (isinstance(x, str) and CODE_FORM.match(x)) else "abweichend"


def _fest_aus(x, erlaubt):
    """`x`, wenn es in `erlaubt` steht (feste Werte des Vertrags); None bleibt None, sonst `abweichend`."""
    try:
        return x if x is None or x in erlaubt else "abweichend"
    except TypeError:
        return "abweichend"


def _fest_codes(xs):
    return [_fest_code(x) for x in xs]


# --- S1 --------------------------------------------------------------------------------------------------------
# Teilzeiten von S1 (23.11.2026): feste Namen, Sekunden aus `time.monotonic()`. Sie gehen nur in die Diagnosedatei und
# nur, wenn die ohnehin entsteht (`messen.teilzeit_notiere`). `s1.bruecke_bereit` misst der Ablauf um den Aufbau des Ziels.
ZUGTEILE = ("sichtbar", "enter", "abgesendet", "fertig")  # je Zug ab seinem Beginn; ein nicht erreichter Teil fehlt


def _zug_gemessen(L, nr, fn, *a, **kw):
    """Ein Zug von S1; seine Teilzeiten kommen als `s1.zug<nr>.<teil>` in die Teilzeiten, auch wenn er mit einer Ausnahme endet."""
    t0 = time.monotonic()
    try:
        return fn(*a, **kw)
    finally:
        zeiten = getattr(L, "zugzeiten", None) or {}
        for teil in ZUGTEILE:
            if teil in zeiten:
                messen.teilzeit_notiere("s1.zug%d.%s" % (nr, teil), zeiten[teil])
        # Gerundet wie die Teile (`Lauf._zugzeit`): Runden ist monoton, so liegt `gesamt` nie unter einem Teil (sonst 0,4157 < 0,416).
        messen.teilzeit_notiere("s1.zug%d.gesamt" % nr, round(time.monotonic() - t0, 3))


def _ca_vorpruefung(L, sid, z):
    """Vorpruefung der Sitzungs-CA (Nachtrag 5), ohne zweiten Start. Sie braucht nur Zug 1 (`z`) und laeuft deshalb direkt nach ihm, vor
    dem Buendel: Endet Zug 2 ohne Merkmal, ist das Ergebnis der CA trotzdem gemessen. (a) Die CLI nimmt den Abschluss an: Die erste
    Anfrage kam am Weiterreicher an, und im Terminal steht kein Zertifikatsfehler. (b) Der Ausgang kommt durch: der Wirt erreicht das Ziel."""
    ca_k = L.k.get("ca", {})
    start_text = L.b.verlauf(sid, 0)[1] + z.verlauf
    mw = L.zaehlstand(sid, "modellweg")
    if mw is None:
        L.ca["cli_nimmt_an"] = None
    else:
        L.ca["cli_nimmt_an"] = mw.get("anfragen", 0) >= 1 and not any(re.search(m, start_text, re.I) for m in ca_k.get("fehlermuster", []))
    if ca_k.get("ausgang_befehl") and ca_k.get("ausgang_ziel"):
        t_aus = time.monotonic()
        try:
            rc_a, out_a = messen.aussen_shell(_fmt(ca_k["ausgang_befehl"], ziel=ca_k["ausgang_ziel"]), frist_s=20)
        finally:
            messen.teilzeit_notiere("s1.ausgang_ca", time.monotonic() - t_aus)
        cd = re.findall(r"\d{3}", out_a)
        L.ca["ausgang"] = rc_a == 0 and bool(cd) and cd[0] != "000"


def s1(L):
    s = Schritt("S1", "Sitzung mit Cloud-Modell starten")
    k = L.k
    vor = L.b.diagnose()
    t_start = time.monotonic()
    try:
        m = L.sitzung_starten("cloud")
    finally:
        messen.teilzeit_notiere("s1.start_gesamt", time.monotonic() - t_start)
    messen.teilzeit_notiere("s1.start_eingabefeld", m.get("dauer_s"))  # t0 bis Eingabefeld; fehlt, wenn es ausblieb
    messen.teilzeit_notiere("s1.vertrauensfrage", m.get("t_vertrauen"))  # t0 bis zum Enter auf die Vertrauensfrage; fehlt ohne Frage
    sid = m["sitzung"]
    s.haupt("Start angenommen", sid is not None, status=m["status"])
    if not sid:
        messen.diagnose_notiere("s1.start", "Start abgelehnt: %s" % (m["antwort"],))  # Antwort der Bruecke: nur in die Diagnosedatei
        s.fehler = "Start abgelehnt"
        return s
    L.s1 = sid
    if sid in L.ende_befund:  # ohne Eingabefeld beim Start: Zustand, Code und feste Zaehler der Sitzung in den Messwerten (nur im Bericht)
        s.messwerte.update(L.ende_befund[sid])
    if m.get("nicht_angemeldet"):  # Copilot CLI zeigt `Please use /login`: eigener Fehlercode, kein Grund, weiter zu warten
        s.code_fest = "s1.nicht-angemeldet"
    if k["harness"][L.start_dict("cloud")["harness"]].get("vertrauensfrage_muster"):
        s.haupt("Vertrauensfrage des Ordners (Enter gesendet), danach erscheint das Eingabefeld",
                m.get("vertrauen_bestaetigt") is True and m["t1"] is not None, t_vertrauen_s=m.get("t_vertrauen"))
    s.haupt("Eingabefeld erscheint", m["t1"] is not None, t0_t1_s=m["dauer_s"], rahmen=m["rahmen"])
    grenze = float(k["grenzen"]["start_s"])
    s.haupt("Zeit bis zum Eingabefeld innerhalb der Grenze", m["dauer_s"] is not None and m["dauer_s"] <= grenze, grenze_s=grenze)
    if hasattr(L.b, "cursor"):  # Seite: Cursor in der Form <zeile>,<spalte>,<sichtbar>
        cur = L.b.cursor(sid)
        s.haupt("Cursor des Rasters hat die Form <zeile>,<spalte>,<sichtbar>", cur is not None and cur[2] is True, cursor_form=cur is not None)
    info = L.b.sitzung(sid)
    route = (info.get("start") or {}).get("route") if isinstance(info, dict) else None
    s.haupt("Start-Struktur nennt die Route cloud", route == L.cloud, start_route=_fest_aus(route, (L.cloud, L.lokal)))
    if L.knapp:
        # Knapper Cloud-Lauf: Eingabe 1 ist die gezaehlte Anfrage ohne Werkzeug, Eingabe 2 das eine Buendel Shell-Befehle als Shell-Eingabe
        # der CLI (`prompts.buendel`, beginnt mit `!`): Die CLI fuehrt es selbst in ihrer Shell im Einschluss aus, das Modell bekommt es nicht.
        L.ziel0 = L.zinfo["ziel_zaehler"]() if L.zinfo.get("ziel_zaehler") else None  # fuer E4-cloud: Zaehler vor den Eingaben
        z = _zug_gemessen(L, 1, L.zug, sid, "zaehlen", "")
        s.haupt("Eingabezeile nimmt Text an, die Anfrage ohne Werkzeug wird beantwortet (FERTIG)", True)
        s.haupt("Kopf-Tausch: eine Antwort kam, obwohl im Einschluss nur der Platzhalter steht (Anfrage am Weiterreicher gezaehlt)",
                z.mw is None or z.mw.get("antworten", 0) >= 1, anfragen_eingabe1=None if z.mw is None else z.mw.get("antworten", 0))
        _zaehlung_cloud(s, z, "S1", L)
        L.zug1 = z
        _ca_vorpruefung(L, sid, z)  # vor dem Buendel: die CA braucht nur Zug 1
        zb = _zug_gemessen(L, 2, L.zug, sid, "buendel", cloud_buendel(L), frist_s=float(k["grenzen"].get("baum_s", 30)))
        L.buendel, L.buendel_zug = buendel_lesen(zb), zb
        erwartet = 5 + len(TOKENPRUEFUNGEN)
        s.haupt("Die Shell der CLI fuehrt das Buendel ohne Rueckfrage aus: es liefert alle Zeilen", len(L.buendel) >= erwartet,
                buendel_zeilen=len(L.buendel), buendel_erwartet=erwartet)
        bn, bw = L.buendel.get("s1_route_namen"), L.buendel.get("s1_route_wert")
        s.haupt("Umgebung des Agenten ohne Routenwert", bn is not None and bw is not None and bn[0] == 0 and bw[0] == 0,
                route_variablen=bn[0] if bn else None, route_werte=bw[0] if bw else None,
                umgebung_variablen=(L.buendel.get("s1_umgebung") or (None,))[0])
    else:
        z = _zug_gemessen(L, 1, L.befehl, sid, k["befehle"]["env"])
        zeilen_env = [l.strip().lstrip("\u23bf ").strip() for l in z.ausgabe().splitlines()]
        namen = [l.split("=", 1)[0] for l in zeilen_env if re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", l)]
        werte = [l.split("=", 1)[1] for l in zeilen_env if re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", l)]
        route_namen = [n for n in namen if "route" in n.lower()]
        s.haupt("Eingabezeile nimmt Text an", z.rc == 0, rc_env=z.rc)
        _zaehlung_cloud(s, z, "S1", L)
        _ca_vorpruefung(L, sid, z)
        s.haupt("Umgebung des Agenten ohne Routenwert", not route_namen and L.cloud not in werte,
                umgebung_variablen=len(namen), route_variablen=len(route_namen))  # Zahl, nie die Namen aus der Sitzung
    if k["bruecke"].get("pruefe_token", True):
        code, _, _ = L.starten(L.start_dict("cloud"), ohne_token=True, ablehnung=True)
        s.haupt("Anfrage ohne Token abgelehnt", code in (401, 403), status_ohne_token=code)
    # Gegenprobe: Ablehnungen vor dem Start eines Kindes. Ohne Diagnose der Bruecke (echt) misst der Laeufer Sitzungsliste und Kinder des Kerns.
    ohne_diagnose = vor is None
    fenster0 = _kinder_fenster(L) if ohne_diagnose else None
    ohne_route = {k_: v for k_, v in L.start_dict("cloud").items() if k_ != "route"}
    unbekannt = dict(L.start_dict("cloud"), route="gibt-es-nicht")
    c1, a1, _ = L.starten(ohne_route, ablehnung=True)
    c2, a2, _ = L.starten(unbekannt, ablehnung=True)
    for code, antw in ((c1, a1), (c2, a2)):
        if code in (200, 201) and isinstance(antw, dict) and antw.get("sitzung"):
            L.sitzungen.append(antw["sitzung"])
    erw1 = k["erwartete_codes"]["ohne_route"]
    s.gegen("Start ohne Route abgelehnt (A43)", c1 == 422 and isinstance(a1, dict) and a1.get("code") in erw1,
            status_ohne_route=c1, code_ohne_route=_fest_aus(a1.get("code") if isinstance(a1, dict) else None, erw1))
    # Antwort 422 der Bruecke (Vertrag bruecke, Abschnitt 5): `code` start.<code>, `grund` start_pruefung, `text` der feste Text der Stufe.
    # Ueber die Seite (Browser-Modus) liest der Laeufer nur Code und Text; dort fehlt `grund`.
    s.gegen("Unbekannte Route: start.route_unbekannt mit festem Text",
            c2 == 422 and isinstance(a2, dict) and a2.get("code") == "start.route_unbekannt" and a2.get("text") == "Route unbekannt"
            and a2.get("grund", "start_pruefung") == "start_pruefung",
            status_unbekannt=c2, code_unbekannt=_fest_aus(a2.get("code") if isinstance(a2, dict) else None, ("start.route_unbekannt",)),
            text_unbekannt=_fest_aus(a2.get("text") if isinstance(a2, dict) else None, ("Route unbekannt",)))
    nach = L.b.diagnose()
    if ohne_diagnose:
        _gegenprobe_ohne_diagnose(L, s, fenster0)
    elif nach is None:
        s.offen.append("diagnose: Bruecke liefert keine Zaehler fuer Startprogramm und Harness")
    else:
        L.gegenprobe = {"weg": "diagnose", "neue_sitzungen": None, "neue_kinder": None}
        # `vor` wurde vor dem Start der Sitzung gelesen: ein Kind und ein Harness gehoeren zu ihr
        rest = {n: nach.get(n, 0) - vor.get(n, 0) for n in ("kinder_gestartet", "harness_gestartet")}
        s.gegen("Weder Startprogramm noch Harness fuer die abgelehnten Starts gelaufen",
                rest["kinder_gestartet"] == 1 and rest["harness_gestartet"] == 1,
                neue_kinder=rest["kinder_gestartet"], neue_harness=rest["harness_gestartet"])
    if k["ziel"] == "attrappe" and L.max_starts is not None:
        # Der Laeufer weist einen zweiten Cloud-Start selbst ab (copilot.start-limit), bevor die Bruecke ihn sieht. Nur gegen die
        # Attrappen: im echten Lauf wuerde ein durchgehender zweiter Start die CLI wirklich starten. Mutationsprobe
        # `zweiter-cloud-start`: der Zaehler des Laeufers ist aus, der Start geht an die Bruecke, der Teil wird rot.
        if "zweiter-cloud-start" in L.fehler:
            L.max_starts = None
        d0 = L.b.diagnose()
        try:
            c3, a3, _ = L.starten(L.start_dict("cloud"))
            abgewiesen = False
            if c3 in (200, 201) and isinstance(a3, dict) and a3.get("sitzung"):
                L.sitzungen.append(a3["sitzung"])
        except lauf.StartLimit:
            abgewiesen = True
        d1 = L.b.diagnose()
        s.gegen("Ein zweiter Cloud-Start wird mit copilot.start-limit abgewiesen, bevor die Bruecke ihn sieht",
                abgewiesen and (d0 is None or d1 is None or d1.get("kinder_gestartet", 0) == d0.get("kinder_gestartet", 0)),
                zweiter_start_abgewiesen=abgewiesen)
    return s


# --- S2 --------------------------------------------------------------------------------------------------------
def fehlerzeile_im_verlauf(zeile, verlauf):
    """Steht die Fehlerzeile eines Befehls im Terminalverlauf? Verglichen ohne Leerraum und Zeilenwechsel: Das Terminal ist 120 Spalten
    breit, und `logische_zeilen` (bruecke.py) fuegt eine umbrochene Bildzeile nur an, wenn die vorige die ganze Breite fuellt. Faellt der
    Umbruch auf ein Leerzeichen (119 Zeichen und ein Leerzeichen), steht im Verlauf `...No\\nsuch file ...`. Gemessen 21.11.: Eine Zeile von
    138 bis 141 Zeichen (Pfad des Vaults im Befehl) machte S2 je nach Pfad des Baums rot oder gruen, unabhaengig von Last und Zeit."""
    ohne = lambda t: re.sub(r"\s+", "", t)  # noqa: E731
    return bool(zeile) and ohne(zeile) in ohne(verlauf)


def s2(L):
    if L.knapp:
        return nur_attrappe("S2", "Datei lesen und fremde Adresse brauchen zwei weitere Eingaben an die Cloud-Sitzung")
    s = Schritt("S2", "Der Einschluss haelt")
    k = L.k
    nk = os.path.join(L.k["vault"], k["notiz_nk"]["rel"])
    cmd_datei = _fmt(k["befehle"]["datei"], pfad=messen.ohne_marker(nk, L.nk))
    cmd_netz = _fmt(k["befehle"]["netz"], url=L.fremd.url())
    fremd0 = L.fremd.zaehler()
    za = L.befehl(L.s1, cmd_datei)
    zb = L.befehl(L.s1, cmd_netz)
    fremd1 = L.fremd.zaehler()
    s.haupt("Datei lesen schlaegt fehl", za.rc not in (None, 0), rc_datei=za.rc)
    s.haupt("Fremde Adresse schlaegt fehl", zb.rc not in (None, 0), rc_netz=zb.rc)
    if za.ergebnis is not None and zb.ergebnis is not None:
        fehltext = [_text_ohne_rc(z.ergebnis).splitlines()[:1] for z in (za, zb)]
        s.haupt("Fehlertext steht im Terminal",
                all(f and fehlerzeile_im_verlauf(f[0], z.verlauf) for f, z in zip(fehltext, (za, zb))), fehlertext_zeilen=len(fehltext))
    mk_verlauf = messen.zaehle_ci(za.verlauf + zb.verlauf, L.nk)
    mk_mit = None if za.mit is None else messen.zaehle(za.mit + zb.mit, L.nk)
    s.haupt("Zaehler des Markers im Terminalverlauf und im Mitschnitt ist 0", _null(mk_verlauf, mk_mit),
            marker_verlauf=mk_verlauf, marker_mitschnitt=mk_mit)
    s.haupt("Netzgrenze: 0 aufgebaute Verbindungen zur fremden Adresse", fremd1 - fremd0 == 0, verbindungen_im_einschluss=fremd1 - fremd0)
    # Gegenprobe: dieselben Befehle ausserhalb des Einschlusses
    rc_a, out_a = messen.aussen_shell(cmd_datei)
    rc_b, out_b = messen.aussen_shell(cmd_netz)
    fremd2 = L.fremd.zaehler()
    s.gegen("Ausserhalb gelingt das Lesen der Datei", rc_a == 0 and messen.zaehle(out_a, L.nk) > 0,
            rc_datei_aussen=rc_a, marker_aussen=messen.zaehle(out_a, L.nk))
    s.gegen("Ausserhalb ist die fremde Adresse erreichbar", rc_b == 0 and fremd2 - fremd1 >= 1,
            rc_netz_aussen=rc_b, verbindungen_aussen=fremd2 - fremd1)
    return s


# --- S3 --------------------------------------------------------------------------------------------------------
def _keine_grundfelder(d):
    verboten = {"grund", "klasse", "meldung", "darf", "gesperrte"}
    if isinstance(d, dict):
        return not (verboten & set(d)) and all(_keine_grundfelder(v) for v in d.values())
    if isinstance(d, list):
        return all(_keine_grundfelder(v) for v in d)
    return True


def _darf(L, vorgabe, route, kunde, klasse):
    fall = {"name": "drehbuch", "vorgabe": vorgabe, "anfragen": [{"einheit": {"klasse": klasse}, "route": route, "kunde": kunde}]}
    import subprocess
    p = subprocess.run(list(L.k["datentor_befehl"]), input=json.dumps(fall).encode("utf-8"), capture_output=True, timeout=15)
    try:
        return json.loads(p.stdout.decode("utf-8"))["antworten"][0]
    except (ValueError, KeyError, IndexError):
        return None


def _marker_der_kunden(L):
    return [L.nk, L.ns]


def _zaehlung(s, z, kennung):
    """Vergleich der Zaehldatei der Suche mit dem, was der Agent sieht (Nachtrag 9).
    - Bekam der Agent im Zug {"error": "protokoll"}, hat die Datei eine Antwort mit Markern gezaehlt, die ihn nie erreichten: Die
      Positivpruefung aus der Zaehldatei ist dann offen (<schritt>.zaehlung_nicht_vergleichbar). Gesperrte Marker in der Datei
      bleiben rot.
    - Hat die Datei weniger Antworten, als der Agent sieht, ist der Schritt rot (<schritt>.zaehlung_zu_wenig)."""
    kz = kennung.lower()
    for v in z.vorversuche:
        s.offen.append("%s.zaehlung_nicht_vergleichbar: Zaehlung der Suche bei 1 Antwort mit Protokollfehler nicht vergleichbar" % kz)
        s.code_offen = kz + ".zaehlung_nicht_vergleichbar"
    gesehen = z.antworten_gesehen()
    if gesehen is not None and z.suche is not None:
        ok = z.suche["antworten"] >= gesehen
        s.haupt("Die Zaehldatei der Suche hat nicht weniger Antworten, als der Agent sieht", ok, antworten_gezaehlt=z.suche["antworten"],
                antworten_gesehen=gesehen)
        if not ok:
            s.code_rot = kz + ".zaehlung_zu_wenig"


def _zs(z, marker):
    """Zuwachs des Markers in der Zaehldatei der Suche waehrend des Zugs; None ohne Zaehldatei."""
    return None if z.suche is None else z.suche["marker"].get(marker, 0)


def _zw(z, marker):
    """Zuwachs des Markers in der Zaehldatei des Weiterreichers waehrend des Zugs; None ohne Zaehldatei."""
    return None if z.mw is None else z.mw["marker"].get(marker, 0)


def _markerliste(L, s, z, kz):
    """Die wirksame Markerliste gegen die Soll-Liste (`marker.nk`, `ni`, `ns`, `np`): (1) die Schluessel von `marker` in der Zaehldatei des
    Weiterreichers (Vertrag 8: jeder Marker der Liste steht dort, auch mit 0) und (2) `marker` in der Startdatei der Sitzung (start.json:
    `liste` und `ordner`). Eine Abweichung oder eine nicht lesbare Startdatei macht die Zaehlung der Sitzung `offen` mit
    `<schritt>.markerliste_abweichend`, nie gruen. Gibt wahr bei einem Befund."""
    soll = set(L.k["marker"].values())
    gruende = []
    if z.mw_liste is not None and z.mw_liste != soll:
        gruende.append("die Schluessel von marker in der Zaehldatei des Weiterreichers weichen von der Soll-Liste ab (%d fehlen, %d fremd)"
                       % (len(soll - z.mw_liste), len(z.mw_liste - soll)))
    if z.start_marker in ("abweichend", "unlesbar"):
        gruende.append("marker in start.json der Sitzung: %s" % z.start_marker)
    for g in gruende:
        s.offen.append("%s.markerliste_abweichend: %s" % (kz, g))
    if gruende:
        s.code_offen = kz + ".markerliste_abweichend"
    return bool(gruende)


def _zaehlung_cloud(s, z, kennung, L):
    """Zaehldatei des Weiterreichers fuer einen Zug der Cloud-Route (Nachtrag 20.11., 9). Die Modellaufrufe der CLI koennen an einer
    Zaehlung vorbeilaufen (etwa eine Richtung, die der Weiterreicher nicht zaehlt); `anfragen` beweist nichts, weil auch die
    Upgrade-Anfrage eines WebSocket zaehlt. Gruen ist der Cloud-Teil nur, wenn der erlaubte Marker ohne Kundenbezug (`marker.np`,
    im Prompt) im Zug mindestens einmal gezaehlt wurde und die Kundenmarker 0 sind. Zaehlt er nicht (auch ohne Zaehldatei): `offen`
    mit `<schritt>.zaehlung_cloud_unbelegt`. Ein Kundenmarker ueber 0 ist rot. Der Marker N-I ist nur ein Hinweis in den Messwerten."""
    kz = kennung.lower()
    if z.mw is None or L.np is None:
        s.offen.append("%s.zaehlung_cloud_unbelegt: keine Zaehldatei des Weiterreichers oder kein erlaubter Marker (marker.np)" % kz)
        s.code_offen = s.code_offen or kz + ".zaehlung_cloud_unbelegt"
        return
    mk = {"nk": _zw(z, L.nk), "ns": _zw(z, L.ns)}
    s.haupt("Am Weiterreicher der Cloud-Route: Marker der Kunden 0 (Zaehldatei des Weiterreichers)", _null(*mk.values()),
            marker_weiterreicher_cloud=mk)
    if _markerliste(L, s, z, kz):
        return
    erlaubt = _zw(z, L.np)
    s.messwerte.setdefault("hinweis_cloud_weiterreicher", {})[kz] = {"anfragen": z.mw.get("antworten", 0), "erlaubt": erlaubt, "intern": _zw(z, L.ni)}
    if erlaubt < 1:
        s.offen.append("%s.zaehlung_cloud_unbelegt: der erlaubte Marker wurde am Weiterreicher der Cloud-Route im Zug nicht gezaehlt" % kz)
        s.code_offen = s.code_offen or kz + ".zaehlung_cloud_unbelegt"


def _sichtbar(z, marker):
    """Gegenprobe ohne Pruefling: Vorkommen des Markers in dem, was der Agent zeigt (Terminal und Antwort)."""
    return messen.zaehle_ci(z.verlauf, marker) + messen.zaehle_ci(z.antwort_text() or "", marker)


def s3(L):
    if L.knapp:
        return nur_attrappe("S3", "Suche und Gegenprobe in der Cloud-Sitzung brauchen weitere Eingaben")
    s = Schritt("S3", "Die Wissenssuche sperrt die Kundennotizen")
    k = L.k
    soll = int(k["erwartet"]["gesperrt_cloud"])
    kn = _marker_der_kunden(L)
    z = L.suche(L.s1)
    a = z.antwort_json()
    text = z.antwort_text()
    _zaehlung(s, z, "S3")
    _zaehlung_cloud(s, z, "S3", L)
    if z.suche is None:
        s.offen.append("zaehlordner: keine Zaehldateien der Suche (kein Ordner konfiguriert)")
    if z.suche is not None and z.suche["antworten"] < 1:
        s.haupt("Die Suche hat die Anfrage beantwortet (Zaehldatei)", False, antworten=z.suche["antworten"])
    s.haupt("Zahl gesperrter Treffer stimmt (Protokollzeilen der Sitzung; Antwort der Suche, wo sichtbar)",
            z.gesperrt == soll and (a is None or _gesperrt(a) == soll), gesperrt=z.gesperrt, gesperrt_antwort=_fest_zahl(_gesperrt(a)), soll=soll)
    s.haupt("Zwei Protokollzeilen: Grund klasse-ueber-route, Klasse " + L.klasse_hoch + " (darf fuer N-K und N-S)",
            len(z.proto) == soll and all(d.get("grund") == "klasse-ueber-route" and d.get("klasse") == L.klasse_hoch for d in z.proto),
            zeilen_gruende=_fest_codes(d.get("grund") for d in z.proto))
    ni = [v for v in (_zs(z, L.ni), messen.zaehle(text, L.ni) if text is not None else None) if v is not None]
    s.haupt("N-I kommt als Treffer an (Zaehldatei der Suche, Antwort)", bool(ni) and all(v > 0 for v in ni), marker_ni=ni)
    if text is not None:
        s.haupt("Zu N-K und N-S kein Titel, Pfad oder Inhalt, kein Grund (A34, A49)",
                k["notiz_nk"]["rel"] not in text and k["notiz_ns"]["rel"] not in text and (a is None or _keine_grundfelder(a)))
    zk = {"suche_zaehldatei": None if z.suche is None else sum(_zs(z, m) for m in kn),
          "modellweg_bytes": None if z.mit is None else sum(messen.zaehle(z.mit, m) for m in kn)}
    s.haupt("Zaehler der Kunden-Marker ist 0 in der Zaehldatei der Suche und am Modellweg (wo gezaehlt wird)", _null(*zk.values()),
            marker_kunden=zk)
    sichtbar = sum(_sichtbar(z, m) for m in kn)
    s.haupt("Gegenprobe ohne Pruefling: kein Kunden-Marker in der sichtbaren Ausgabe des Agenten", sichtbar == 0,
            marker_kunden_sichtbar=sichtbar)
    if k.get("datentor_befehl"):
        einst = konfig.einstellungen_datei(k, "basis")
        ds = [_darf(L, konfig.vorgabe_fuer_tor(einst), L.cloud, kd, L.klasse_hoch) for kd in (k["kunde"], k["kunde2"])]
        s.haupt("darf (Tor-Programm) fuer N-K und N-S: Grund klasse-ueber-route, Klasse " + L.klasse_hoch,
                all(d and d["darf"] is False and d["grund"] == "klasse-ueber-route" and d["klasse"] == L.klasse_hoch for d in ds),
                darf_gruende=_fest_codes((d or {}).get("grund") for d in ds))
    # Gegenprobe: Route cloud mit der hoechsten Klasse, k-nord darf die Route, keine Freigabe
    variante = L.einstellungen_schreiben("cloud_kundendaten")
    try:
        if k.get("datentor_befehl"):
            d2 = _darf(L, konfig.vorgabe_fuer_tor(variante), L.cloud, k["kunde"], L.klasse_hoch)
            s.gegen("darf (Tor-Programm): Grund kundendaten-an-cloud ohne Freigabe (A50)",
                    bool(d2) and d2["darf"] is False and d2["grund"] == "kundendaten-an-cloud",
                    darf_grund_gegenprobe=_fest_code((d2 or {}).get("grund")))
        # (1) In der ersten Sitzung (immer): Die Sitzung hat ihre Einstellungen beim Start gelesen, eine Aenderung danach oeffnet sie nicht.
        z1 = L.suche(L.s1)
        _zaehlung(s, z1, "S3")
        _zaehlung_cloud(s, z1, "S3", L)
        s.gegen("Gegenprobe (erste Sitzung, Einstellungen danach geaendert): dieselbe Zahl gesperrt, Kunden-Marker weder gezaehlt noch sichtbar",
                z1.gesperrt == soll and sum(_sichtbar(z1, x) for x in kn) == 0
                and _null(*[_zs(z1, x) for x in kn], *[_mz(z1, x) for x in kn]), gesperrt_gegenprobe_erste=z1.gesperrt)
        s.gegen("Gegenprobe: eine Aenderung der Einstellungen nach dem Start oeffnet die laufende Sitzung nicht (weiter klasse-ueber-route)",
                len(z1.proto) == soll and all(d_.get("grund") == "klasse-ueber-route" for d_ in z1.proto),
                gegenprobe_zeilen_erste=len(z1.proto))
        # (2) In einer zweiten Sitzung, wenn die Grenze `probelauf.max_starts` einen zweiten Cloud-Start erlaubt (gegen die Attrappen): eine
        # neue Sitzung liest die geaenderten Einstellungen, und fuer k-nord steht kundendaten-an-cloud (D11) im Protokoll. Im echten Lauf
        # mit einem Start entfaellt die zweite Sitzung: der Teil D11 ist dann `offen` mit `s3.nur_attrappe`, nie gruen.
        if L.max_starts is not None and L.starts >= L.max_starts:
            s.offen.append("s3.nur_attrappe: die Gegenprobe in einer zweiten Cloud-Sitzung (Protokollzeile kundendaten-an-cloud, D11, fuer k-nord) "
                           "braucht einen zweiten Cloud-Start")
            s.code_offen = s.code_offen or "s3.nur_attrappe"
        else:
            m = L.sitzung_starten("cloud")
            if not m["sitzung"] or m["t1"] is None:
                s.gegen("Sitzung der Gegenprobe startet", False, status_gegenprobe=m["status"])
            else:
                z2 = L.suche(m["sitzung"])
                _zaehlung(s, z2, "S3")
                _zaehlung_cloud(s, z2, "S3", L)
                s.gegen("Gegenprobe: dieselbe Zahl gesperrt, Kunden-Marker weder gezaehlt noch sichtbar",
                        z2.gesperrt == soll and sum(_sichtbar(z2, x) for x in kn) == 0
                        and _null(*[_zs(z2, x) for x in kn], *[_mz(z2, x) for x in kn]), gesperrt_gegenprobe=z2.gesperrt)
                s.gegen("Gegenprobe: Protokollzeile von k-nord mit Grund kundendaten-an-cloud",
                        any(d_.get("kunde") == k["kunde"] and d_.get("grund") == "kundendaten-an-cloud" for d_ in z2.proto),
                        gegenprobe_zeilen=len(z2.proto))
                L.beenden(m["sitzung"])
    finally:
        L.einstellungen_schreiben("basis")
    return s


# --- S4 --------------------------------------------------------------------------------------------------------
def s4(L):
    if L.knapp:
        return nur_attrappe("S4", "die Protokollzeilen der Sperre entstehen erst durch die Suche von S3, die im knappen Lauf entfaellt")
    s = Schritt("S4", "Die Sperre steht im Protokoll, die Kette haelt")
    k = L.k
    dateien = L.protokoll_dateien()
    zeilen = L.protokoll_zeilen()
    s.haupt("Protokolldatei vorhanden", bool(dateien) and bool(zeilen), zeilen=len(zeilen))
    if not zeilen:
        return s
    meine = [d for _, d in zeilen if isinstance(d, dict) and d.get("sitzung") == L.s1 and d.get("aktionsname") == "ausgabe.gesperrt"]
    s.haupt("Vier Zeilen ausgabe.gesperrt fuer die Sitzung aus S1 (zwei Suchen aus S3), je Kunde zwei",
            len(meine) == 4 and sorted(str(d.get("kunde")) for d in meine) == sorted([k["kunde"], k["kunde2"]] * 2), zeilen_s1=len(meine))
    gemeinsam = {"route": L.cloud, "klasse": L.klasse_hoch, "grund": "klasse-ueber-route", "anzahl": 1, "sitzung": L.s1,
                 "aktionsname": "ausgabe.gesperrt", "ergebnis": "gesperrt", "fassung": 2}
    s.haupt("Felder beider Zeilen stimmen (Vertrag 5)",
            len(meine) == 4 and all(all(d.get(a) == b for a, b in gemeinsam.items()) and "vorher" in d and "zeit" in d and len(d) == 11
                                    for d in meine),
            zeile_felder=[len(d) for d in meine], zeile_gruende=_fest_codes(d.get("grund") for d in meine),
            zeile_kunden=_fest_codes(d.get("kunde") for d in meine))
    inhalt = b"".join(open(d, "rb").read() for d in dateien)
    wortfrage = re.findall(r"[\wäöüÄÖÜß]+", L.frage)
    zaehler = {"marker": sum(messen.zaehle(inhalt, m) for m in _marker_der_kunden(L)),
               "titel": messen.zaehle(inhalt, k["notiz_nk"]["titel"]) + messen.zaehle(inhalt, k["notiz_ns"]["titel"]),
               "woerter": messen.zaehle_woerter(inhalt, wortfrage)}
    s.haupt("Protokolldatei ohne Marker, Titel und Woerter der Frage (A12, A104)", _null(*zaehler.values()), zaehler_protokoll=zaehler)
    gruen = messen.kette_pruefen(k["kette_befehl"], dateien)
    aus = gruen["ausgabe"] or {}
    letzte = aus.get("letzte")
    s.haupt("Kettenpruefung endet mit 0 und nennt den Hash der letzten Zeile",
            gruen["exit"] == 0 and aus.get("ok") is True and bool(re.match(r"^[0-9a-f]{64}$", letzte or "")),
            kette_exit=gruen["exit"], kette_zeilen=_fest_zahl(aus.get("zeilen")), letzte=(letzte or "")[:12] if re.match(r"^[0-9a-f]{64}$", letzte or "") else None)
    wurzel = messen.neuer_tempordner(prefix="kopie-", dir=L.arbeit)
    try:
        kopien = []
        for i, d in enumerate(dateien):
            ordner = os.path.join(wurzel, str(i))
            os.makedirs(ordner)
            kopien.append(os.path.join(ordner, os.path.basename(d)))
            shutil.copy(d, kopien[-1])
        roh = open(kopien[-1], "rb").read().decode("utf-8").split("\n")[:-1]
        nr_letzte = len(roh)
        roh[-1] = roh[-1].replace('"anzahl":1', '"anzahl":2')
        with open(kopien[-1], "w", encoding="utf-8") as f:
            f.write("\n".join(roh) + "\n")
        r = messen.kette_pruefen(k["kette_befehl"], kopien, letzte=letzte)
        b = (r["ausgabe"] or {}).get("bruch") or {}
        s.gegen("Geaenderte letzte Zeile: mit --letzte Exit 1, letzte_falsch, Zeilennummer",
                r["exit"] == 1 and b.get("grund") == "letzte_falsch" and b.get("zeile") == nr_letzte,
                gegen_exit=r["exit"], gegen_grund=_fest_code(b.get("grund")), gegen_zeile=_fest_zahl(b.get("zeile")))
        ohne = messen.kette_pruefen(k["kette_befehl"], kopien)
        s.gegen("Ohne --letzte bleibt die geaenderte letzte Zeile unentdeckt (Vertrag 5)", ohne["exit"] == 0, ohne_letzte_exit=ohne["exit"])
        if nr_letzte >= 2:
            orig = open(dateien[-1], "rb").read().decode("utf-8").split("\n")[:-1]
            geaendert = [orig[0].replace('"anzahl":1', '"anzahl":2')] + orig[1:]
            with open(kopien[-1], "w", encoding="utf-8") as f:
                f.write("\n".join(geaendert) + "\n")
            r2 = messen.kette_pruefen(k["kette_befehl"], kopien[-1:])
            b2 = (r2["ausgabe"] or {}).get("bruch") or {}
            s.gegen("Geaenderte Zeile mit Nachfolger: kette_gebrochen in der folgenden Zeile",
                    r2["exit"] == 1 and b2.get("grund") == "kette_gebrochen" and b2.get("zeile") == 2,
                    gegen_mitte_grund=_fest_code(b2.get("grund")), gegen_mitte_zeile=_fest_zahl(b2.get("zeile")))
            leer = ["{ " + orig[0][1:]] + orig[1:]
            with open(kopien[-1], "w", encoding="utf-8") as f:
                f.write("\n".join(leer) + "\n")
            r3 = messen.kette_pruefen(k["kette_befehl"], kopien[-1:])
            b3 = (r3["ausgabe"] or {}).get("bruch") or {}
            s.gegen("Eingefuegtes Leerzeichen: nicht_kanonisch in der geaenderten Zeile",
                    r3["exit"] == 1 and b3.get("grund") == "nicht_kanonisch" and b3.get("zeile") == 1,
                    gegen_leer_grund=_fest_code(b3.get("grund")), gegen_leer_zeile=_fest_zahl(b3.get("zeile")))
    finally:
        messen.sicher_loeschen(wurzel)
    return s


# --- S5 --------------------------------------------------------------------------------------------------------
def s5(L):
    s = Schritt("S5", "Das lokale Modell bekommt die Kundennotiz mit Freigabe, nicht die ohne")
    k = L.k
    soll = int(k["erwartet"]["gesperrt_lokal"])
    kn = _marker_der_kunden(L)
    L.einstellungen_schreiben("basis")
    m = L.sitzung_starten("lokal")
    sid = m["sitzung"]
    s.haupt("Zweite Sitzung ueber die Route lokal gestartet", sid is not None and m["t1"] is not None, status=m["status"])
    if not sid:
        messen.diagnose_notiere("s5.start", "Start der lokalen Sitzung abgelehnt: %s" % (m["antwort"],))
        s.fehler = "Start der lokalen Sitzung abgelehnt"
        return s
    try:
        z = L.suche(sid, "suche_lokal")
        a = z.antwort_json()
        text = z.antwort_text()
        _zaehlung(s, z, "S5")
        _markerliste(L, s, z, "s5")
        if z.suche is None:
            s.offen.append("zaehlordner: keine Zaehldateien der Suche (kein Ordner konfiguriert)")
        if z.mw is None:
            s.offen.append("zaehlordner: keine Zaehldatei des Weiterreichers")
        s.haupt("Zahl gesperrter Treffer stimmt (nur N-S; Protokollzeilen der Sitzung, Antwort der Suche wo sichtbar)",
                z.gesperrt == soll and (a is None or _gesperrt(a) == soll), gesperrt=z.gesperrt, gesperrt_antwort=_fest_zahl(_gesperrt(a)), soll=soll)
        s.haupt("Fuer die Sitzung entsteht genau eine Zeile ausgabe.gesperrt (lokal, k-sued, route-fuer-kunde-nicht-erlaubt)",
                len(z.proto) == 1 and z.proto[0].get("route") == L.lokal and z.proto[0].get("kunde") == k["kunde2"]
                and z.proto[0].get("grund") == "route-fuer-kunde-nicht-erlaubt", protokollzeilen_s5=len(z.proto))
        zs = {"nk": _zs(z, L.nk), "ni": _zs(z, L.ni), "ns": _zs(z, L.ns)}
        s.haupt("N-K und N-I kommen an, N-S nicht (Zaehldatei der Suche)",
                zs["nk"] is None or (zs["nk"] > 0 and zs["ni"] > 0 and zs["ns"] == 0), marker_suche=zs)
        if text is not None:
            s.haupt("Antwort der Suche: N-K und N-I da, N-S nicht, kein Grund",
                    messen.zaehle(text, L.nk) > 0 and messen.zaehle(text, L.ni) > 0 and messen.zaehle(text, L.ns) == 0
                    and k["notiz_ns"]["rel"] not in text and (a is None or _keine_grundfelder(a)))
        # Der Weiterreicher meldet den Probelauf beim Start: stderr-Zeile (nur Attrappen) und Zeile in gateway-<JJJJ-MM>.jsonl
        err = k.get("weiterreicher", {}).get("stderr_datei")
        if err:
            try:
                with open(err, encoding="utf-8") as f:
                    s.haupt("Weiterreicher meldet beim Start auf stderr: mw.probelauf: Marker werden gezaehlt",
                            "mw.probelauf: Marker werden gezaehlt" in f.read())
            except OSError:
                s.haupt("Weiterreicher meldet beim Start auf stderr: mw.probelauf: Marker werden gezaehlt", False)
        gz = []
        for d in sorted(glob.glob(os.path.join(k["protokoll"]["ordner"], "gateway-*.jsonl"))):
            gz += [x for _, x in messen.protokollzeilen(d)]
        s.haupt("gateway-<JJJJ-MM>.jsonl hat eine Zeile probelauf.zaehlung",
                any(isinstance(x, dict) and x.get("aktionsname") == "probelauf.zaehlung" for x in gz), gateway_zeilen=len(gz))
        zw = {"nk": _zw(z, L.nk), "ns": _zw(z, L.ns)}
        s.haupt("Am Weiterreicher: Marker von N-S 0 (Zaehldatei des Weiterreichers)", _null(zw["ns"]), marker_weiterreicher=zw)
        s.haupt("Gegenprobe ohne Pruefling: Marker von N-S nicht in der sichtbaren Ausgabe", _sichtbar(z, L.ns) == 0,
                marker_ns_sichtbar=_sichtbar(z, L.ns))
        muster = k.get("antwort_muster_lokal", [])
        fehlt = [p for p in muster if not re.search(p, z.verlauf, re.I)]
        s.haupt("Antwort des Modells nennt den Bezug auf N-K", not fehlt, antwort_muster_fehlen=fehlt)
        if L.knapp:
            s.offen.append("s5.nur_attrappe: die Gegenprobe in der ersten Cloud-Sitzung (A48) braucht eine weitere Eingabe an sie")
            s.code_offen = s.code_offen or "s5.nur_attrappe"
        else:
            z1 = L.suche(L.s1)
            a1 = z1.antwort_json()
            _zaehlung(s, z1, "S5")
            _zaehlung_cloud(s, z1, "S5", L)
            s.gegen("Erste Sitzung: weiter dieselbe Zahl gesperrt, Kunden-Marker weder gezaehlt noch sichtbar (A48)",
                    z1.gesperrt == int(k["erwartet"]["gesperrt_cloud"]) and (a1 is None or _gesperrt(a1) == z1.gesperrt)
                    and sum(_sichtbar(z1, x) for x in kn) == 0 and _null(*[_zs(z1, x) for x in kn], *[_mz(z1, x) for x in kn]),
                    gesperrt_sitzung1=z1.gesperrt)
    finally:
        L.beenden(sid)
    # Gegenprobe: bekommt k-sued die Route lokal, kommt N-S an
    L.einstellungen_schreiben("ks_lokal")
    try:
        m2 = L.sitzung_starten("lokal")
        if not m2["sitzung"] or m2["t1"] is None:
            s.gegen("Sitzung der Gegenprobe startet", False, status_gegenprobe=m2["status"])
        else:
            try:
                z2 = L.suche(m2["sitzung"], "suche_lokal")
                ns2 = _zs(z2, L.ns)
                if ns2 is None:
                    ns2 = messen.zaehle(z2.antwort_text() or z2.verlauf, L.ns)
                s.gegen("Mit Freigabe fuer k-sued kommt N-S an, nichts gesperrt", z2.gesperrt == 0 and ns2 > 0,
                        gesperrt_mit_freigabe=z2.gesperrt, marker_ns_mit_freigabe=ns2)
            finally:
                L.beenden(m2["sitzung"])
    finally:
        L.einstellungen_schreiben("basis")
    return s


# --- E1 --------------------------------------------------------------------------------------------------------
def e1(L):
    if L.knapp:
        return nur_attrappe("E1", "Umgebung, Export, Suche und Rohanfrage brauchen vier weitere Eingaben an die Cloud-Sitzung")
    s = Schritt("E1", "Route haengt am Start")
    k = L.k
    kn = _marker_der_kunden(L)
    ze = L.befehl(L.s1, k["befehle"]["env"])
    namen = [l.strip().lstrip("\u23bf ").split("=", 1)[0] for l in ze.ausgabe().splitlines() if "=" in l]
    ziele = sorted({n for n in namen if "route" in n.lower() and re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", n)} | set(k["befehle"].get("route_namen", [])))
    zx = L.befehl(L.s1, "export " + " ".join("%s=%s" % (n, L.lokal) for n in ziele))
    s.haupt("Routenvariablen auf lokal gesetzt", zx.rc == 0, gesetzte_variablen=len(ziele))  # Zahl, nie die Namen aus der Sitzung
    z = L.suche(L.s1)
    a = z.antwort_json()
    _zaehlung(s, z, "E1")
    _zaehlung_cloud(s, z, "E1", L)
    s.haupt("Ergebnis wie in S3: gleiche Zahl gesperrt, Zaehler der Kunden-Marker 0 (Zaehldatei, Modellweg, sichtbare Ausgabe)",
            z.gesperrt == int(k["erwartet"]["gesperrt_cloud"]) and (a is None or _gesperrt(a) == z.gesperrt)
            and sum(_sichtbar(z, x) for x in kn) == 0 and _null(*[_zs(z, x) for x in kn], *[_mz(z, x) for x in kn]),
            gesperrt=z.gesperrt, marker_kunden_sichtbar=sum(_sichtbar(z, x) for x in kn))
    roh = json.dumps({"op": "search", "query": L.frage, "route": L.lokal})
    skript = ("import socket;s=socket.socket(socket.AF_UNIX);s.connect('%s');s.sendall(b'%s\\n');print(s.makefile().readline())"
              % (k["befehle"]["such_socket"], roh))
    zr = L.befehl(L.s1, _fmt(k["befehle"]["rohanfrage"], skript=shlex.quote(skript)))
    zeilen = [l.strip().lstrip("\u23bf ") for l in _text_ohne_rc(zr.ausgabe()).splitlines()]
    zeilen = [l for l in zeilen if l.startswith("{")]
    try:
        antw = json.loads(zeilen[-1]) if zeilen else None
    except ValueError:
        antw = None
    s.haupt("Anfrage mit eigenem Feld route wird abgelehnt (bad_request)",
            isinstance(antw, dict) and antw.get("error") == "bad_request" and "hits" not in antw,
            roh_antwort_error=_fest_aus((antw or {}).get("error") if isinstance(antw, dict) else None, ("bad_request",)))
    return s


# --- E2 --------------------------------------------------------------------------------------------------------
def e2(L):
    if L.knapp:
        return nur_attrappe("E2", "der Dateibaum braucht eine weitere Eingabe an die Cloud-Sitzung")
    s = Schritt("E2", "Notizen liegen ausserhalb")
    k = L.k
    muster = messen.ohne_marker(L.nk, L.nk)
    z = L.befehl(L.s1, _fmt(k["befehle"]["baum"], muster=muster), frist_s=float(k["grenzen"].get("baum_s", 30)))
    treffer = [l for l in _text_ohne_rc(z.ausgabe()).splitlines() if l.strip().lstrip("\u23bf ").startswith("/")]
    _zaehlung_cloud(s, z, "E2", L)
    s.haupt("Im sichtbaren Dateibaum: 0 Treffer", z.rc is not None and not treffer, treffer_im_einschluss=len(treffer), rc_grep=z.rc)
    rc, out = messen.aussen_shell(_fmt(k["befehle"]["baum_aussen"], muster=muster, vault=k["vault"]), frist_s=60)
    n = len([l for l in out.splitlines() if l.strip()])
    s.gegen("Ausserhalb des Einschlusses: Treffer groesser 0", rc == 0 and n > 0, treffer_aussen=n)
    return s


# --- E3 --------------------------------------------------------------------------------------------------------
def e3(L):
    if L.knapp:
        return nur_attrappe("E3", "die Prozessliste braucht weitere Eingaben an die Cloud-Sitzung")
    s = Schritt("E3", "Suche ist ein eigener Prozess")
    k = L.k
    sp = k["such_prozess"]
    z = L.befehl(L.s1, k["befehle"]["ps"])
    sichtbar = [l.split() for l in _text_ohne_rc(z.ausgabe()).splitlines() if sp["name_im_einschluss"] in l and re.match(r"^\W*\d+\s", l)]
    s.haupt("Prozessliste gelesen", z.rc == 0, rc_ps=z.rc)
    _zaehlung_cloud(s, z, "E3", L)
    if not sichtbar:
        s.haupt("Such-Prozess im Einschluss nicht sichtbar (eigener PID-Namensraum)", True, such_prozess_sichtbar=False)
    else:
        pid = sichtbar[0][0]
        zk = L.befehl(L.s1, _fmt(k["befehle"]["kill"], pid=pid))
        zu = L.befehl(L.s1, _fmt(k["befehle"]["umgebung_lesen"], pid=pid))
        s.haupt("Signal an den Such-Prozess scheitert", zk.rc not in (None, 0), such_prozess_sichtbar=True, rc_signal=zk.rc)
        s.haupt("Umgebung des Such-Prozesses nicht lesbar", zu.rc not in (None, 0) or not _text_ohne_rc(zu.ergebnis), rc_umgebung=zu.rc)
    # Gegenprobe: ohne die Grenze des Einschlusses ist der Such-Prozess erreichbar
    import subprocess
    p = subprocess.run(["pgrep", "-f", sp["muster_aussen"]], capture_output=True, timeout=10)
    pids = [x for x in p.stdout.decode().split() if x.isdigit()]
    if not pids:
        s.gegen("Such-Prozess ausserhalb des Einschlusses gefunden", False, such_prozess_aussen=0)
    else:
        rc = subprocess.run(["kill", "-0", pids[0]], capture_output=True, timeout=10).returncode
        s.gegen("Ausserhalb des Einschlusses gelingt das Signal (kill -0)", rc == 0, rc_signal_aussen=rc)
    return s


# --- E4 --------------------------------------------------------------------------------------------------------
def e4_lokal(L):
    s = Schritt("E4-lokal", "Der Schluessel des Gateways bleibt draussen (lokal)")
    k = L.k
    g = k["gateway"]
    muster = g["schluessel_muster"]
    m = L.sitzung_starten("lokal")
    sid = m["sitzung"]
    s.haupt("Lokale Sitzung fuer die Pruefung gestartet", sid is not None and m["t1"] is not None, status=m["status"])
    if not sid:
        return s
    try:
        def zahl(z):
            t = _text_ohne_rc(z.ergebnis if z.ergebnis is not None else z.verlauf)
            f = re.findall(r"^\W*(\d+)\s*$", t, re.M)
            return int(f[-1]) if f else None
        z_env = L.befehl(sid, _fmt(k["befehle"]["env_zaehle_schluessel"], muster=muster))
        z_ps = L.befehl(sid, _fmt(k["befehle"]["ps_zaehle_schluessel"], muster=muster))
        # Positivkontrolle der Umgebungssuche: `PI_OFFLINE` setzt das Programm im Harness `pi` immer auf 1 (Vertrag 6 `rust/spec/start/README.md`,
        # Abschnitt pi und Abschnitt Starter "Umgebung fuer pi"). Gezaehlt wird nur der Name, nie ein Wert. Erwartet >= 1.
        kontrolle = zahl(L.befehl(sid, k["befehle"]["kontrolle_lokal_env"]))
        s.messwerte.update(kontrolle_lokal_env=kontrolle)
        blind = kontrolle is None or kontrolle < 1
        if blind:
            s.offen.append("e4-lokal.kontrolle_blind: die Suche in der Umgebung der lokalen Sitzung zaehlt PI_OFFLINE nicht, sie ist blind")
            s.code_offen = "e4-lokal.kontrolle_blind"
        z_dat = L.befehl(sid, _fmt(k["befehle"]["dateien_schluessel"], muster=muster), frist_s=float(k["grenzen"].get("baum_s", 30)))
        treffer_dat = [l for l in _text_ohne_rc(z_dat.ausgabe()).splitlines() if l.strip().lstrip("\u23bf ").startswith("/")]
        gruen_ok = zahl(z_env) == 0 and zahl(z_ps) == 0 and not treffer_dat and z_dat.rc is not None
        if not (blind and gruen_ok):  # ein blinder Weg behauptet keinen gruenen Teil; ein Treffer bleibt rot
            s.haupt("Schluessel nicht in Umgebung, Argumenten und Dateien des Einschlusses", gruen_ok,
                    schluessel_umgebung=zahl(z_env), schluessel_argumente=zahl(z_ps), schluessel_dateien=len(treffer_dat))
        if L.gateway_schluessel:
            sichtbar = sum(messen.zaehle(z.verlauf, L.gateway_schluessel) for z in (z_env, z_ps, z_dat))
            s.haupt("Bekannter Schluessel kommt im Terminal nicht vor", sichtbar == 0, schluessel_sichtbar=sichtbar)
        if g.get("url"):
            z_dir = L.befehl(sid, _fmt(k["befehle"]["gateway_direkt"], url=g["url"]))
            code = re.findall(r"^\W*(\d{3})\s*$", _text_ohne_rc(z_dir.ausgabe()), re.M)
            s.haupt("Direkter Aufruf des Gateways scheitert: Netzgrenze oder Ablehnung ohne Schluessel",
                    z_dir.rc is not None and (z_dir.rc != 0 or (code and code[-1] in ("401", "403"))),
                    rc_direkt=z_dir.rc, http_direkt=code[-1] if code else None)
        else:
            s.offen.append("gateway.url: Adresse des Gateways fehlt, direkter Aufruf nicht geprueft")
    finally:
        L.beenden(sid)
    if k["ziel"] == "echt" and not g.get("suche"):
        # Das Muster des Schluessels ist die zweite, schwaechere Probe: Passt der echte Schluessel nicht dazu, findet sie nichts.
        # Massgeblich ist die Suche nach dem genauen Wert ausserhalb des Einschlusses (Nachtrag vom 20.11.2026); sie ist nicht gebaut.
        s.offen.append("gateway.genauer_wert: die Suche nach dem genauen Wert des Schluessels fehlt, das Muster allein ergibt kein Gruen")
    # Gegenprobe ausserhalb des Einschlusses
    import urllib.error
    import urllib.request
    def post(url):
        r = urllib.request.Request(url.rstrip("/") + "/v1/chat/completions", data=b"{}", method="POST",
                                   headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(r, timeout=10) as a:
                return a.status
        except urllib.error.HTTPError as e:
            return e.code
        except OSError:
            return None
    if g.get("url") and g.get("weiterreicher_url"):
        c_dir, c_wr = post(g["url"]), post(g["weiterreicher_url"])
        s.gegen("Ausserhalb: das Gateway lehnt ohne Schluessel ab, der Weiterreicher mit Schluessel gelingt",
                c_dir in (401, 403) and c_wr == 200, gateway_ohne_schluessel=c_dir, weiterreicher=c_wr)
    else:
        s.offen.append("gateway.weiterreicher_url: Gegenprobe ausserhalb des Einschlusses nicht moeglich")
    return s


# --- Suche nach dem genauen Schluessel (Vertrag 8, Abschnitt 9a) -------------------------------------------------
def _zielliste_nach_suche(L, pfad, sid, art_):
    """Auflage vom 21.11.2026: Im echten Lauf entfernt der Laeufer die Zielliste, die die Suche gelesen hat, direkt nach der Suche, und nur
    fuer die Cloud-Sitzung, die dieser Lauf selbst gestartet hat (`sid` ist `L.s1`). Gegen die Attrappen geschieht nichts. Lief die
    Suche nicht (`art_` ist weder `gruen` noch `rot`), bleibt die Datei liegen. Gibt die festen Meldungen des Laufs (Nachtrag 1, 21.11.):
    sie stehen nicht im Schritt und veraendern das Urteil von E4-cloud nicht."""
    if L.k["ziel"] != "echt" or sid is None or sid != getattr(L, "s1", None) or L.art_von.get(sid) != "cloud":
        return []
    if art_ not in ("gruen", "rot"):
        return [messen.ZIELLISTE_SUCHE_NICHT_GELAUFEN] if os.path.lexists(pfad) else []
    _, hinweis = messen.zielliste_entfernen(pfad, sid)
    return [hinweis] if hinweis else []


def _suche_route(L, s, art, quelle_option, quelle, ins, schluessel_datei_fuer_verweis, programm, kennung, nach_suche=None):
    """Eine Suche je Route und die Gegenprobe mit dem Verweis. Ergaenzt den Schritt `s` (E4-lokal oder E4-cloud). `nach_suche(art)` laeuft
    nach der Suche UND nach ihrer Gegenprobe, auch wenn die Gegenprobe mit einer Ausnahme endet: Die Gegenprobe ruft die Suche noch einmal
    mit derselben Quelle auf (`--zielliste <datei>`), die Quelle muss bis dahin liegen bleiben (Testlauf vom 21.11.2026: in einer frueheren Fassung lief
    `nach_suche` vor der Gegenprobe; die Zielliste fehlte ihr, E4-cloud wurde `e4-cloud.schluessel_suche_fehler`)."""
    k = L.k
    frist = float(k["grenzen"].get("suche_s", 60))
    art_, zahlen = messen.schluessel_suche(programm, quelle_option, quelle, ins, frist)
    try:
        _suche_werten(s, art, art_, zahlen, quelle_option, quelle, schluessel_datei_fuer_verweis, programm, kennung, frist)
    finally:
        if nach_suche:
            nach_suche(art_)


def _suche_werten(s, art, art_, zahlen, quelle_option, quelle, schluessel_datei_fuer_verweis, programm, kennung, frist):
    """Urteil der Suche und die Gegenprobe mit dem Verweis (Teil von `_suche_route`)."""
    w = {"suche_%s_art" % art: art_}
    if zahlen:
        w.update({"suche_%s_%s" % (art, n): v for n, v in zahlen.items()})
    name = "Der genaue Schluessel steht nicht in Arbeitsordnern, Protokoll, Zaehlordner und sichtbarer Ausgabe (Suche ausserhalb des Einschlusses)"
    if art_ == "gruen":
        s.haupt(name, True, **w)
    elif art_ == "rot":
        s.haupt(name, False, **w)
    else:
        s.messwerte.update(w)
        s.offen.append("schluessel_suche: die Suche nach dem genauen Schluessel (%s) ist nicht auswertbar" % art)
        s.code_offen = s.code_offen or "%s.schluessel_suche_%s" % (kennung.lower(), "unlesbar" if art_ == "unlesbar" else "fehler")
    # Gegenprobe des Laeufers: ein Verweis auf die Schluesseldatei darf nie als Fund zaehlen (S3). Der Verweis liegt in einem eigenen
    # Ordner 0700 ausserhalb jedes Arbeitsordners; die Suche bekommt nur ihn.
    if schluessel_datei_fuer_verweis:
        ordner = messen.neuer_tempordner(prefix="wbv-")
        try:
            os.chmod(ordner, 0o700)
            os.symlink(schluessel_datei_fuer_verweis, os.path.join(ordner, "verweis"))
            g_art, g_z = messen.schluessel_suche(programm, quelle_option, quelle, [os.path.realpath(ordner)], frist)
        except OSError:
            g_art, g_z = "fehler", None
        finally:
            messen.sicher_loeschen(ordner)
        gw = {"verweis_%s_art" % art: g_art}
        if g_z:
            gw.update({"verweis_%s_%s" % (art, n): v for n, v in g_z.items()})
        if g_art == "gruen" and g_z["uebersprungen"] == 1 and g_z["dateien"] == 0:
            s.gegen("Gegenprobe: ein Verweis auf die Schluesseldatei zaehlt nicht als Fund (0 Treffer, 1 uebersprungen)", True, **gw)
        elif g_art == "rot":
            s.gegen("Gegenprobe: ein Verweis auf die Schluesseldatei zaehlt nicht als Fund (0 Treffer, 1 uebersprungen)", False, **gw)
        else:
            s.gegenwerte.update(gw)
            s.offen.append("schluessel_suche: die Gegenprobe mit dem Verweis ist nicht auswertbar (%s)" % art)
            s.code_offen = s.code_offen or "%s.schluessel_suche_fehler" % kennung.lower()


def suche_nachtrag(L, objekte):
    """Nach allen Schritten und nach dem Ende der Sitzungen: je Route eine Suche nach dem genauen Schluessel (Nachtrag 18). E4-lokal
    wird nur bei Exit 0 der lokalen Suche gruen, E4-cloud nur bei Exit 0 der Cloud-Suche. Die Ausgabedatei (alles, was der Laeufer
    von den Sitzungen gesehen hat) liegt 0600 in einem eigenen Ordner 0700 ausserhalb jedes Arbeitsordners, wird mit O_EXCL angelegt und
    in jedem Fall geloescht; von ihr kommt kein Teil in einen Bericht. Ohne Konfiguration `gateway.suche` tut die Funktion nichts."""
    k = L.k
    cfg = k["gateway"].get("suche")
    ziele = [(n, objekte[n]) for n in ("E4-lokal", "E4-cloud") if n in objekte and not objekte[n].uebersprungen and objekte[n].fehler is None]
    if not cfg or not ziele:
        return []
    from . import ablauf
    programm = list(cfg["programm"])
    if k["ziel"] == "attrappe" and L.fehler:
        programm += ["--fehler", ",".join(L.fehler)]
    hinweise = []  # feste Meldungen des Laufs (Zielliste nach der Suche), nicht Teil eines Schritts
    ordner = None
    try:
        ordner = messen.neuer_tempordner(prefix="wbs-")
        os.chmod(ordner, 0o700)
        if any(konfig.unter(a, ordner) for a in ablauf.arbeitsordner(k)):
            raise OSError("Ordner liegt in einem Arbeitsordner")
        ausgabe = os.path.join(ordner, "ausgabe.txt")
        fd = os.open(ausgabe, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8", errors="replace") as f:
            f.write("\n".join(L.gesehen) + "\n")
        ins = []
        for p in ablauf.arbeitsordner(k) + [os.path.realpath(k["protokoll"]["ordner"])]:
            if p not in ins:
                ins.append(p)
        zo = k.get("zaehlordner")
        if zo:
            ins.append(os.path.realpath(zo))
        ins.append(os.path.realpath(ausgabe))
        for kennung, s in ziele:
            if kennung == "E4-lokal":
                datei = cfg.get("schluessel_datei")
                if not datei or str(datei).startswith("OFFEN"):
                    s.offen.append("schluessel_suche: Ort der Schluesseldatei fehlt")
                    s.code_offen = s.code_offen or "e4-lokal.schluessel_suche_fehler"
                    continue
                _suche_route(L, s, "lokal", "--schluessel-datei", datei, ins, datei, programm, kennung)
            else:
                zl = cfg.get("zielliste")
                sid_zl = None  # Kennung der Cloud-Sitzung, zu der die Zielliste gehoert (nur bei `{sitzung}` im Ort)
                if zl and "{sitzung}" in str(zl):  # echter Ort je Sitzung (Entscheidung vom 20.11.2026): Kennung der ersten Cloud-Sitzung
                    sid = getattr(L, "s1", None)
                    zl = str(zl).replace("{sitzung}", sid) if sid and re.match(r"^[A-Za-z0-9._-]{1,64}$", sid) and sid not in (".", "..") else None
                    sid_zl = sid if zl else None
                if not zl or str(zl).startswith("OFFEN") or not os.path.isfile(zl):
                    s.offen.append("schluessel_suche: Ort der Zielliste der Cloud-Sitzung fehlt")  # Vertrag 8 nennt ihn nicht (OPEN)
                    s.code_offen = s.code_offen or "e4-cloud.schluessel_suche_fehler"
                    continue
                verweis = None
                try:  # Pfad einer Schluesseldatei aus der Zielliste (die Zielliste enthaelt keine Schluessel)
                    with open(zl, encoding="utf-8") as f:
                        verweis = next(z["schluessel_datei"] for z in json.load(f)["ziele"] if z.get("schluessel_datei"))
                except (OSError, ValueError, StopIteration, KeyError, TypeError):
                    pass
                _suche_route(L, s, "cloud", "--zielliste", zl, ins, verweis, programm, kennung,
                             nach_suche=lambda art_, zl=zl, sid_zl=sid_zl: hinweise.extend(_zielliste_nach_suche(L, zl, sid_zl, art_)))
    except OSError:
        for kennung, s in ziele:
            s.offen.append("schluessel_suche: Ausgabedatei nicht sicher anlegbar")
            s.code_offen = s.code_offen or "%s.schluessel_suche_fehler" % kennung.lower()
    finally:
        if ordner:
            messen.sicher_loeschen(ordner)
    return hinweise


# --- E4 cloud -------------------------------------------------------------------------------------------------
def _zahl(z):
    t = _text_ohne_rc(z.ausgabe())
    f = re.findall(r"^\W*(\d+)\s*$", t, re.M)
    return int(f[-1]) if f else None


def e4_cloud(L):
    s = Schritt("E4-cloud", "Das Token der CLI bleibt draussen (cloud)")
    k = L.k
    c = k["copilot"]
    if L.ca["cli_nimmt_an"] is False:
        s.offen.append("ca.cli_abgelehnt: die CLI im Einschluss lehnt den Abschluss mit der Sitzungs-CA ab")
        s.code_fest = "ca.cli_abgelehnt"
        return s
    if L.ca["ausgang"] is False:
        s.offen.append("ca.ausgang_blockiert: der Ausgang des Wirts zum Ziel ist gesperrt")
        s.code_fest = "ca.ausgang_blockiert"
        return s
    if L.knapp and L.buendel is None:
        # S1 endete vor dem Buendel (etwa s1.zug_frist oder s1.antwort_ohne_merkmal). Der Grund ist das fehlende Buendel; eine CA, deren
        # Vorpruefung (nach Zug 1) nicht lief, waere hier eine irrefuehrende Meldung.
        s.offen.append("e4-cloud.offen: das Buendel der Cloud-Sitzung aus S1 liegt nicht vor")
        return s
    if L.ca["cli_nimmt_an"] is None or L.ca["ausgang"] is None:
        s.offen.append("ca: Vorpruefung nicht gelaufen (keine Zaehldatei des Weiterreichers oder kein Ausgangsbefehl)")
        return s
    sid = L.s1
    ziel0 = L.ziel0 if L.knapp else (L.zinfo["ziel_zaehler"]() if L.zinfo.get("ziel_zaehler") else None)
    vorlagen = TOKENPRUEFUNGEN
    zaehler, zuege = {}, []
    if L.knapp:  # die Pruefungen liefen im einen Buendel von S1 (Eingabe 2)
        for n in vorlagen:
            v = L.buendel.get("e4c_" + n)
            zaehler[n] = v[0] if v else None
        zuege.append(L.buendel_zug)
        kontrolle = (L.buendel.get(BUENDEL_KONTROLLE) or (None,))[0]
        kontrolle_proc = (L.buendel.get(BUENDEL_KONTROLLE_PROC) or (None,))[0]
    else:
        for n in vorlagen:
            z = L.befehl(sid, _fmt(k["befehle"][n], muster=c["token_muster"], platzhalter=c["platzhalter"]),
                         frist_s=float(k["grenzen"].get("baum_s", 30)) if n == "token_dateien" else None)
            zaehler[n] = _zahl(z)
            zuege.append(z)
    if L.knapp:
        # Zug 2 ist eine Shell-Eingabe der CLI ohne Modellanfrage: Die Route der Cloud-Sitzung belegt die Zaehlung von Zug 1 (die eine
        # Modellanfrage der Sitzung). Fuer Zug 2 gilt weiter: Kundenmarker 0 am Weiterreicher. Die Zahl seiner Anfragen im Zug 2 steht als
        # Messwert da (erwartet 0: das Buendel ging nicht an das Modell).
        _zaehlung_cloud(s, L.zug1, "E4-cloud", L)
    else:
        _zaehlung_cloud(s, zuege[-1], "E4-cloud", L)
    if not L.knapp:  # voller Lauf: die Positivkontrolle ist eine eigene Eingabe; im knappen Lauf kam sie im Buendel von S1
        kontrolle = _zahl(L.befehl(sid, _fmt(k["befehle"][KONTROLLE_ENV], platzhalter=ohne_tokenpraefix(c["platzhalter"]))))
        kontrolle_proc = _zahl(L.befehl(sid, _fmt(k["befehle"][KONTROLLE_PROC], platzhalter=ohne_tokenpraefix(c["platzhalter"]))))
    s.messwerte.update(kontrolle_platzhalter=kontrolle, kontrolle_proc=kontrolle_proc)
    blind_env = kontrolle is None or kontrolle < 1
    blind_proc = kontrolle_proc is None or kontrolle_proc < 1
    blind = blind_env or blind_proc
    if blind_proc:  # eigener Grund fuer `token_proc`; derselbe feste Code
        s.offen.append("e4-cloud.kontrolle_blind: token_proc liest den Platzhalter in den Prozessen der Cloud-Sitzung nicht, der Weg ist blind")
        s.code_offen = "e4-cloud.kontrolle_blind"
    if blind_env:
        # Positivkontrolle: Der Platzhalter steht in der Umgebung der CLI, der Weg von `token_env` muss ihn also zaehlen (>= 1). Bei 0 ist
        # die Suche in der Umgebung blind, und "kein Treffer des Tokens" sagt nichts: der Schritt ist `offen` mit festem Code, nie gruen.
        # Der Code steht als `code_offen`: Ein Treffer des Tokens in einer anderen Pruefung macht den Schritt weiter rot (eigener Teilcode).
        s.offen.append("e4-cloud.kontrolle_blind: die Suche in der Umgebung der Cloud-Sitzung zaehlt den Platzhalter nicht, sie ist blind")
        s.code_offen = "e4-cloud.kontrolle_blind"
    if not (blind and all(v == 0 for v in zaehler.values())):  # ein blinder Weg behauptet keinen gruenen Teil
        s.haupt("Kein Treffer des Tokens ausser dem Platzhalter: Umgebung, Argumente, Prozesse, Dateien, gh, secret-tool",
                all(v == 0 for v in zaehler.values()), treffer=zaehler)
    marke = L.zinfo.get("marke")
    if marke:
        sichtbar = sum(messen.zaehle(z_.verlauf, marke) for z_ in zuege)
        s.haupt("Gegenprobe ohne Pruefling: die Marke des echten Tokens steht nicht im Terminal", sichtbar == 0, marke_sichtbar=sichtbar)
    if c.get("ziel_url") and L.knapp:
        s.offen.append("e4-cloud.nur_attrappe: der direkte Aufruf des Ziels aus der Cloud-Sitzung braucht eine weitere Eingabe")
        s.code_offen = s.code_offen or "e4-cloud.nur_attrappe"
    elif c.get("ziel_url"):
        zd = L.befehl(sid, _fmt(k["befehle"]["cloud_direkt"], url=c["ziel_url"], platzhalter=c["platzhalter"]))
        code = re.findall(r"^\W*(\d{3})\s*$", _text_ohne_rc(zd.ausgabe()), re.M)
        s.haupt("Direkter Aufruf des Ziels ohne Weiterreicher scheitert",
                zd.rc is not None and (zd.rc != 0 or (code and code[-1] in ("401", "403"))), rc_direkt=zd.rc, http_direkt=code[-1] if code else None)
    else:
        s.offen.append("copilot.ziel_url: Adresse des Ziels fehlt, direkter Aufruf nicht geprueft")
    if ziel0 is not None:
        s.haupt("Der Zaehler am Ziel steigt (das echte Token kam dort an)", L.zinfo["ziel_zaehler"]() > ziel0,
                ziel_zaehler_zuwachs=L.zinfo["ziel_zaehler"]() - ziel0)
    else:
        s.offen.append("copilot.ziel_zaehler: kein Zaehler am Ziel (nur Attrappen)")
    # Gegenprobe ausserhalb des Einschlusses: der Weiterreicher hat das Token
    if c.get("weiterreicher_url") and c.get("ziel_url"):
        import urllib.error
        import urllib.request

        def post(url, kopf):
            r = urllib.request.Request(url.rstrip("/") + "/v1/chat/completions", data=b"{}", method="POST",
                                       headers=dict({"Content-Type": "application/json"}, **kopf))
            try:
                with urllib.request.urlopen(r, timeout=10) as a:
                    return a.status
            except urllib.error.HTTPError as e:
                return e.code
            except OSError:
                return None
        z_vor = L.zinfo["ziel_zaehler"]() if L.zinfo.get("ziel_zaehler") else None
        direkt = post(c["ziel_url"], {})
        ueber = post(c["weiterreicher_url"], {"Authorization": "Bearer " + c["platzhalter"]})
        s.gegen("Ausserhalb: das Ziel lehnt ohne Token ab, der Weiterreicher hat das Token und sein Aufruf gelingt",
                direkt in (401, 403) and ueber == 200 and (z_vor is None or L.zinfo["ziel_zaehler"]() > z_vor),
                ziel_ohne_token=direkt, weiterreicher=ueber)
    else:
        s.offen.append("copilot.weiterreicher_url: Gegenprobe ausserhalb des Einschlusses nicht moeglich")
    zb = L.buendel_zug if L.knapp else None
    if zb is not None and zb.mw is not None:  # als letzter Teil: die Nummern der Teile davor bleiben wie vorher
        mk2 = {"nk": _zw(zb, L.nk), "ns": _zw(zb, L.ns)}
        s.haupt("Am Weiterreicher im Zug des Buendels: Marker der Kunden 0", _null(*mk2.values()), marker_weiterreicher_buendel=mk2)
        s.messwerte.update(buendel_anfragen_weiterreicher=_fest_zahl(zb.mw.get("antworten", 0)))
    return s


# --- E5 --------------------------------------------------------------------------------------------------------
def _e5_sitzung(L, sid, art, s):
    k = L.k
    gp = k["gastpunkt"]
    anderer = gp["lokal" if art == "cloud" else "cloud"]
    eigen = gp[art]
    br = L.zinfo.get("bruecke_url")
    z1 = L.befehl(sid, _fmt(k["befehle"]["e5_url"], url=br + "/")) if br else None
    z2 = L.befehl(sid, _fmt(k["befehle"]["e5_url"], url="http://127.0.0.1:%d/" % eigen))
    z3 = L.befehl(sid, _fmt(k["befehle"]["e5_url"], url="http://127.0.0.1:%d/" % anderer))
    z4 = L.befehl(sid, _fmt(k["befehle"]["e5_connect"], port=eigen, host=k["netzziel_fremd"]))
    nz = L.befehl(sid, _fmt(k["befehle"]["e5_connect"], port=eigen, host=k["copilot"]["netzziel"]))
    code4 = re.findall(r"^\W*(\d{3})\s*$", _text_ohne_rc(z4.ausgabe()), re.M)
    pre = "[%s] " % art
    if br:
        s.haupt(pre + "Port des Rechners (die Bruecke) ist aus dem Einschluss nicht erreichbar", z1.rc not in (None, 0), rc_bruecke=z1.rc)
    s.haupt(pre + "Der Gastpunkt der eigenen Route ist erreichbar", z2.rc == 0, rc_gastpunkt=z2.rc)
    s.haupt(pre + "Der Gastpunkt der anderen Route gibt es nicht", z3.rc not in (None, 0), rc_anderer_gastpunkt=z3.rc)
    s.haupt(pre + "CONNECT auf ein anderes Ziel scheitert",
            z4.rc is not None and (z4.rc != 0 or (code4 and code4[-1] == "403")), rc_anderes_ziel=z4.rc)
    if art == "cloud":
        s.haupt(pre + "CONNECT auf ein Netzziel der Route gelingt", nz.rc == 0, rc_netzziel=nz.rc)
    else:
        s.haupt(pre + "Der Gastpunkt der Cloud-Route fehlt, CONNECT scheitert", nz.rc not in (None, 0), rc_netzziel_lokal=nz.rc)


def e5(L):
    s = Schritt("E5", "Die Netzgrenze haelt")
    k = L.k
    if L.knapp:
        s.offen.append("e5.nur_attrappe: die Netzgrenze der Cloud-Sitzung braucht fuenf weitere Eingaben an sie")
        s.code_offen = "e5.nur_attrappe"
    else:
        _e5_sitzung(L, L.s1, "cloud", s)
    m = L.sitzung_starten("lokal")
    if m["sitzung"] and m["t1"] is not None:
        try:
            _e5_sitzung(L, m["sitzung"], "lokal", s)
        finally:
            L.beenden(m["sitzung"])
    else:
        s.haupt("Lokale Sitzung fuer die Pruefung gestartet", False, status=m["status"])
    # Gegenprobe ohne Namensraum: derselbe Versuch von aussen erreicht den Port des Rechners
    br = L.zinfo.get("bruecke_url")
    if br:
        rc, out = messen.aussen_shell(_fmt(k["befehle"]["e5_url"], url=br + "/"))
        cd = re.findall(r"\d{3}", out)
        s.gegen("Ohne eigenen Netz-Namensraum erreicht derselbe Versuch den Port des Rechners", rc == 0 and bool(cd) and cd[0] != "000",
                rc_aussen=rc)
    else:
        s.offen.append("bruecke_url: kein Port fuer die Gegenprobe ohne Namensraum")
    return s


# --- E6 --------------------------------------------------------------------------------------------------------
def _warte_bereit(L, sid, harness):
    muster = L.k["harness"][harness]["eingabefeld_muster"]
    ende = time.monotonic() + 5
    while time.monotonic() < ende:
        r = L.b.raster(sid)
        if isinstance(r, dict) and messen.ist_eingabefeld(r.get("zeilen", []), muster):
            return True
        time.sleep(0.05)
    return False


def _versuch(L, start):
    """Ein Start, den die Ordnermarke abweisen muss. Der Start geht an die Bruecke; Lauf starten weist vor jedem Kind ab, die
    Sitzung endet dann mit Code und `grund` in der Sitzungsliste (Vertrag 1: Kindende; Vertrag `dienst` Abschnitt 18). Zaehlt nur als
    Copilot-Start, wenn ein Harness lief."""
    d0 = L.b.diagnose()
    code, antw, _ = L.b.starten(start)
    sid = antw.get("sitzung") if isinstance(antw, dict) else None
    erg = {"sid": sid, "status": code, "abgewiesen": False, "code": None, "harness_neu": None, "laeuft": False}
    if not sid:
        return erg
    L.sitzungen.append(sid)
    L.art_von[sid] = "cloud" if L.k["harness"][start["harness"]]["art"] == "cloud" else "lokal"
    info = None
    # Bis zu 5 s auf das Ende der abgewiesenen Sitzung und ihren Code warten (wie _warte_bereit).
    ende = time.monotonic() + 5
    while time.monotonic() < ende:
        info = L.b.sitzung(sid)
        if isinstance(info, dict) and info.get("zustand") == "beendet":
            break
        time.sleep(0.05)
    erg["abgewiesen"] = isinstance(info, dict) and info.get("zustand") == "beendet" and info.get("code") not in (0, None)
    erg["laeuft"] = isinstance(info, dict) and info.get("zustand") == "laeuft"
    # Der Code von Lauf starten kommt aus der Sitzungsliste: `grund` des Eintrags (Zeile von `lauf`, Vertrag `dienst` DI12 Punkt 5 und
    # Abschnitt 18), nicht mehr aus dem Verlauf (Weg A, Entscheidung vom 23.11.2026: das Bild einer frueh beendeten Sitzung bleibt nur bis zur Neuvergabe
    # ihrer Nummer). Ein Grund in anderer Form wird `abweichend`, ohne Grund steht None.
    grund = info.get("grund") if isinstance(info, dict) and info.get("zustand") == "beendet" else None
    erg["code"] = None if grund is None else messen.fester_code(grund, "abweichend")
    d1 = L.b.diagnose()
    if d0 and d1:
        erg["harness_neu"] = d1.get("harness_gestartet", 0) - d0.get("harness_gestartet", 0)
    if erg["laeuft"] and L.k["harness"][start["harness"]].get("zaehlt_als_start"):
        L.starts += 1
    return erg


def e6(L):
    s = Schritt("E6", "Die Ordnermarke haelt")
    k = L.k
    om = k["ordnermarke"]
    ok_ = om["erwartet"]
    o1 = k["start"]["lokal"]["arbeitsordner"]
    u = os.path.join(o1, "u")
    eltern = os.path.dirname(o1)
    link = os.path.join(eltern, "s")
    datei = os.path.join(o1, "notiz.txt")
    os.makedirs(u, exist_ok=True)
    if not os.path.lexists(link):
        os.symlink(o1, link)
    # Ausgangslage: Eine lokale Sitzung fuer k-nord in O1 schreibt den Marker in eine Datei ihres Ordners.
    m = L.sitzung_starten("lokal")
    if not m["sitzung"] or m["t1"] is None:
        s.haupt("Lokale Sitzung in O1 gestartet", False, status=m["status"])
        return s
    try:
        z = L.befehl(m["sitzung"], _fmt(k["befehle"]["marker_schreiben"], praefix=L.nk[:-1], rest=L.nk[-1], pfad=datei))
        s.haupt("Der Agent schreibt den Marker in eine Datei seines Ordners", z.rc == 0 and os.path.isfile(datei), rc_schreiben=z.rc)
    finally:
        L.beenden(m["sitzung"])
    cloud = dict(L.start_dict("cloud"), kunde=k["kunde"])
    lokal2 = dict(L.start_dict("lokal"), kunde=k["kunde2"])
    versuche = []
    gestoppt = False
    for name, pfad in (("O1", o1), ("O1/u", u), ("Elternordner von O1", eltern), ("Symlink auf O1", link)):
        if gestoppt:
            break
        v = _versuch(L, dict(cloud, arbeitsordner=pfad))
        versuche.append(("copilot " + name, v, ok_["hoeher"]))
        if v["laeuft"] and L.max_starts is not None:
            gestoppt = True  # jeder weitere Versuch waere ein weiterer Start der CLI
    for name, pfad in (("O1", o1), ("O1/u", u)):
        v = _versuch(L, dict(lokal2, arbeitsordner=pfad))
        versuche.append(("lokal fuer k-sued " + name, v, ok_["anderer"]))
    for name, v, code in versuche:
        s.haupt("%s: abgewiesen mit %s" % (name, code), v["abgewiesen"] and v["code"] == code, code_gefunden=_fest_code(v["code"]))
    s.haupt("Kein Kind startet: kein Harness lief fuer die Versuche",
            all((v["harness_neu"] in (0, None)) and not v["laeuft"] for _, v, _ in versuche),
            harness_gestartet=[v["harness_neu"] for _, v, _ in versuche])
    # Gegenprobe im Fehlerfall: Lief ein Versuch, liest diese Sitzung die Datei, und der Zaehler am Modellweg steigt.
    leck = 0
    for name, v, _ in versuche:
        if v["laeuft"]:
            harness = "copilot" if name.startswith("copilot") else "pi"
            harness = [h for h, d in k["harness"].items() if (d["art"] == "cloud") == name.startswith("copilot")][0]
            if _warte_bereit(L, v["sid"], harness):
                L.befehl(v["sid"], _fmt(k["befehle"]["marker_lesen"], pfad=datei))
            st = L.zaehlstand(v["sid"], "modellweg")
            leck += (st or {}).get("marker", {}).get(L.nk, 0)
    zaehler = 0
    for _, v, _ in versuche:
        if v["sid"] and not v["laeuft"]:
            st = L.zaehlstand(v["sid"], "modellweg")
            zaehler += (st or {}).get("marker", {}).get(L.nk, 0)
    sichtbar = sum(messen.zaehle_ci(L.b.verlauf(v["sid"], 0)[1], L.nk) for _, v, _ in versuche if v["sid"] and not v["laeuft"])
    s.haupt("Zaehler von MK-7Q2 an beiden Modellwegen ist 0 (auch nach dem Versuch, die Datei zu lesen), und der Marker steht nicht in der sichtbaren Ausgabe",
            zaehler == 0 and leck == 0 and sichtbar == 0, marker_modellweg=zaehler + leck, marker_sichtbar=sichtbar)
    for _, v, _ in versuche:
        if v["sid"]:
            L.beenden(v["sid"])
    # beschaedigte Marke: fail closed (Vertrag 1, Ordnermarke): `ordnermarken.json` ist kaputt, dann startet keine Sitzung, weder im
    # markierten Ordner noch in einem anderen, unmarkierten. Danach stellt der Laeufer die Datei byte-gleich wieder her.
    zo = om.get("zustandsordner")
    markendatei = os.path.join(zo, "ordnermarken.json") if zo and os.path.isdir(zo) else None
    marke = _marke_von(markendatei, o1)
    if marke:
        orig = open(markendatei, "rb").read()
        o3 = os.path.join(os.path.dirname(eltern), "c", "o3")  # ein Ordner ohne Marke, keine Verwandtschaft mit O1
        os.makedirs(o3, exist_ok=True)
        try:
            with open(markendatei, "wb") as f:
                f.write(b"{kaputt")
            v = _versuch(L, dict(L.start_dict("lokal"), arbeitsordner=o1))
            s.haupt("Beschaedigte Marke: kein Start im markierten Ordner, Code lauf.ordner_marke_kaputt",
                    v["abgewiesen"] and v["code"] == ok_["kaputt"] and not v["laeuft"], code_beschaedigt=_fest_code(v["code"]))
            if v["sid"]:
                L.beenden(v["sid"])
            v3 = _versuch(L, dict(L.start_dict("lokal"), arbeitsordner=o3))
            s.haupt("Beschaedigte Marke: auch in einem anderen, unmarkierten Ordner startet nichts",
                    v3["abgewiesen"] and v3["code"] == ok_["kaputt"] and not v3["laeuft"], code_beschaedigt_anderer=_fest_code(v3["code"]))
            if v3["sid"]:
                L.beenden(v3["sid"])
        finally:
            with open(markendatei, "wb") as f:
                f.write(orig)
        s.haupt("Die Markendatei ist byte-gleich wiederhergestellt", open(markendatei, "rb").read() == orig and bool(orig))
    else:
        s.offen.append("ordnermarke.zustandsordner: Ort der Markendatei oder Marke fuer O1 nicht lesbar, beschaedigte Marke nicht geprueft")
    # Der Mensch loest die Marke mit dem eigenen Befehl, ausserhalb des Einschlusses; Protokollzeile mit der Kennung der Marke
    bef = om.get("loeschen_befehl")
    if bef and marke:
        kennung = marke["kennung"]
        rc = subprocess.run(list(bef) + [o1], capture_output=True, timeout=20).returncode
        weg = _marke_von(markendatei, o1) is None
        pdir = k["protokoll"]["ordner"]
        zeilen = []
        for dd in sorted(glob.glob(os.path.join(pdir, om["protokoll_datei"]))):
            zeilen += messen.protokollzeilen(dd)
        treffer = [(r, x) for r, x in zeilen if isinstance(x, dict) and x.get("aktionsname") == om["protokoll_aktion"]]
        gleich = [x for _, x in treffer if x.get("marke") == kennung]
        s.haupt("Loesen mit dem Befehl: Exit 0, Marke weg, Protokollzeile %s mit der Kennung der Marke" % om["protokoll_aktion"],
                rc == 0 and weg and len(gleich) == 1, rc_loeschen=rc, zeilen_mit_kennung=len(gleich))
        # Die Zeile traegt keinen Pfad und keinen Hash des Pfads
        pfad = os.path.realpath(o1)
        verbogen = [pfad] + [getattr(hashlib, h)(pfad.encode("utf-8")).hexdigest() for h in ("md5", "sha1", "sha256", "sha512")]
        roh_treffer = [r for r, x in treffer if x.get("marke") == kennung]
        s.haupt("Die Zeile traegt weder den Pfad noch einen Hash des Pfads",
                bool(roh_treffer) and not any(w in r for r in roh_treffer for w in verbogen),
                pfad_in_zeile=sum(1 for r in roh_treffer for w in verbogen if w in r))
        kq = messen.kette_quelle_pruefen(k["kette_befehl"], pdir, "ordnermarke")
        s.haupt("kette pruefe-quelle ordnermarke: die Kette ist ganz (Exit 0)",
                kq["exit"] == 0 and isinstance(kq["ausgabe"], dict) and kq["ausgabe"].get("ok") is True,
                kette_quelle_exit=kq["exit"], kette_quelle_zeilen=_fest_zahl((kq["ausgabe"] or {}).get("zeilen")))
        v = _versuch(L, dict(L.start_dict("lokal"), arbeitsordner=o1))
        s.haupt("Danach startet ein Lauf in O1 wie in einem unmarkierten Ordner", v["laeuft"], laeuft_nach_loeschen=v["laeuft"])
        if v["sid"]:
            L.beenden(v["sid"])
        o2 = k["start"]["cloud"]["arbeitsordner"]
        subprocess.run(list(bef) + [o2], capture_output=True, timeout=20)  # Aufraeumen: die Marke des zweiten Ordners
        subprocess.run(list(bef) + [o3], capture_output=True, timeout=20)  # und die des dritten, falls ein Start durchging
    elif not bef:
        s.offen.append("ordnermarke.loeschen_befehl: der Befehl zum Loeschen der Marke ist offen")
    return s


def _marke_von(datei, ordner):
    """Der Eintrag der Marke fuer `ordner` aus `ordnermarken.json` (Vertrag 1, Form `{"fassung", "marken": [...]}`) oder None."""
    if not datei:
        return None
    try:
        with open(datei, encoding="utf-8") as f:
            d = json.load(f)
        for m in d["marken"]:
            if m.get("ordner") == os.path.realpath(ordner):
                return m if re.match(r"^[0-9a-f]{32}$", str(m.get("kennung", ""))) else None
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        pass
    return None


REIHENFOLGE = [("S1", s1), ("S2", s2), ("S3", s3), ("S4", s4), ("S5", s5), ("E1", e1), ("E2", e2), ("E3", e3), ("E4-lokal", e4_lokal), ("E4-cloud", e4_cloud), ("E5", e5), ("E6", e6)]
