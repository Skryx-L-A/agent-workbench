#!/usr/bin/env python3
"""Attrappe der Copilot CLI (Testfassung 0.0.88, Form der echten 1.0.88) fuer den Probelauf: Sie wird von "Lauf starten" wie die echte CLI gestartet (Vertrag 1, `rust/spec/start/README.md`,
Harness-Tabelle und Abschnitt copilot; Faelle `aufruf-copilot-auto` und `aufruf-gueltig-copilot`), kostet aber nichts und braucht kein Konto.

Nur Standardbibliothek, Python 3. Nicht ausgeliefert (nicht in der Lieferliste), wie alle Attrappen. Der Name im Wegwerf-Abbild ist `copilot`.

Was sie tut:
  1. Schreibt argv und die NAMEN der Umgebung in eine Datei (JSON). Werte schreibt sie nicht. Nur vom Token sagt sie, welches Format er hat
     (`token_format`: `platzhalter` fuer `gho_` und 36 Nullen, `gho` fuer jedes andere `gho_`-Format, `falsch`, `fehlt`). Ort: Umgebung
     `COPILOT_ATTRAPPE_PROTOKOLL`, sonst `$HOME/.copilot/attrappe-protokoll.json` (nur fuer den Test).
  2. Fehlt `COPILOT_GITHUB_TOKEN`, zeigt sie `Please use /login` und wartet. Sonst zeigt sie die Vertrauensfrage (`Confirm folder trust`),
     wartet auf Enter und zeigt dann das Eingabefeld: ein `❯` zwischen zwei Linien aus `─` und die Fusszeile `/ commands` (wie 1.0.88).
  3. Je Eingabe schickt sie EINE HTTPS-Anfrage ueber `HTTPS_PROXY` (CONNECT) an das Ziel der Route, mit dem Platzhalter im Kopf
     `Authorization` und dem Text der Eingabe im Koerper. Ziel: Umgebung `COPILOT_ATTRAPPE_ZIEL` (`host:port`), sonst
     `copilot-attrappe.test:443`; Zertifikate prueft sie gegen `COPILOT_ATTRAPPE_CA` (Datei), sonst gegen das Bundle des Systems.
  4. Danach fuehrt sie die Befehle der Eingabe aus und schreibt `FERTIG T<n>`. Verstanden werden die Eingaben der Attrappen-Konfiguration
     (`#T<n> [MARKER] BEFEHL <befehl>`, `... BUENDEL <skript>`, `... ZAEHLEN`) und die der Konfiguration des Probelaufs (`Zug T<n>` und ein
     Skript zwischen Gravis). `SUCHE` fuehrt sie nicht aus (keine Wissenssuche). Sie laeuft nur auf Eingaben des Laeufers: Der Text wird in
     einer Shell ausgefuehrt, das ist Absicht und gilt nur im Einschluss der Wegwerf-VM.
     Eine Eingabe mit `!` am Anfang ist eine Shell-Eingabe: Die Attrappe fuehrt den Rest in einer Shell aus und schickt KEINE Anfrage an
     das Ziel (wie die Shell-Eingabe der echten CLI, die das Modell nicht fragt). Das Merkmal `FERTIG T<n>` schreibt dann nur der Befehl selbst.
  5. Paketform: Sie hat die Form des Plattformpakets der echten CLI (Programm `copilot`, Fassung `0.0.88` im Paket und im
     Cache-Pfad; Aufbau und Werkzeug `paket_legen.py`, siehe README). Das Produkt bereitet den Cache-Ordner der CLI vor, mit `<harness.copilot>
     --version`. Darum legt sie mit genau diesem Aufruf, wenn eine der vier Dateien unter `$HOME/.cache/copilot/pkg/linux-x64/0.0.88` fehlt, die
     fehlenden an (`.extraction-complete` zuletzt) und schreibt sonst nichts; ist alles da, schreibt sie nichts. Bei jedem anderen Start meldet sie
     ohne die vier Dateien `Failed to extract bundled package` (Fehlerausgabe), Exit 1, ohne Sitzung und ohne Protokoll.
  6. Abbruchgrund: Jeder Abbruch vor dem ersten Bild schreibt eine feste erste Zeile auf die Fehlerausgabe, die im Pseudoterminal im Bild
     steht: `Failed to extract bundled package` (Exit 1, Punkt 5), `Failed to find the platform package @github/copilot-linux-x64` (Loader, Exit 1),
     `Failed to start the platform package` (Loader, `exec` scheitert, Exit 1) und fuer jede Ausnahme in Python `Unexpected error in the CLI attrappe`,
     darunter der Name der Ausnahme (nie ihr Text), Exit 70.
"""
from __future__ import annotations

import json
import os
import re
import socket
import ssl
import subprocess
import sys
from urllib.parse import urlparse

PLATZHALTER = re.compile(r"^gho_0{36}$")
TOKEN_FORM = re.compile(r"^gho_[0-9A-Za-z]{36}$")
ZUG = re.compile(r"^\s*#T(\d+)\s+(?:[A-Z]{2}-[0-9A-Z]{3}\s+)?(BEFEHL|BUENDEL|SUCHE|ZAEHLEN)(?:\s+(.*))?$", re.S)
ZUG_PROSA = re.compile(r"Zug T(\d+)")
GRAVIS = re.compile(r"`([^`]+)`", re.S)
LINIE = "─" * 60
VERSION_ZEILE = "GitHub Copilot CLI 0.0.88 (Attrappe)\n"
PAKET_FASSUNG = "0.0.88"  # Testfassung, ueberall dieselbe: Zeile von --version, Haupt- und Plattformpaket, Cache-Pfad (Form `paketform-copilot-1.0.88`)
PLATTFORM = "linux-x64"
CACHE_DATEIEN = ("index.js", "app.js", "prebuilds/%s/runtime.node" % PLATTFORM, ".extraction-complete")  # die vier Pfade; der letzte zuletzt
ENTPACKFEHLER = "Failed to extract bundled package\n"
ABBRUCH_ZEILE = "Unexpected error in the CLI attrappe\n"  # feste erste Zeile jedes anderen Abbruchs vor dem ersten Bild (Ausnahme in Python), Exit 70


def token_format(wert):
    if wert is None or wert == "":
        return "fehlt"
    if PLATZHALTER.match(wert):
        return "platzhalter"
    return "gho" if TOKEN_FORM.match(wert) else "falsch"


def cache_ordner(env):
    return os.path.join(env.get("HOME", "."), ".cache", "copilot", "pkg", PLATTFORM, PAKET_FASSUNG)


def fehlende_cache_dateien(env):
    """Die der vier Pfade, die unter dem Cache-Ordner nicht als gewoehnliche Datei vorhanden sind."""
    o = cache_ordner(env)
    return [d for d in CACHE_DATEIEN if not os.path.isfile(os.path.join(o, d)) or os.path.islink(os.path.join(o, d))]


def entpacke(env):
    """Der Ersatz fuers Entpacken: legt die fehlenden der vier Pfade an, `.extraction-complete` zuletzt. Sonst nichts."""
    o = cache_ordner(env)
    for d in fehlende_cache_dateien(env):
        pfad = os.path.join(o, d)
        os.makedirs(os.path.dirname(pfad), exist_ok=True)
        with open(pfad, "w", encoding="utf-8") as f:
            f.write("" if d == ".extraction-complete" else "// attrappe\n")


def protokoll_pfad(env):
    p = env.get("COPILOT_ATTRAPPE_PROTOKOLL")
    if p:
        return p
    return os.path.join(env.get("HOME", "."), ".copilot", "attrappe-protokoll.json")


def schreibe_protokoll(pfad, argv, env, anfragen):
    """argv, Namen der Umgebung und Zahlen. Nie ein Wert der Umgebung."""
    d = {"argv": ["copilot"] + list(argv[1:]), "env_namen": sorted(env), "token_format": token_format(env.get("COPILOT_GITHUB_TOKEN")),
         "anfragen": anfragen}
    os.makedirs(os.path.dirname(pfad) or ".", exist_ok=True)
    tmp = pfad + ".neu"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(d, f, indent=1)
        f.write("\n")
    os.replace(tmp, pfad)


def https_anfrage(proxy, ziel, pfad, kopf, body, ca=None, frist_s=15.0):
    """Eine HTTPS-Anfrage ueber einen HTTP-Proxy (CONNECT). `ziel` ist `host:port`. Gibt den Statuscode der Antwort."""
    host, _, port = ziel.rpartition(":")
    p = urlparse(proxy)
    roh = socket.create_connection((p.hostname, p.port), timeout=frist_s)
    try:
        roh.sendall(("CONNECT %s HTTP/1.1\r\nHost: %s\r\n\r\n" % (ziel, ziel)).encode("ascii"))
        kopf_proxy = b""
        while b"\r\n\r\n" not in kopf_proxy:
            b = roh.recv(4096)
            if not b:
                raise OSError("Proxy schloss die Verbindung")
            kopf_proxy += b
        if not re.match(rb"^HTTP/1\.[01] 200", kopf_proxy):
            raise OSError("CONNECT abgewiesen")
        ctx = ssl.create_default_context(cafile=ca) if ca else ssl.create_default_context()
        ctx.verify_flags &= ~ssl.VERIFY_X509_STRICT  # Python ab 3.13 prueft streng (Schluesselkennungen, Nutzungen); die Test-CA der Wegwerf-VM ist schlicht
        s = ctx.wrap_socket(roh, server_hostname=host)
        try:
            zeilen = ["POST %s HTTP/1.1" % pfad, "Host: %s" % host, "Connection: close", "Content-Length: %d" % len(body)]
            zeilen += ["%s: %s" % (n, w) for n, w in kopf.items()]
            s.sendall(("\r\n".join(zeilen) + "\r\n\r\n").encode("ascii") + body)
            # Gelesen wird die Antwort als HTTP, bis der Kopf vollstaendig ist (Statuszeile). Wartet man auf das Ende der Verbindung,
            # laeuft bei einem Ziel, das sie offen laesst, die Frist ab, und der Status wird 0 (Befund eines Testlaufs).
            antwort = b""
            while b"\r\n\r\n" not in antwort:
                try:
                    b = s.recv(4096)
                except ssl.SSLError:
                    break
                if not b:
                    break
                antwort += b
        finally:
            s.close()
        m = re.match(rb"^HTTP/1\.[01] (\d{3})", antwort)
        return int(m.group(1)) if m else 0
    finally:
        roh.close()


def lies_zeile(fd_ein, aus):
    """Eine Eingabezeile bis Enter. Am Terminal (cbreak seit dem Start, mit eigenem Echo, keine Grenze der Zeilenlaenge des Terminals),
    sonst zeilenweise. Gibt None am Ende der Eingabe."""
    if os.isatty(fd_ein):
        puffer = bytearray()
        while True:
            b = os.read(fd_ein, 1)
            if not b:
                return None if not puffer else puffer.decode("utf-8", "replace")
            if b in (b"\r", b"\n"):
                aus.write("\n")
                aus.flush()
                return puffer.decode("utf-8", "replace")
            puffer += b
            aus.write(b.decode("utf-8", "ignore"))
            aus.flush()
    z = sys.stdin.readline()
    return z.rstrip("\r\n") if z else None


def eingabefeld(aus):
    aus.write("\n%s\n❯ \n%s\n  / commands · ? help\n" % (LINIE, LINIE))
    aus.flush()


def verstehe(text):
    """(n, art, arg) der Eingabe oder None."""
    m = ZUG.match(text)
    if m:
        return int(m.group(1)), m.group(2), (m.group(3) or "").strip()
    m = ZUG_PROSA.search(text)
    if not m:
        return None
    g = GRAVIS.search(text)
    return int(m.group(1)), ("BEFEHL" if g else "ZAEHLEN"), (g.group(1) if g else "")


def fuehre_aus(arg, aus):
    try:
        p = subprocess.run(["/bin/sh", "-c", arg], capture_output=True, timeout=120, stdin=subprocess.DEVNULL)
        ausgabe = (p.stdout + p.stderr).decode("utf-8", "replace")
    except (OSError, subprocess.TimeoutExpired) as e:
        ausgabe = "Fehler: %s\n" % type(e).__name__
    aus.write(ausgabe if ausgabe.endswith("\n") or not ausgabe else ausgabe + "\n")


def main(argv, env, ein=0, aus=None):
    aus = aus or sys.stdout
    if list(argv[1:]) == ["--version"]:  # `copilot --version`: sofort eine feste Zeile, Exit 0, ohne stdin zu lesen (Befund eines Testlaufs)
        if fehlende_cache_dateien(env):  # das Vorentpacken des Produkts (Paketform, Punkt 6): fehlende Pfade anlegen, sonst nichts schreiben
            entpacke(env)
        aus.write(VERSION_ZEILE)
        aus.flush()
        return 0
    if fehlende_cache_dateien(env):  # wie die echte CLI ohne entpacktes Paket: Fehler, keine Sitzung, nichts geschrieben
        sys.stderr.write(ENTPACKFEHLER)
        sys.stderr.flush()
        return 1
    alt = None
    if os.isatty(ein):  # wie die echte CLI: roher Modus ab dem Start, damit eine Eingabe vor der ersten Ausgabe nicht verloren geht
        import termios
        import tty
        alt = termios.tcgetattr(ein)
        tty.setcbreak(ein, termios.TCSANOW)
    try:
        return _lauf(argv, env, ein, aus)
    finally:
        if alt is not None:
            import termios
            termios.tcsetattr(ein, termios.TCSANOW, alt)


def _lauf(argv, env, ein, aus):
    anfragen = []
    pfad = protokoll_pfad(env)
    schreibe_protokoll(pfad, argv, env, anfragen)
    if not env.get("COPILOT_GITHUB_TOKEN"):
        aus.write("Please use /login to sign in\n")
        aus.flush()
        while lies_zeile(ein, aus) is not None:
            pass
        return 1
    aus.write("Copilot CLI (Attrappe)\n\nConfirm folder trust\nDo you trust the files in this folder?\n❯ 1. Yes\n  2. No\n")
    aus.flush()
    if lies_zeile(ein, aus) is None:
        return 0
    eingabefeld(aus)
    proxy = env.get("HTTPS_PROXY")
    ziel = env.get("COPILOT_ATTRAPPE_ZIEL", "copilot-attrappe.test:443")
    ca = env.get("COPILOT_ATTRAPPE_CA")
    platzhalter = env.get("COPILOT_GITHUB_TOKEN", "")
    while True:
        text = lies_zeile(ein, aus)
        if text is None:
            return 0
        if not text.strip():
            continue
        if text.startswith("!"):  # Shell-Eingabe: die CLI fuehrt den Rest selbst in ihrer Shell aus, ohne Anfrage an das Ziel
            fuehre_aus(text[1:], aus)
            eingabefeld(aus)
            aus.flush()
            continue
        status = 0
        if proxy:
            try:
                status = https_anfrage(proxy, ziel, "/v1/messages", {"Authorization": "Bearer " + platzhalter, "Content-Type": "application/json"},
                                       json.dumps({"messages": [{"role": "user", "content": text}]}).encode("utf-8"), ca)
            except (OSError, ssl.SSLError):
                status = 0
        anfragen.append({"status": status})
        schreibe_protokoll(pfad, argv, env, anfragen)
        aus.write("Anfrage an das Ziel: HTTP %d\n" % status)
        z = verstehe(text)
        if z is None:
            aus.write("bereit\n")
        else:
            n, art, arg = z
            if art in ("BEFEHL", "BUENDEL"):
                fuehre_aus(arg, aus)
            elif art == "SUCHE":
                aus.write("SUCHE nicht unterstuetzt\n")
            aus.write("FERTIG T%d\n" % n)
        eingabefeld(aus)
        aus.flush()


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv, dict(os.environ)))
    except (KeyboardInterrupt, BrokenPipeError):
        sys.exit(0)
    except Exception as e:  # kein Traceback als erste Zeile: eine feste Zeile, dann nur der Name der Ausnahme (nie ihr Text)
        sys.stderr.write(ABBRUCH_ZEILE + type(e).__name__ + "\n")
        sys.stderr.flush()
        sys.exit(70)
