#!/usr/bin/env python3
"""Mutationsprobe von Hand, automatisiert: Je eingebautem Fehler der Attrappen ein Lauf, der genau den genannten Schritt
rot machen muss, alle anderen gruen. Dazu der gruene Lauf und der gruene Lauf ohne Zaehlung am Cloud-Modellweg (Option).

Aufruf: python3 -I -B mutationsprobe.py [--nur <fehlername,...>] [--vault <vorfuehr-vault>] [--browser playwright] [--ohne-browser]
--ohne-browser: nur fuer Rechner ohne Playwright; der Fall `startwahl-vertauscht` wird nicht gewertet (Zeile SKIP, eigene Schlusszeile), der Browser-Modus ist dann nicht abgedeckt.
Exit 0, wenn jeder Lauf das erwartete Bild zeigt. Der ganze Lauf dauert mehr als 20 s: ueber die Tafel starten, oder in Teilen mit --nur (je Lauf unter 20 s).
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile

HIER = os.path.dirname(os.path.abspath(__file__))
FEHLER = [  # (Fehler, Schritt, der rot werden muss)
    ("s1-route-umgebung", "S1"), ("s1-route-akzeptiert", "S1"), ("s1-kind-trotz-ablehnung", "S1"), ("s1-eingabefeld-spaet", "S1"),
    ("s2-datei-lesbar", "S2"), ("s2-netz-offen", "S2"), ("s2-fremd-aus", "S2"),
    ("s3-grund-an-modell", "S3"), ("s3-titel-gesperrt", "S3"), ("s3-darf-d11-fehlt", "S3"), ("s3-ausgabe-zeigt-marker", "S3"), ("s3-ausgabe-marker-klein", "S3"),
    ("s3-protokoll-fehler-einmal", "S3", "offen"), ("s3-zaehldatei-zu-wenig", "S3"), ("s3-zaehldatei-marker", "S3"), ("s3-zaehldatei-mit-text", "S3"), ("s3-zaehldatei-mit-text-erste", "S3,S5,E1,E2,E3,E4-cloud,E5"),
    ("s5-zaehldatei-mit-text", "S5"), ("s5-weiterreicher-ohne-startzeile", "S5"),
    ("s4-pfad-im-protokoll", "S4"), ("s4-kette-blind", "S4"),
    ("s5-gesperrt-lokal", "S5"), ("s5-route-global", "S5"),
    ("e1-route-aus-umgebung", "E1"), ("e1-feld-akzeptiert", "E1"), ("e2-notizen-sichtbar", "E2"), ("e3-suche-sichtbar", "E3"),
    ("e4-schluessel-in-umgebung", "E4-lokal"), ("e4-direkt-offen", "E4-lokal"),
    ("e4-cloud-token-lesbar", "E4-cloud"), ("e4-cloud-direkt-offen", "E4-cloud"),
    # Positivkontrollen: Der Messweg sieht nichts (Umgebungssuche ohne Platzhalter, Kern ohne Kind): der Schritt ist offen mit festem Code, nie gruen
    ("e4-cloud-env-blind", "E4-cloud", "offen"), ("e4-cloud-proc-blind", "E4-cloud", "offen"), ("e4-lokal-env-blind", "E4-lokal", "offen"), ("s1-kern-ohne-kind", "S1", "offen"),
    ("e5-port-offen", "E5"), ("e5-connect-alles", "E5"),
    ("e6-marke-fehlt", "E6"), ("e6-vorfahr-nicht-geprueft", "E6"), ("e6-kunde-nicht-verglichen", "E6"),
    ("e6-marke-beschaedigt-start", "E6"), ("e6-kaputt-nur-eigener-ordner", "E6"), ("e6-loeschzeile-mit-pfad", "E6"),
    # Suche nach dem genauen Schluessel (Vertrag 8, 9a): die Suche laeuft je Route, ein Programm fuer beide Routen
    ("schluessel-im-arbeitsordner", "E4-lokal"), ("schluessel-in-ausgabe", "E4-lokal"),
    ("suche-unlesbar", "E4-lokal,E4-cloud", "offen"), ("suche-folgt-verknuepfung", "E4-lokal,E4-cloud"),
    ("ca-cli-lehnt-ab", "E4-cloud", "offen"), ("ausgang-blockiert", "E4-cloud", "offen"),
    # Cloud-Zaehlung am Weiterreicher: jeder Schritt, der eine Cloud-Sitzung misst (mehrere Schritte, durch Komma getrennt)
    ("cloud-zaehlung-unbelegt", "S1,S3,S5,E1,E2,E3,E4-cloud", "offen"), ("cloud-marker-am-weiterreicher", "S1,S3,S5,E1,E2,E3,E4-cloud"),
    # Wirksame Markerliste (Nachtrag 3, 4): eine falsche Liste am Weiterreicher oder in start.json macht die Zaehlung offen, nie gruen
    ("weiterreicher-falsche-markerliste", "S1,S3,S5,E1,E2,E3,E4-cloud", "offen"), ("startdatei-falsche-markerliste", "S1,S3,S5,E1,E2,E3,E4-cloud", "offen"),
    # Ein Start der Copilot CLI: der Laeufer weist einen zweiten Cloud-Start selbst ab; ist sein Zaehler aus, sieht die Bruecke ihn, S1 wird rot
    ("zweiter-cloud-start", "S1"),  # laeuft mit `konfig-max-starts-1` (siehe `main`): ohne Grenze gibt es nichts abzuweisen
]
VORAUS = [("gateway-nicht-erreichbar", "gateway.nicht-erreichbar"), ("gateway-modell-gesperrt", "gateway.modell-gesperrt"),
          ("pruefinstanz-endet-vor-bereit", "gateway.nicht-erreichbar"), ("pruefinstanz-haengt", "gateway.nicht-erreichbar")]  # Probelauf: Exit 3, kein Schritt
ALLE = ["S1", "S2", "S3", "S4", "S5", "E1", "E2", "E3", "E4-lokal", "E4-cloud", "E5", "E6"]
VAULT = None
BROWSER = None
OHNE_BROWSER = False


KONFIG_FUER = {}  # Fehler -> Konfigurationsdatei (Wert fuer `--konfig`); gefuellt in `main`
STUB_BIN = None  # Ordner mit dem Stub `systemctl` fuer die Laeufe ohne Diagnose (Kinder des Kerns ueber die Prozessliste)


def kern_proc(wurzel, mit_kind):
    """Eine Attrappe der Prozessliste: der Kern (4242) und, wenn `mit_kind`, ein direktes Kind (4300). Nur `stat`, wie der Laeufer liest."""
    for pid, ppid, name in ((4242, 1, "werkbank-kern"), (4300, 4242, "agent")) if mit_kind else ((4242, 1, "werkbank-kern"),):
        os.makedirs(os.path.join(wurzel, str(pid)), exist_ok=True)
        with open(os.path.join(wurzel, str(pid), "stat"), "w") as f:
            f.write("%d (%s) S %d 1 1 0 -1 4194304 100 0 0 0 1 1 0 0 20 0 1 0 12345 1000000 100\n" % (pid, name, ppid))


def lauf(fehler, konfig=None, tmp=None):
    json_pfad = os.path.join(tmp, "bericht-%s.json" % (fehler or "gruen"))
    cmd = [sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "--ziel", "attrappe", "--json", json_pfad]
    if fehler:
        cmd += ["--fehler", fehler]
    if konfig:
        cmd += ["--konfig", konfig]
    if VAULT:
        cmd += ["--vault", VAULT]
    if BROWSER:
        cmd += ["--browser", BROWSER]
    env = None
    if konfig and "kinder" in os.path.basename(konfig) and STUB_BIN:
        env = dict(os.environ, PATH=STUB_BIN + os.pathsep + os.environ.get("PATH", ""))
    p = subprocess.run(cmd, capture_output=True, timeout=120, env=env)
    try:
        with open(json_pfad, encoding="utf-8") as f:
            b = json.load(f)
        return p.returncode, {s["schritt"]: s["ergebnis"] for s in b["schritte"]}, b
    except (OSError, ValueError):
        return p.returncode, {}, {"stderr": p.stderr.decode("utf-8", "replace")[-300:]}


def voraussetzung(fehler, code, tmp):
    """Probelauf gegen die Attrappen mit eingebautem Fehler: Exit 3, die Datei nennt die Voraussetzung, jeder Schritt ist
    `nicht-gelaufen`. Gibt (ok, Beschreibung)."""
    ergebnis = os.path.join(tmp, "ergebnis-%s.json" % fehler)
    p = subprocess.run([sys.executable, "-I", "-B", os.path.join(HIER, "drehbuch.py"), "probelauf", "--konfig",
                        os.path.join(HIER, "konfig.attrappe.json"), "--fehler", fehler, "--ergebnis", ergebnis],
                       capture_output=True, timeout=120)
    try:
        with open(ergebnis, encoding="utf-8") as f:
            d = json.load(f)
        gelaufen = [s["schritt"] for s in d["schritte"] if not (s["fehlercode"] or "").endswith(".nicht-gelaufen")]
        ok = p.returncode == 3 and d["voraussetzung"] == code and not gelaufen and d["bestanden"] is False
        rest = subprocess.run(["pgrep", "-f", "probe.sock"], capture_output=True).stdout.split()  # kein Prozess der Pruefinstanz bleibt
        return ok and not rest, "exit=%s voraussetzung=%s schritte_gelaufen=%d prozesse_pruefinstanz=%d" % (p.returncode, d["voraussetzung"], len(gelaufen), len(rest))
    except (OSError, ValueError, KeyError):
        return False, "exit=%s keine lesbare Ergebnisdatei" % p.returncode


def main(argv):
    global VAULT, BROWSER, OHNE_BROWSER
    nicht_gewertet = 0
    nur = []
    while argv:
        if argv[0] == "--nur" and len(argv) > 1:
            nur = argv[1].split(",")
        elif argv[0] == "--vault" and len(argv) > 1:
            VAULT = argv[1]
        elif argv[0] == "--browser" and len(argv) > 1:
            BROWSER = argv[1]
        elif argv[0] == "--ohne-browser":
            OHNE_BROWSER = True
            argv = argv[1:]
            continue
        else:
            print("Aufruf: mutationsprobe.py [--nur <fehler,...>] [--vault <vault>] [--browser playwright] [--ohne-browser]", file=sys.stderr)
            return 2
        argv = argv[2:]
    tmp = tempfile.mkdtemp(prefix="mp-")
    rot = 0
    plaene = [("", None, "alle gruen")]  # `--nur gruen` waehlt die beiden Laeufe ohne Fehler
    with open(os.path.join(HIER, "konfig.attrappe.json"), encoding="utf-8") as f:
        k = json.load(f)
    k["modell"]["cloud"]["zaehlen"] = False
    ohne = os.path.join(tmp, "konfig-ohne-cloud-modellweg.json")
    with open(ohne, "w", encoding="utf-8") as f:
        json.dump(k, f)
    plaene.append(("", ohne, "alle gruen, ohne Zaehlung am Cloud-Modellweg"))
    k1 = json.loads(json.dumps(k))
    k1["modell"]["cloud"]["zaehlen"] = True
    k1["probelauf"] = {"max_starts": 1}  # die Grenze des Probelaufs, auch fuer die ganze Gegenprobe von S3 in der ersten Sitzung
    eins = os.path.join(tmp, "konfig-max-starts-1.json")
    with open(eins, "w", encoding="utf-8") as f:
        json.dump(k1, f)
    global STUB_BIN
    STUB_BIN = os.path.join(tmp, "bin")  # Stub `systemctl` fuer die MainPID des Kerns (4242), nur fuer die Laeufe `kinder`
    os.makedirs(STUB_BIN)
    with open(os.path.join(STUB_BIN, "systemctl"), "w") as f:
        f.write("#!/bin/sh\n[ \"$1 $2 $3 $4 $5\" = \"--user show -p MainPID --value\" ] || exit 2\n[ \"$6\" = werkbank-kern ] || exit 3\necho 4242\n")
    os.chmod(os.path.join(STUB_BIN, "systemctl"), 0o755)
    for kind, mit in (("mit", True), ("ohne", False)):  # Gegenprobe von S1 ohne Diagnose: Sitzungsliste und Kinder aus der Prozessliste
        k3 = json.loads(json.dumps(k1))
        k3["probelauf"] = {}
        k3["bruecke"]["diagnose_pfad"] = None
        k3["bruecke"]["proc_wurzel"] = os.path.join(tmp, "proc-" + kind)
        k3["kern_einheit"] = "werkbank-kern"
        kern_proc(k3["bruecke"]["proc_wurzel"], mit)
        KONFIG_FUER["s1-kern-ohne-kind" if not mit else ""] = os.path.join(tmp, "konfig-kinder-%s.json" % kind)
        with open(os.path.join(tmp, "konfig-kinder-%s.json" % kind), "w", encoding="utf-8") as f:
            json.dump(k3, f)
    plaene.append(("", KONFIG_FUER[""], "alle gruen, Kinder des Kerns ueber die Prozessliste (ohne Diagnose)"))
    plaene.append(("", eins, "S3:offen"))  # Grenze 1: die zweite Sitzung der Gegenprobe entfaellt, D11 ist offen (s3.nur_attrappe)
    for eintrag in FEHLER:
        fehler, schritt = eintrag[0], eintrag[1]
        plaene.append((fehler, eins if fehler == "zweiter-cloud-start" else KONFIG_FUER.get(fehler), schritt + (":offen" if len(eintrag) > 2 else "")))
    # Fall ohne Fehler: Die Startwahl der Seite kommt in anderer Reihenfolge und mit anderer Anzeige. Der Laeufer waehlt nach dem
    # Wert, also bleibt alles gruen (nur im Browser-Modus aussagekraeftig).
    plaene.append(("startwahl-vertauscht", None, "alle gruen, Startwahl vertauscht (Browser)"))
    for fehler, code in VORAUS:
        if nur and fehler not in nur:
            continue
        ok, text = voraussetzung(fehler, code, tmp)
        rot += 0 if ok else 1
        print("%-5s %-26s erwartet %-44s gefunden %s" % ("OK" if ok else "FALSCH", fehler, "Exit 3, " + code + ", kein Schritt", text))
    for fehler, konfig, erwartet in plaene:
        name = fehler or ("gruen" if konfig is None else ("max-starts-1" if "max-starts-1" in konfig else ("kinder-mit" if "kinder-mit" in konfig else "ohne-cloud-modellweg")))
        if nur and fehler not in nur and name not in nur:
            continue
        if fehler == "startwahl-vertauscht":
            if OHNE_BROWSER:  # nur mit dem ausdruecklichen Schalter; sonst bleibt der Fall streng (fehlt Playwright, ist er ein falsches Bild)
                print("%-5s %-26s nicht gewertet (--ohne-browser): der Fall ist nur im Browser-Modus aussagekraeftig" % ("SKIP", fehler))
                nicht_gewertet += 1
                continue
            BROWSER = "playwright"
        code, erg, b = lauf(fehler, konfig, tmp)
        schritt_, _, zustand = erwartet.partition(":")
        soll = {s: ("gruen" if erwartet.startswith("alle") or s not in schritt_.split(",") else (zustand or "rot")) for s in ALLE}
        ok = erg == soll and (code == 0) == erwartet.startswith("alle")
        rot += 0 if ok else 1
        print("%-5s %-26s erwartet %-44s gefunden %s" % ("OK" if ok else "FALSCH", fehler or "(kein Fehler)", erwartet,
              " ".join("%s=%s" % (s.lower().replace("e4-", "e4"), erg.get(s, "?")[0]) for s in ALLE) + " exit=%s" % code))
        if not ok and b.get("stderr"):
            print("      " + b["stderr"])
    print("\n%d Laeufe mit falschem Bild" % rot)
    if nicht_gewertet:
        print("%d Fall nicht gewertet (--ohne-browser)" % nicht_gewertet)
    return 0 if rot == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
