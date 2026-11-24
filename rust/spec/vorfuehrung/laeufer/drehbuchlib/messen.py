"""Kleine Messfunktionen des Laeufers. Jede misst eine Sache und gibt eine Zahl oder eine kleine Struktur zurueck.

- Markerzaehlung in Byte-Mitschnitten und Text (`zaehle`)
- Rueckgabecodes aus dem Text eines Befehls (`rueckgabecode`)
- Zeit t0/t1 ueber einen Pruefclient gegen die Bruecke (`start_bis_eingabefeld`)
- Zaehlung von Protokollzeilen (`protokollzeilen`, `zaehle_zeilen_der_sitzung`)
- Kettenpruefung mit `--letzte` (`kette_pruefen`)
- Gateway-Pruefung ueber eine eigene Pruefinstanz des Weiterreichers (`gateway_pruefinstanz`) und Suche nach dem genauen Schluessel
  (`schluessel_suche`, `suche_auswerten`; Vertrag 8, Abschnitte 4, 9a)
- Diagnosedatei fuer veraenderlichen Text (`diagnose_notiere`, `diagnose_schreibe`; 22.11.)
"""
from __future__ import annotations

import http.client
import json
import os
import re
import select
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

NULL64 = "0" * 64

# --- Loeschen von Arbeitsordnern (requirement of 21.11.2026) -------------------------------------------------------------------
# Der Laeufer loescht nur Ordner, die er selbst mit `neuer_tempordner` angelegt hat, und Ordner darunter.
_wurzeln = set()  # aufgeloeste Pfade der selbst angelegten Wurzeln
LOESCH_ABGELEHNT = "Loeschen abgelehnt: Der Pfad liegt nicht in einem eigenen Arbeitsordner. Nichts geloescht."


def neuer_tempordner(prefix, dir=None):
    """Wie `tempfile.mkdtemp(prefix=..., dir=...)`, merkt sich zusaetzlich den aufgeloesten Pfad als eigene Wurzel."""
    ordner = tempfile.mkdtemp(prefix=prefix, dir=dir)
    _wurzeln.add(os.path.realpath(ordner))
    return ordner


def sicher_loeschen(pfad):
    """Loescht `pfad` nur, wenn `os.path.realpath(pfad)` gleich einer gemerkten Wurzel ist oder darunter liegt (ganze Pfadteile,
    verglichen an `/`). Ist `pfad` selbst ein Symlink, loescht sie nichts. Sonst loescht sie nichts und meldet den festen Satz
    `LOESCH_ABGELEHNT` auf stderr, ohne Pfad. Gibt True, wenn sie geloescht hat."""
    if not pfad or os.path.islink(pfad):
        print(LOESCH_ABGELEHNT, file=sys.stderr)
        return False
    ziel = os.path.realpath(pfad)
    if not any(ziel == w or ziel.startswith(w + os.sep) for w in _wurzeln):
        print(LOESCH_ABGELEHNT, file=sys.stderr)
        return False
    shutil.rmtree(ziel, ignore_errors=True)
    return True


# --- Dateien einer Sitzung nach dem Lauf (Zielliste nach der Schluesselsuche, Entscheidung vom 21.11.2026; Zaehldateien, Befund eines Testlaufs)-------
# Kennung fest: `fullmatch`, weil `$` auch vor einem abschliessenden Zeilenumbruch passt.
ZIELLISTE_KENNUNG = re.compile(r"s-[0-9]{1,9}")
SITZUNGSDATEI_ENDUNGEN = (".zielliste.json", ".suche.json", ".modellweg.json")
ZIELLISTE_NICHT_ENTFERNT = "Sitzungsdatei nicht entfernt: kein eigener regulaerer Eintrag."
ZIELLISTE_SUCHE_NICHT_GELAUFEN = "Zielliste nicht entfernt: die Suche ist nicht gelaufen."


def sitzungsdatei_entfernen(pfad, sid, endung):
    """Entfernt `<ordner>/<sid><endung>`, nur wenn `sid` dem festen Muster der Sitzungskennungen entspricht, `endung` in
    `SITZUNGSDATEI_ENDUNGEN` steht und der Eintrag eine regulaere Datei mit der eigenen uid ist. Der Elternordner wird ueber einen
    Deskriptor (O_DIRECTORY | O_NOFOLLOW) geoeffnet, der Eintrag darueber mit `os.stat(..., follow_symlinks=False)` geprueft und mit
    `os.unlink(..., dir_fd=...)` entfernt. Kein rmtree, kein Pfad ueber fremde Teile. Gibt (entfernt, hinweis): `hinweis` ist ein fester
    Satz ohne Pfad oder None; fehlt die Datei, gibt es keinen Hinweis."""
    if not isinstance(sid, str) or not ZIELLISTE_KENNUNG.fullmatch(sid) or endung not in SITZUNGSDATEI_ENDUNGEN:
        return False, ZIELLISTE_NICHT_ENTFERNT
    ordner, name = os.path.split(pfad)
    if name != sid + endung:
        return False, ZIELLISTE_NICHT_ENTFERNT
    try:
        dfd = os.open(ordner or ".", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    except FileNotFoundError:
        return False, None
    except OSError:
        return False, ZIELLISTE_NICHT_ENTFERNT
    try:
        try:
            st = os.stat(name, dir_fd=dfd, follow_symlinks=False)
        except FileNotFoundError:
            return False, None
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid():
            return False, ZIELLISTE_NICHT_ENTFERNT
        os.unlink(name, dir_fd=dfd)
        return True, None
    except OSError:
        return False, ZIELLISTE_NICHT_ENTFERNT
    finally:
        os.close(dfd)


def zielliste_entfernen(pfad, sid):
    """Die Zielliste der Sitzung (`<sid>.zielliste.json`); siehe `sitzungsdatei_entfernen`."""
    return sitzungsdatei_entfernen(pfad, sid, ".zielliste.json")


def zaehle(daten, marker):
    """Zahl der Vorkommen von `marker` (Text oder Bytes) in `daten` (Text oder Bytes), ohne Ueberlappung."""
    if isinstance(daten, str):
        daten = daten.encode("utf-8")
    if isinstance(marker, str):
        marker = marker.encode("utf-8")
    return daten.count(marker)


def zaehle_ci(daten, marker):
    """Wie `zaehle`, aber ohne Beachtung der Gross- und Kleinschreibung. Fuer die sichtbare Ausgabe des Agenten (Gegenprobe ohne
    Pruefling): Ein Agent kann einen Marker in anderer Schreibweise ausgeben (etwa `mk-7q2`). In den Zaehldateien der Suche und des
    Weiterreichers bleibt es byte-genau, denn dort zaehlen die Pruefer selbst."""
    if isinstance(daten, bytes):
        daten = daten.decode("utf-8", "replace")
    if isinstance(marker, bytes):
        marker = marker.decode("utf-8", "replace")
    return daten.lower().count(marker.lower())


def zaehle_woerter(daten, woerter):
    """Zahl der Vorkommen ganzer Woerter, ohne Beachtung der Gross- und Kleinschreibung."""
    if isinstance(daten, bytes):
        daten = daten.decode("utf-8", "replace")
    return sum(len(re.findall(r"(?<![\w])%s(?![\w])" % re.escape(w), daten, re.I)) for w in woerter)


def ohne_marker(text, marker):
    """Schreibt `marker` in `text` als Zeichenklasse (`MK-7Q2` wird `MK-7Q[2]`), damit der Befehl den Marker nicht traegt."""
    return text.replace(marker, marker[:-1] + "[" + marker[-1] + "]")


def rueckgabecode(text):
    """Der Wert der letzten Zeile `RC=<n>` im Text eines Befehlsergebnisses, sonst None."""
    treffer = re.findall(r"^\W*RC=(\d+)\s*$", text or "", re.M)
    return int(treffer[-1]) if treffer else None


def ist_eingabefeld(zeilen, muster):
    """Wahr, wenn eine Zeile des Rasters, ohne Leerzeichen, auf einen der regulaeren Ausdruecke in `muster` passt.
    Das Merkmal gehoert dem Harness und steht in der Konfiguration (Beispiel fuer claude: `\\?forshortcuts`, `>Try` und das
    Zeichen U+276F, wie `bereit()` im Aufnahmeskript des Vertrags `traeger`)."""
    for z in zeilen:
        s = re.sub(r"\s+", "", z)
        if any(re.search(m, s) for m in muster):
            return True
    return False


def start_bis_eingabefeld(bruecke, start, muster, frist_s=10.0, takt_s=0.02, vertrauen=None, ohne_anmeldung=None):
    """t0 = Absenden von `POST /starten`, t1 = erster Rasterrahmen mit dem Eingabefeld. Gibt ein Dict.
    `vertrauen`: Muster der Vertrauensfrage eines Harness (Copilot CLI: `Confirm folder trust`). Steht sie im Bild, sendet der Laeufer
    einmal Enter und wartet danach auf das Eingabefeld (`vertrauen_bestaetigt`, Zeit `t_vertrauen`). `ohne_anmeldung`: Muster
    der Meldung ohne Anmeldung (`Please use /login`); sie beendet das Warten (`nicht_angemeldet`)."""
    code, antwort, t0 = bruecke.starten(start)  # `bruecke` ist hier der Lauf (zaehlt Starts) oder ein Bruecken-Client
    erg = {"status": code, "antwort": antwort, "sitzung": None, "t0": t0, "t1": None, "dauer_s": None, "rahmen": 0,
           "vertrauen_bestaetigt": False, "t_vertrauen": None, "nicht_angemeldet": False}
    if code != 201 and code != 200:
        return erg
    sid = antwort.get("sitzung") if isinstance(antwort, dict) else None
    erg["sitzung"] = sid
    ende = t0 + frist_s
    while sid and time.monotonic() < ende:
        r = (getattr(bruecke, "b", None) or bruecke).raster(sid)
        erg["rahmen"] += 1
        zeilen = r.get("zeilen", []) if isinstance(r, dict) else []
        flach = "".join(re.sub(r"\s+", "", z) for z in zeilen)
        if ohne_anmeldung and re.search(ohne_anmeldung, flach):
            erg["nicht_angemeldet"] = True
            break
        if vertrauen and not erg["vertrauen_bestaetigt"] and re.search(vertrauen, flach):
            (getattr(bruecke, "b", None) or bruecke).eingabe(sid, "")  # Enter bestaetigt die Vertrauensfrage
            erg["vertrauen_bestaetigt"], erg["t_vertrauen"] = True, round(time.monotonic() - t0, 3)
            time.sleep(takt_s)
            continue
        if isinstance(r, dict) and ist_eingabefeld(zeilen, muster):
            erg["t1"] = time.monotonic()
            erg["dauer_s"] = round(erg["t1"] - t0, 3)
            break
        time.sleep(takt_s)
    return erg


def protokollzeilen(datei):
    """Liest die Protokolldatei als Liste von (Rohtext, Dict). Fehlende Datei: leere Liste."""
    try:
        with open(datei, "rb") as f:
            inhalt = f.read().decode("utf-8")
    except FileNotFoundError:
        return []
    erg = []
    for z in inhalt.split("\n"):
        if z:
            try:
                erg.append((z, json.loads(z)))
            except ValueError:
                erg.append((z, None))
    return erg


def zaehle_zeilen_der_sitzung(datei, sitzung):
    return sum(1 for _, d in protokollzeilen(datei) if isinstance(d, dict) and d.get("sitzung") == sitzung)


def kette_pruefen(befehl, dateien, letzte=None, vorher=None, frist_s=15):
    """Ruft `kette pruefe [--vorher h] [--letzte h] <datei>...` auf. Gibt {"exit", "ausgabe" (Dict oder None), "stderr"}."""
    cmd = list(befehl)
    if vorher:
        cmd += ["--vorher", vorher]
    if letzte:
        cmd += ["--letzte", letzte]
    cmd += list(dateien)
    p = subprocess.run(cmd, capture_output=True, timeout=frist_s)
    try:
        aus = json.loads(p.stdout.decode("utf-8")) if p.stdout.strip() else None
    except ValueError:
        aus = None
    return {"exit": p.returncode, "ausgabe": aus, "stderr": p.stderr.decode("utf-8", "replace")[:200]}


class _UnixVerbindung(http.client.HTTPConnection):
    """HTTP ueber einen Unix-Socket (der Socket des Weiterreichers einer Sitzung, Vertrag 8, Abschnitt 3)."""
    def __init__(self, pfad, timeout):
        super().__init__("localhost", timeout=timeout)
        self._pfad = pfad

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self._pfad)


def gateway_pruefen(adresse, modell, frist_s=15):
    """Die eine Anfrage der Gateway-Pruefung (Vertrag 8, Abschnitte 7, 10, 11): `POST /v1/chat/completions` an den Weiterreicher,
    Koerper `{"model": <modell>, "messages": [{"role": "user", "content": "ping"}], "max_tokens": 1}`, ohne Marker.
    `adresse`: `("tcp", host, port)` oder `("unix", pfad)`. Gibt `(code, status)`; `code` ist None (erreichbar: 200),
    `gateway.modell-gesperrt` (403, vom Ziel etwa `policy_model_blocked`, oder `mw.modell_nicht_erlaubt`) oder
    `gateway.nicht-erreichbar` (502 `mw.ziel_nicht_erreichbar`, 504 `mw.ziel_frist`, kein Verbindungsaufbau und, fail closed, jeder
    andere Status)."""
    roh = json.dumps({"model": modell, "messages": [{"role": "user", "content": "ping"}], "max_tokens": 1}).encode("utf-8")
    try:
        c = _UnixVerbindung(adresse[1], frist_s) if adresse[0] == "unix" else http.client.HTTPConnection(adresse[1], adresse[2], timeout=frist_s)
        try:
            c.request("POST", "/v1/chat/completions", roh, {"Content-Type": "application/json"})
            r = c.getresponse()
            status, koerper = r.status, r.read(65536)
        finally:
            c.close()
    except (OSError, http.client.HTTPException):
        return "gateway.nicht-erreichbar", None
    if status == 200:
        return None, status
    if status == 403:
        return "gateway.modell-gesperrt", status
    return "gateway.nicht-erreichbar", status


def gateway_ziel_modell(einst_pfad, route):
    """`ziel` und `modell` der Route aus der Einstellungsdatei des Laufs (nie feste Werte). Gibt (ziel, modell) oder (None, None)."""
    try:
        with open(einst_pfad, encoding="utf-8") as f:
            r = next(x for x in json.load(f)["routen"] if x.get("schluessel") == route)
        ziel, modell = r.get("ziel"), r.get("modell")
    except (OSError, ValueError, StopIteration, KeyError, TypeError, AttributeError):
        return None, None
    return (ziel, modell) if isinstance(ziel, str) and isinstance(modell, str) and ziel and modell else (None, None)


def _beende_instanz(p, frist_s=5):
    """SIGTERM, Frist, sonst SIGKILL an die eigene Prozessgruppe. Gibt True, wenn SIGTERM genuegte (oder die Instanz schon beendet war);
    False, wenn die Instanz bis zum SIGKILL haengt oder auch danach noch laeuft."""
    if p.poll() is not None:
        return True
    try:
        p.send_signal(signal.SIGTERM)
        p.wait(timeout=frist_s)
        return True
    except subprocess.TimeoutExpired:
        pass
    except OSError:
        return p.poll() is not None
    try:
        os.killpg(p.pid, signal.SIGKILL)
    except OSError:
        pass
    try:
        p.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    return False


def gateway_pruefinstanz(programm, route, ziel, modell, schluessel_datei, ausserhalb=(), frist_bereit_s=10.0, frist_ende_s=5.0, frist_anfrage_s=15.0):
    """Pruefung des Gateways ueber eine eigene Pruefinstanz des Weiterreichers (Vertrag 8, Abschnitt 4; Entscheidung vom 20.11.2026).

    Vor dem Lauf gibt es noch keinen Weiterreicher der lokalen Sitzung. Der Laeufer startet darum eine eigene Instanz nur fuer diese
    eine Anfrage: eigener Ordner 0700 ausserhalb jedes Ordners aus `ausserhalb` mit `protokoll/` (0700); leere Umgebung ausser
    `PATH`, stdin `/dev/null`; Warten auf die Zeile `{"bereit":true}` (Frist 10 s); genau eine Anfrage (`gateway_pruefen`); dann
    SIGTERM (Frist 5 s), sonst SIGKILL. Haengt die Instanz bis zum SIGKILL, ist die Voraussetzung nicht erfuellt (fail closed).
    Der Ordner wird in jedem Fall geloescht. Keine Zaehlung, kein `--probelauf`. Der Schluessel wird nie gelesen, nur sein Pfad
    uebergeben.

    Gibt `(code, hinweis)`. `code` ist None (erreichbar), `gateway.nicht-erreichbar` oder `gateway.modell-gesperrt`. `hinweis` ist
    None oder ein fester Text: der Code der Instanz auf stderr (nur wenn er auf `^mw\\.[a-z_]+$` passt), `keine-bereitschaft` (die
    Instanz meldet sich nicht in der Frist, oder ihr stdout beginnt nicht mit der Bereitschaftszeile), `haengt` (SIGTERM genuegte
    nicht), `start` (das Programm liess sich nicht starten) oder `ordner` (der Ordner liess sich nicht sicher anlegen)."""
    basis = tempfile.gettempdir()
    if len(basis) + 40 > 100:  # sun_path: hoechstens 107 Byte
        basis = "/tmp"
    zustand = {"p": None, "ordner": None}

    def versuch():
        try:
            zustand["ordner"] = ordner = neuer_tempordner(prefix="wbp-", dir=basis)
            os.chmod(ordner, 0o700)
            wo = os.path.realpath(ordner)
            if any(wo == os.path.realpath(a) or wo.startswith(os.path.realpath(a) + os.sep) for a in ausserhalb):
                raise OSError("Ordner liegt in einem Arbeitsordner")
            os.mkdir(os.path.join(ordner, "protokoll"), 0o700)
        except OSError:
            return "gateway.nicht-erreichbar", "ordner"
        sock = os.path.join(ordner, "probe.sock")
        cmd = list(programm) + ["--betriebsart", "openai-lokal", "--socket", sock, "--socket-rechte", "0600", "--sitzung", "probe",
                                "--route", route, "--protokoll", os.path.join(ordner, "protokoll"), "--ziel", ziel, "--modell", modell,
                                "--schluessel-datei", schluessel_datei]
        try:
            p = zustand["p"] = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                                env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")}, start_new_session=True)
        except OSError:
            return "gateway.nicht-erreichbar", "start"
        puffer, ende = b"", time.monotonic() + frist_bereit_s
        while time.monotonic() < ende:
            r, _, _ = select.select([p.stdout], [], [], 0.05)
            if r:
                d = os.read(p.stdout.fileno(), 4096)
                if not d:
                    break
                puffer += d
                if b"\n" in puffer:
                    break
            elif p.poll() is not None:
                break
        if puffer.split(b"\n", 1)[0] != b'{"bereit":true}':
            hinweis = "keine-bereitschaft"
            try:
                p.wait(timeout=1)
                z = os.read(p.stderr.fileno(), 400).decode("ascii", "replace").split("\n", 1)[0].split(":", 1)[0]
                if re.match(r"^mw\.[a-z_]+$", z):
                    hinweis = z
            except (subprocess.TimeoutExpired, OSError):
                pass
            return "gateway.nicht-erreichbar", hinweis
        return gateway_pruefen(("unix", sock), modell, frist_s=frist_anfrage_s)[0], None

    try:
        code, hinweis = versuch()
    finally:
        p, haengt = zustand["p"], False
        if p is not None:
            haengt = not _beende_instanz(p, frist_ende_s)
            for f in (p.stdout, p.stderr):
                try:
                    f.close()
                except OSError:
                    pass
        if zustand["ordner"]:
            sicher_loeschen(zustand["ordner"])
    return ("gateway.nicht-erreichbar", "haengt") if haengt else (code, hinweis)


def suche_auswerten(exit_, stdout):
    """Wertet einen Aufruf von `werkbank-modellweg suche` aus (Vertrag 8, 9a, S5, S6, S8). Gibt (art, zahlen):
    `gruen` (Exit 0), `rot` (Exit 1), `unlesbar` (Exit 3), `fehler` (64, 78, jeder andere Exit, stdout nicht genau eine Zeile in
    RFC 8785 mit genau den Schluesseln `dateien`, `nicht_lesbar`, `treffer`, `uebersprungen` als nichtnegative ganze Zahlen, oder
    ein Exit, der den Zahlen widerspricht). `zahlen` ist das Dict oder None."""
    def ohne_doppel(paare):
        if len({k for k, _ in paare}) != len(paare):
            raise ValueError("doppelter Schluessel")
        return dict(paare)
    try:
        text = stdout.decode("utf-8")
        d = json.loads(text, object_pairs_hook=ohne_doppel)
    except (ValueError, UnicodeDecodeError):
        return "fehler", None
    if not isinstance(d, dict) or set(d) != {"dateien", "nicht_lesbar", "treffer", "uebersprungen"} \
            or any(not isinstance(v, int) or isinstance(v, bool) or v < 0 for v in d.values()) \
            or text != json.dumps(d, sort_keys=True, separators=(",", ":")) + "\n":
        return "fehler", None
    if exit_ == 0 and d["treffer"] == 0 and d["nicht_lesbar"] == 0:
        return "gruen", d
    if exit_ == 1 and d["treffer"] > 0:
        return "rot", d
    if exit_ == 3 and d["treffer"] == 0 and d["nicht_lesbar"] > 0:
        return "unlesbar", d
    return "fehler", None


def schluessel_suche(programm, quelle_option, quelle, ins, frist_s=60):
    """Ruft `<programm> suche <quelle_option> <quelle> --in <pfad> ...` auf (leere Umgebung ausser `PATH`, stdin `/dev/null`).
    Gibt `(art, zahlen)` nach `suche_auswerten`; ein Programm, das sich nicht starten laesst oder die Frist ueberschreitet, ist `fehler`.
    Von stderr bleibt nichts: weder ein Pfad noch eine Zeile."""
    cmd = list(programm) + ["suche", quelle_option, quelle]
    for i in ins:
        cmd += ["--in", i]
    try:
        p = subprocess.run(cmd, stdin=subprocess.DEVNULL, capture_output=True, timeout=frist_s,
                           env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")})
    except (OSError, subprocess.TimeoutExpired):
        return "fehler", None
    return suche_auswerten(p.returncode, p.stdout)


def kette_quelle_pruefen(befehl, ordner, quelle, frist_s=15):
    """Ruft `kette pruefe-quelle <ordner> <quelle>` auf. `befehl` ist `kette_befehl` (`... pruefe`); der Unterbefehl wird ersetzt.
    Gibt wie `kette_pruefen` {"exit", "ausgabe", "stderr"}."""
    cmd = list(befehl)
    i = cmd.index("pruefe")
    cmd = cmd[:i] + ["pruefe-quelle", ordner, quelle]
    p = subprocess.run(cmd, capture_output=True, timeout=frist_s)
    try:
        aus = json.loads(p.stdout.decode("utf-8")) if p.stdout.strip() else None
    except ValueError:
        aus = None
    return {"exit": p.returncode, "ausgabe": aus, "stderr": p.stderr.decode("utf-8", "replace")[:200]}


def aussen_shell(befehl, frist_s=15):
    """Gewoehnliche Shell des Vorfuehrenden ausserhalb des Einschlusses: (Rueckgabecode, Ausgabe)."""
    p = subprocess.run(["/bin/sh", "-c", befehl], capture_output=True, timeout=frist_s)
    return p.returncode, (p.stdout + p.stderr).decode("utf-8", "replace")


def zaehle_zeilen_der_sitzung_in(zeilen, sitzung):
    """Wie `zaehle_zeilen_der_sitzung`, aber ueber schon gelesene Zeilen (Liste von (Rohtext, Dict))."""
    return sum(1 for _, d in zeilen if isinstance(d, dict) and d.get("sitzung") == sitzung)


# --- Gegenprobe von S1 ohne Diagnose: direkte Kinder des Kerns --------------------------------------------------------------
EINHEIT = re.compile(r"^[a-z][a-z0-9@._-]{0,63}$")  # nie mit `-` am Anfang, sonst liest systemctl den Namen als Schalter


def main_pid(einheit, frist_s=10):
    """MainPID der Einheit aus `systemctl --user show -p MainPID --value <einheit>`; Umgebung nur PATH, HOME, XDG_RUNTIME_DIR und
    DBUS_SESSION_BUS_ADDRESS, Stdin /dev/null, die Ausgabe wird nur als Zahl gelesen. None bei Fehler, bei einem ungueltigen Namen und bei
    MainPID 0 (die Einheit laeuft nicht)."""
    if not isinstance(einheit, str) or not EINHEIT.match(einheit):
        return None
    env = {n: os.environ[n] for n in ("PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS") if n in os.environ}
    try:
        p = subprocess.run(["systemctl", "--user", "show", "-p", "MainPID", "--value", einheit], stdin=subprocess.DEVNULL,
                           capture_output=True, env=env, timeout=frist_s)
    except (OSError, subprocess.TimeoutExpired):
        return None
    text = p.stdout.decode("utf-8", "replace").strip()
    if p.returncode != 0 or not re.match(r"^[0-9]{1,10}$", text):
        return None
    pid = int(text)
    return pid if pid > 0 else None


def kinder_zahl(proc_wurzel, pid):
    """Zahl der Prozesse mit PPID gleich `pid`, gelesen nur aus `<proc_wurzel>/[0-9]*/stat` (PPID: viertes Feld, gezaehlt nach der letzten
    `)`; der Name steht in Klammern und darf Leerzeichen und Klammern tragen). Liest nie `environ` oder `cmdline`. None, wenn die Wurzel
    nicht lesbar ist, kein Prozess gelesen werden konnte oder `pid` fehlt. Ein Prozess, der waehrend des Lesens endet, wird uebergangen."""
    if not pid:
        return None
    try:
        namen = [n for n in os.listdir(proc_wurzel) if n.isdigit()]
    except OSError:
        return None
    gelesen, kinder = 0, 0
    for n in namen:
        try:
            with open(os.path.join(proc_wurzel, n, "stat"), "rb") as f:
                roh = f.read(4096).decode("utf-8", "replace")
        except OSError:
            continue
        rest = roh.rpartition(")")[2].split()
        if len(rest) < 2 or not rest[1].isdigit():
            continue
        gelesen += 1
        if int(rest[1]) == pid:
            kinder += 1
    return kinder if gelesen else None


# --- Diagnosedatei (22.11.) ----------------------------------------------------------------------------------------------------
# Ergebnisdatei, Bericht und Konsole tragen nur feste Codes, Zahlen und feste Saetze aus dem Code. Veraenderlicher Text, den die
# Ursachensuche braucht (der Text einer Ausnahme, das Ende des Verlaufs einer Sitzung, die Antwort der Bruecke), geht allein in diese
# Sammlung und von dort in EINE Datei neben der Ergebnisdatei: `<ergebnis>.diagnose`.
# - Die Datei entsteht nur, wenn es etwas zu schreiben gibt (`diagnose_notiere` wurde gerufen).
# - Rechte 0600, angelegt mit `O_CREAT|O_EXCL|O_NOFOLLOW` unter der Umask 077. Gibt es den Namen schon, schreibt der Laeufer nichts
#   hinein: Er liest die Datei nie, ueberschreibt sie nie und loescht sie nie. Das Loeschen nach der Ursachensuche ist ein Schritt
#   des Antrags, nicht des Laeufers.
# - Die Konsole nennt nur die feste Zeile `diagnose: angelegt` (oder `diagnose: nicht angelegt`), nie einen Inhalt.
DIAGNOSE_GRENZE = 65536  # Bytes der Datei; der Rest wird abgeschnitten
DIAGNOSE_ENDUNG = ".diagnose"
FESTER_CODE = re.compile(r"^[a-z][a-z0-9._-]{0,63}$")  # Form eines festen Codes (`ziel.aufbau`, `s1.zug_frist`)
_diagnose_zeilen = []
_diagnose_stand = {"zuletzt": None}  # "angelegt", "nicht angelegt" oder None (nichts zu schreiben)


def diagnose_leeren():
    """Beginn eines Laufs: nichts aus einem frueheren Lauf bleibt stehen."""
    del _diagnose_zeilen[:]
    _diagnose_stand["zuletzt"] = None


def diagnose_notiere(ort, text):
    """Merkt einen Text mit veraenderlichem Inhalt fuer die Diagnosedatei. `ort` ist ein fester Name der Stelle. Der Text kommt nie in
    Ergebnisdatei, Bericht oder Konsole."""
    roh = "%s" % (text,)
    _diagnose_zeilen.append("%s: %s" % (ort, roh.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r")))


def fester_code(text, ersatz):
    """`text`, wenn er die Form eines festen Codes hat, sonst `ersatz` (ebenfalls ein fester Code)."""
    return text if isinstance(text, str) and FESTER_CODE.match(text) else ersatz


def diagnose_schreibe(ergebnis_pfad):
    """Legt `<ergebnis_pfad>.diagnose` an, wenn es etwas zu schreiben gibt. Gibt "angelegt", "nicht angelegt" (die Datei liess sich
    nicht anlegen, auch weil der Name schon vergeben ist) oder None (nichts zu schreiben). Merkt sich den Wert fuer `diagnose_zeile`."""
    if not _diagnose_zeilen:
        _diagnose_stand["zuletzt"] = None
        return None
    inhalt = ("\n".join(_diagnose_zeilen) + "\n").encode("utf-8", "replace")[:DIAGNOSE_GRENZE]
    flaggen = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    alt = os.umask(0o077)
    try:
        fd = os.open(ergebnis_pfad + DIAGNOSE_ENDUNG, flaggen, 0o600)
    except OSError:
        _diagnose_stand["zuletzt"] = "nicht angelegt"
        return "nicht angelegt"
    finally:
        os.umask(alt)
    try:
        ansicht = memoryview(inhalt)
        while ansicht:
            n = os.write(fd, ansicht)
            ansicht = ansicht[n:]
        _diagnose_stand["zuletzt"] = "angelegt"
    except OSError:
        _diagnose_stand["zuletzt"] = "nicht angelegt"
    finally:
        os.close(fd)
    return _diagnose_stand["zuletzt"]


def diagnose_zeile():
    """Die feste Zeile fuer die Konsole oder None, wenn nichts zu schreiben war."""
    z = _diagnose_stand["zuletzt"]
    return None if z is None else "diagnose: %s" % z
