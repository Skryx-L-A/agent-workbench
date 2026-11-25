"""Attrappe des Such-Prozesses je Sitzung (Vertrag 4, `rust/spec/such-socket/README.md`).

Ein Unix-Socket je Sitzung, die Route kommt aus dem Start, nie aus der Anfrage. Tor `darf` vor dem Kuerzen, Zahl der
gesperrten Treffer, Protokollzeile vor dem Absenden. Die Rangliste ist ein einfaches Wortzaehlen; sie genuegt fuer den
Vorfuehr-Vault (N-I vor N-K vor dem Schichtplan). Fehler zum Einschalten (`fehler`, Mutationsprobe):
  s3-grund-an-modell     die erste Antwort mit gesperrten Treffern traegt das Feld `grund` (A49 verletzt)
  s3-titel-gesperrt      die erste Antwort mit gesperrten Treffern traegt deren Titel
  s5-gesperrt-lokal      auf einer lokalen Route bleibt Kundendaten gesperrt (Tor falsch)
  s5-route-global        die Route gilt global: die zuletzt gestartete lebende Sitzung bestimmt sie (A48 verletzt)
  e1-route-aus-umgebung  die Route kommt aus einer Umgebungsvariable des Agenten mit `route` im Namen
  e1-feld-akzeptiert     ein Feld `route` in der Anfrage wird angenommen
  s3-protokoll-fehler-einmal  die erste Antwort mit gesperrten Treffern wird gezaehlt, die Protokollzeile scheitert, der Agent bekommt
                         {"error": "protokoll"} (Vertrag 4, Klarstellung 5)
  s3-zaehldatei-zu-wenig die Zaehldatei der ersten Sitzung zaehlt die erste Antwort nicht
  s3-zaehldatei-marker   die Zaehldatei meldet bei der ersten Antwort einen Marker, der gesperrt ist
  s3-zaehldatei-mit-text die Zaehldatei der zweiten Sitzung (Gegenprobe von S3) traegt ein Textfeld
  s3-zaehldatei-mit-text-erste die Zaehldatei der ersten Sitzung traegt ab der zweiten Antwort (Gegenprobe von S3 in der ersten Sitzung) ein Textfeld
"""
from __future__ import annotations

import json
import os
import re
import socket
import threading
import time

import darf as darf_mod
import kette

STOPP = {"wann", "dass", "eine", "einer", "einen", "oder", "wird", "sind", "dies", "diese", "nach", "auch", "wenn"}
ERLAUBT = {"op", "id", "query", "k", "space"}
SCHLUESSEL = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")


def lade_vault(wurzel):
    notizen = []
    for ordner, _, dateien in sorted(os.walk(wurzel)):
        for name in sorted(dateien):
            if not name.endswith(".md"):
                continue
            pfad = os.path.join(ordner, name)
            with open(pfad, encoding="utf-8") as f:
                text = f.read()
            teile = text.split("---\n", 2)
            kopf, rumpf = {}, text
            if len(teile) == 3 and teile[0] == "":
                rumpf = teile[2]
                for z in teile[1].splitlines():
                    k, _, v = z.partition(":")
                    v = v.strip()
                    if len(v) >= 2 and v[0] == v[-1] == '"':
                        v = v[1:-1]
                    kopf[k.strip()] = None if v in ("", "null") else v
            notizen.append({"rel": os.path.relpath(pfad, wurzel), "kopf": kopf, "text": rumpf.strip(),
                            "title": kopf.get("title") or name})
    return notizen


def tokens(text):
    return {t for t in re.findall(r"[a-zäöüß0-9]+", text.lower()) if len(t) >= 4 and t not in STOPP}


def rangliste(notizen, frage):
    q = tokens(frage)
    erg = []
    for n in notizen:
        s = len(q & tokens(n["title"] + " " + n["text"]))
        if s > 0:
            erg.append((s, n))
    erg.sort(key=lambda x: (-x[0], x[1]["rel"]))
    return erg


class SuchProzess:
    def __init__(self, vault, socket_pfad, route, sitzung, einstellungen, protokoll_ordner, zaehlordner, marker,
                 fehler=(), registry=None, umgebung=None, probelauf=True):
        # Nachbildung von `--probelauf --marker --marker-zaehlung`: nur zusammen (alle drei oder keine), sonst Aufruffehler
        if bool(probelauf) != bool(zaehlordner) or bool(zaehlordner) != bool(marker):
            raise ValueError("Aufruf (64): --probelauf, --marker und --marker-zaehlung nur zusammen")
        self.vault, self.socket_pfad, self.route, self.sitzung = vault, socket_pfad, route, sitzung
        self.protokoll_ordner, self.zaehlordner, self.marker = protokoll_ordner, zaehlordner, marker  # marker: {name: wert}
        self.zaehlung = {"sitzung": sitzung, "antworten": 0, "marker": {m: 0 for m in marker.values()}}
        self._vorab = None  # Bytes der Antwort, die vor der (gescheiterten) Protokollzeile gezaehlt wurden
        self.fehler, self.registry, self.umgebung = set(fehler), registry, umgebung
        with open(einstellungen, encoding="utf-8") as f:  # einmal beim Start (Vertrag 4)
            self.vorgabe = json.load(f)
        self.vorgabe.pop("fassung", None)
        if not any(r["schluessel"] == route and r.get("klasse") for r in self.vorgabe["routen"]):
            raise ValueError("Route nicht in der Einstellungsdatei oder ohne Klasse")
        self.lock = threading.Lock()
        self.srv = None

    # --- Lebenslauf -------------------------------------------------------------------------------
    def starten(self):
        if os.path.lexists(self.socket_pfad):
            raise OSError("Socketpfad belegt")
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(self.socket_pfad)
        os.chmod(self.socket_pfad, 0o600)
        self.srv.listen(8)
        threading.Thread(target=self._annehmen, daemon=True).start()

    def beenden(self):
        try:
            if self.srv:
                self.srv.close()
        finally:
            if os.path.lexists(self.socket_pfad):
                os.unlink(self.socket_pfad)

    def _annehmen(self):
        while True:
            try:
                c, _ = self.srv.accept()
            except OSError:
                return
            threading.Thread(target=self._verbindung, args=(c,), daemon=True).start()

    def _zaehlen(self, antwort_bytes):
        """Zaehldatei `<ordner>/<sitzung>.suche.json`: Zahl der Antworten und Zaehler je Marker, ueber alle Antworten dieser
        Sitzung, vor dem Absenden geschrieben (Nachtrag zum Folgeauftrag). Keine Bytes, keine Inhalte."""
        with self.lock:
            self.zaehlung["antworten"] += 1
            for m in self.zaehlung["marker"]:
                self.zaehlung["marker"][m] += antwort_bytes.count(m.encode("utf-8"))
            if "s3-zaehldatei-marker" in self.fehler and self.zaehlung["antworten"] == 1 and self._einmal("s3-zaehldatei-marker"):
                self.zaehlung["marker"][self.marker["nk"]] += 1  # Mutationsprobe: die Datei meldet einen gesperrten Marker
            pfad = os.path.join(self.zaehlordner, self.sitzung + ".suche.json")
            tmp = pfad + ".neu"
            datei = dict(self.zaehlung)
            if "s3-zaehldatei-zu-wenig" in self.fehler and self.sitzung == "s-1" and self.zaehlung["antworten"] == 1:
                datei["antworten"] = 0  # Mutationsprobe: die Zaehldatei zaehlt die erste Antwort nicht
            if "s3-zaehldatei-mit-text" in self.fehler and self.sitzung == "s-2":  # nur die zweite Sitzung (Gegenprobe von S3)
                datei["anfrage"] = "Wann ist das"  # Mutationsprobe: die Datei traegt Text
            if "s3-zaehldatei-mit-text-erste" in self.fehler and self.sitzung == "s-1" and self.zaehlung["antworten"] >= 2:  # ab der zweiten Antwort
                datei["anfrage"] = "Wann ist das"  # Mutationsprobe: die Datei traegt Text
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump(datei, f)
            os.replace(tmp, pfad)

    def _verbindung(self, c):
        with c, c.makefile("rwb") as f:
            while True:
                zeile = f.readline(1048577)
                if not zeile:
                    return
                antwort = (json.dumps(self.bearbeite(zeile), ensure_ascii=False) + "\n").encode("utf-8")
                if self._vorab is not None:  # die Suche hat vor dem Protokoll gezaehlt; der Agent bekommt nur den Fehler
                    self._zaehlen(self._vorab)
                    self._vorab = None
                else:
                    self._zaehlen(antwort)
                try:
                    f.write(antwort)
                    f.flush()
                except OSError:
                    return

    # --- Anfrage ----------------------------------------------------------------------------------
    def wirksame_route(self):
        if "s5-route-global" in self.fehler and self.registry is not None:
            return self.registry.zuletzt_gestartete_route(self.route)
        if "e1-route-aus-umgebung" in self.fehler and self.umgebung is not None:
            for k, v in list(self.umgebung.items()):
                if "route" in k.lower():
                    return v
        return self.route

    def bearbeite(self, zeile):
        try:
            if not zeile.endswith(b"\n") or len(zeile) > 1048576:
                raise ValueError
            a = json.loads(zeile.decode("utf-8"))
            if not isinstance(a, dict):
                raise ValueError
        except ValueError:
            return {"error": "bad_request"}
        kennung = {"id": a["id"]} if isinstance(a.get("id"), (str, int)) and not isinstance(a.get("id"), bool) else {}
        route = self.wirksame_route()
        erlaubt = set(ERLAUBT)
        if "e1-feld-akzeptiert" in self.fehler:
            erlaubt.add("route")
            if isinstance(a.get("route"), str):
                route = a["route"]
        if set(a) - erlaubt or a.get("op") not in ("search", "ping"):
            return dict({"error": "bad_request"}, **kennung)
        if a["op"] == "ping":
            return dict({"ok": True}, **kennung)
        k = a.get("k", 5)
        if not isinstance(a.get("query"), str) or isinstance(k, bool) or not isinstance(k, int) or not 1 <= k <= 50:
            return dict({"error": "bad_request"}, **kennung)
        try:
            notizen = lade_vault(self.vault)
        except OSError:
            return dict({"error": "unreadable"}, **kennung)
        liste = rangliste(notizen, a["query"])
        entscheide = []
        for score, n in liste:
            e = self._tor(n, route)
            entscheide.append((score, n, e))
        hits, gesperrt = [], []
        for i, (score, n, e) in enumerate(entscheide):
            if i < k and not e["darf"]:
                gesperrt.append((n, e))
            if e["darf"] and len(hits) < k:
                hits.append({"rel": n["rel"], "title": n["title"], "score": score, "text": n["text"]})
        antwort = {"fallback": True, "hits": hits, "gesperrt": len(gesperrt)}
        if gesperrt and "s3-grund-an-modell" in self.fehler and self._einmal("s3-grund-an-modell"):
            antwort["grund"] = gesperrt[0][1]["grund"]
        if gesperrt and "s3-titel-gesperrt" in self.fehler and self._einmal("s3-titel-gesperrt"):
            antwort["gesperrte"] = [{"title": n["title"]} for n, _ in gesperrt]
        if gesperrt:
            try:
                if "s3-protokoll-fehler-einmal" in self.fehler and self._einmal("s3-protokoll-fehler-einmal"):
                    self._vorab = (json.dumps(dict(antwort, **kennung), ensure_ascii=False) + "\n").encode("utf-8")
                    raise OSError("Mutationsprobe: die Protokollzeile scheitert")
                self._protokollieren(gesperrt, route)
            except (OSError, ValueError):
                return dict({"error": "protokoll"}, **kennung)
        return dict(antwort, **kennung)

    def _einmal(self, name):
        """Die Fehler von S3 wirken nur bei der ersten Antwort mit gesperrten Treffern im ganzen Dienst (Mutationsprobe)."""
        return self.registry is None or self.registry.einmal(name)

    def _tor(self, n, route):
        kopf = n["kopf"]
        klasse = kopf.get("datenklasse")
        kunde = kopf.get("kunde")
        e = darf_mod.darf(self.vorgabe, {"klasse": klasse}, route, kunde)
        if "s5-gesperrt-lokal" in self.fehler and e["darf"] is True and klasse == self.vorgabe["klassen"][-1]:
            art = next((r["art"] for r in self.vorgabe["routen"] if r["schluessel"] == route), None)
            if art == "lokal":
                return {"darf": False, "grund": "klasse-ueber-route", "klasse": klasse, "meldung": ""}
        return e

    def _protokollieren(self, gesperrt, route):
        gruppen = {}
        for n, e in gesperrt:
            kunde = n["kopf"].get("kunde")
            kunde = kunde if isinstance(kunde, str) and SCHLUESSEL.match(kunde) else None
            schluessel = (e["klasse"], kunde, e["grund"])
            gruppen.setdefault(schluessel, 0)
            gruppen[schluessel] += 1
        jetzt = int(time.time() * 1000)
        datei = os.path.join(self.protokoll_ordner, "brain-%s.jsonl" % kette.zeit_text(jetzt)[:7])
        for (klasse, kunde, grund), anzahl in gruppen.items():
            kette.anhaengen_gesperrt(datei, {"route": route, "sitzung": self.sitzung, "grund": grund, "klasse": klasse,
                                             "kunde": kunde, "anzahl": anzahl, "zeit_ms": jetzt,
                                             "pfad": gesperrt[0][0]["rel"]},
                                     fehler=[f.replace("s4-", "") for f in self.fehler])
