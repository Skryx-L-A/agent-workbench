"""wb_datenklasse_regel.py -- die Klassenregel des Datenklassen-Tors an EINER Stelle.

Das Tor shell/wb-datenklasse (klasse, klassen, pruefe --dir, pruefe --klasse) und die
Entscheidungsstelle laden dieses Modul im Prozess, neben realpath der eigenen Datei
(nie ueber sys.path oder PYTHONPATH). Schnittstelle (docs/DATENKLASSE.md, Abschnitt "Modul"):

  klasse_fuer(ordner, heim=None, lage=None, marker=True)
      -> {"klasse", "kunden", "quelle", "gruende", "warnungen"} | {"fehler", "kennwort"}

  - Wirft nie: jede Ausnahme wird {"fehler": <Text>, "kennwort": <Wort>}. Kennwoerter: kein-start,
    konfig, richtlinie, marker, verzeichnis, kennung, intern (KENNWOERTER).
  - Liest keine Umgebungsvariable. heim=None heisst Heimat laut Benutzerdatenbank (geteuid).
  - Nur ein fehlender Pfad heisst 'kein Marker' (FileNotFoundError, NotADirectoryError). Ein Ordner auf dem Weg
    (Projektordner, Hauptbaum) ohne Betretrecht oder ein unlesbares .git lehnt mit marker ab, nie die schwaechere Klasse.
  - Jede Datei, die dieses Modul liest, geht durch text_lesen oder begrenzt_lesen: oeffnen ohne zu blockieren, nur eine
    regulaere Datei, hoechstens MAX_RICHTLINIE, MAX_MARKER oder MAX_GIT_EINTRAG Bytes. Eine FIFO haelt keinen Aufrufer an,
    ein grosser Marker kostet keinen Speicher; beides lehnt ab (Regel R-KL-DATEI, 31.10.2026).
  - Regel R-KL-WORKTREE (31.10.2026): Ein Hauptbaum zaehlt fuer die Liste `projekte` nur, wenn seine Git-Verwaltung den
    Worktree zurueck nennt (worktree_belegt). Die eigene Datei .git ist Text des Prozesses im Ordner: Unbelegt heben ihre
    Wege (Marker, Listeneintrag ueber der Untergrenze), senken aber nie. Fehlt die Verwaltung (Hauptbaum verschoben,
    Faelschung), ist die Klasse nicht feststellbar: Ablehnung mit dem Kennwort marker.
  - Regel R-KL-PFAD (31.10.2026): Der Pfad einer Frage kommt ungekuerzt an. Nur die Funktion pfad_des_kernels nimmt `..`
    weg, und zwar so, wie der Kernel es liest: `link/..` ist der Elternordner des Linkziels. Die lexikalische Lesart zaehlt als
    weiterer Weg, weil ein logisches `cd` in Shellskripten so geht; sie hebt nur. Wer vor dieser Frage selbst kuerzt
    (abspath, path.resolve), misst einen anderen Ordner.
  - Marker werden je Aufruf frisch gelesen. Das Ergebnis des Lesers wb_konfig.py (werkbank.json) und
    den Text der Richtlinien behaelt der Prozess, solange jeder Pfad, der sie bestimmt hat, denselben
    Stat-Schluessel hat (Geraet, Inode, Groesse, mtime_ns, ctime_ns, Modus, Eigentuemer; "fehlt" zaehlt
    mit), genommen VOR dem Lesen. Je Heimat eine eigene Merkstelle. Ein Fehler wird nie gemerkt.
    Siehe KONFIG_CACHE und RICHTLINIE_CACHE. Der Code des Lesers bleibt je Prozess und Pfad geladen.
  - heim reicht die Lage an wb_konfig.lade(heim=...) weiter: die werkbank.json der Person darf die
    Schalter verschaerfen, auch wenn der Prozess unter einer anderen Kennung laeuft.
  - "quelle" und die Zeile KLASSE<TAB>KUNDEN<TAB>QUELLE sind derselbe Vertrag wie `wb-datenklasse klasse`.
  - marker=False (Regel R-KL-MEHRKONTO, 27.10.2026): die Lage laesst NUR marker_auf_dem_weg aus und betritt den
    Ordner selbst nicht (betretbar, R-KL-MEHRKONTO-OHNE-ORDNER, 29.10.2026). Ein unpruefbares .git (Hauptbaum) zaehlt
    dann nur ohne Liste projekte der Systemrichtlinie nicht; mit ihr lehnt es ab wie vorher. Untergrenze,
    Systemliste, Schalter, Nutzerrichtlinie und fail closed bleiben. Die Entscheidungsstelle ruft so im
    Mehrkontenbetrieb (stelle.konto gesetzt); dort kommt die Projektklasse aus ihrem Register. Vorgabe True:
    das Tor und jeder andere Aufrufer bekommen dasselbe Ergebnis wie vorher.

Die Regel (Punkte 0d und 0w, Beschluesse E-02 und D-2):
  - Die Klasse ist das Maximum ueber ALLE Marker `.datenklasse` vom Ordner bis zur Wurzel, ueber den
    genannten und den aufgeloesten Pfad und bei einem verknuepften Git-Worktree auch ueber den gleichen
    Ort im Hauptbaum. Ein tieferer Marker senkt nie. Kunden addieren sich.
  - Eine kunde:-Zeile hebt auf mindestens CUSTOMER, wenn der Schalter datenklasse.kundeHebtAufCustomer
    es sagt; QUELLE heisst dann `<marker>:kunde`.
  - Untergrenze ist der Schalter datenklasse.unmarkiert (INTERNAL | CUSTOMER | kein-start) oder, fuer
    ein Projekt der Liste `projekte` der Systemrichtlinie, dessen Klasse. kein-start ohne Marker: Fehler
    mit Kennwort kein-start; mit Marker gilt UNTERGRENZE_KEIN_START.
  - Die Systemrichtlinie gilt nur, wenn Datei und Ordner SYSTEM_EIGENTUEMER gehoeren und weder gruppen-
    noch weltschreibbar sind; liegt sie unsicher, gilt mit Warnung STRENGE_RICHTLINIE (Cloud
    hoechstens PUBLIC, keine Projektliste). Die Nutzerrichtlinie unter der Heimat darf nur
    verschaerfen; ihre Projektliste zaehlt nie. Eine ungueltige Systemebene von werkbank.json
    behandelt der Leser wb_konfig.py (je Schluessel der strengste Wert, task).

Schalter datenklasse.lanEndpunkte (decision 2026-10-28): Lage.lan_endpunkte() liest die Karte Host -> Klasse (Betreiberebene,
Vorgabe {}) und gibt nur die brauchbaren Eintraege weiter; die Auswertung steht in `route --registry` des Tors. Die Klasse
der Systemrichtlinie (datenklasse.json) bekommt dafuer keinen neuen Schluessel (SCHLUESSEL bleibt).

Testnaht: keine Variable. Das Tor reicht seine umschreibbaren Konstanten (SYSTEM_ORDNER,
SYSTEM_EIGENTUEMER, NUTZER_HEIM) als Argumente von Lage herein; ohne sie gelten die Werte hier.
"""
import importlib.util
import json
import os
import pwd
import re
import stat
import sys

ORDER = ["NONE", "PUBLIC", "INTERNAL", "CONFIDENTIAL", "CUSTOMER"]
MARKER = ".datenklasse"
RICHTLINIE_NAME = "datenklasse.json"
SYSTEM_ORTE = {"darwin": "/Library/Application Support/agent-workbench",
               "linux": "/etc/agent-workbench"}
SYSTEM_ORDNER = SYSTEM_ORTE.get("linux" if sys.platform.startswith("linux") else sys.platform, "")
SYSTEM_EIGENTUEMER = 0

# Produktvorgabe ohne Systemrichtlinie. Seit der Entscheidung "kein Cloud-Aufruf ohne Firmenprofil"
# (23.10.2026, nur development) traegt die Route cloud hier nur noch PUBLIC: ob Daten der Klasse
# INTERNAL oder hoeher an einen Cloud-Anbieter gehen duerfen, entscheidet das Firmenprofil, also die
# Systemrichtlinie datenklasse.json des Betreibers, nie die Vorgabe des Produkts. Lokal heisst: Anbieter
# mit kind 'local' in der Registry UND eine Adresse auf dieser Maschine (oder keine); alles andere ist Cloud.
PRODUKT_QUELLE = "Produktvorgabe"
PRODUKT_RICHTLINIE = {
    "routen": [
        {"id": "lokal", "art": "lokal", "maxKlasse": "CUSTOMER"},
        {"id": "cloud", "maxKlasse": "PUBLIC"},
    ],
    "kunden": {},
    "nutzerprofile": {},
    "nutzerZuordnung": {},
}
# Gilt statt der Systemrichtlinie, wenn sie da, aber unsicher abgelegt ist (Eigentuemer, Rechte, Verweis,
# keine regulaere Datei): strenger als die Produktvorgabe, nie lockerer (decision 2026-10-23, Variante C).
STRENGE_RICHTLINIE = {
    "routen": [
        {"id": "lokal", "art": "lokal", "maxKlasse": "CUSTOMER"},
        {"id": "cloud", "maxKlasse": "PUBLIC"},
    ],
    "kunden": {},
    "nutzerprofile": {},
    "nutzerZuordnung": {},
}
# REGEL R-KL-DATEI: Grenzen des Lesens. MAX_RICHTLINIE wie MAX_BYTES in wb_konfig.py (werkbank.json), MAX_MARKER und
# MAX_GIT_EINTRAG weit ueber dem, was eine echte Datei dieser Art traegt.
MAX_RICHTLINIE = 1 << 20
MAX_MARKER = 64 << 10
MAX_GIT_EINTRAG = 8 << 10
OEFFNEN = os.O_RDONLY | os.O_NONBLOCK   # eine FIFO ohne Schreiber haelt open sonst ohne Frist an

KUNDE_ZEILE = re.compile(r"^\s*kunde\s*:(.*)$", re.IGNORECASE)
KUNDE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]*$")
SCHLUESSEL = {"routen", "kunden", "nutzerprofile", "nutzerZuordnung",
              "gesperrteHarnesses", "projekte"}
REGEL_FELDER = {"id", "maxKlasse", "modell", "provider", "art", "harness", "host"}

KONFIG_MODUL = "wb_konfig.py"   # Konfigurationsleser neben diesem Modul (Stufe 1e)
SCHALTER_WERTE = ("INTERNAL", "CUSTOMER", "kein-start")
UNTERGRENZE_KEIN_START = "INTERNAL"   # Projekte mit Marker, wenn der Schalter kein-start ist
KEIN_START_TEXT = ("Projekt ohne Datenklasse; Marker .datenklasse anlegen "
                   "(Vorgabe des Betreibers: kein-start)")
KENNWOERTER = ("kein-start", "konfig", "richtlinie", "marker", "verzeichnis", "kennung", "intern")


class Abgelehnt(Exception):
    """Jeder Fehler, der einen Start verhindert. kennwort: eines aus KENNWOERTER."""

    def __init__(self, text, kennwort="richtlinie"):
        super().__init__(text)
        self.kennwort = kennwort


class KeinStart(Abgelehnt):
    """Schalter kein-start und Projekt ohne Marker: Exit 3 auch fuer `klasse`."""

    def __init__(self, text=KEIN_START_TEXT):
        super().__init__(text, "kein-start")


def klasse_pruefen(wert, wo, none_erlaubt=False, kennwort="richtlinie"):
    k = wert.strip().upper() if isinstance(wert, str) else ""
    if k not in ORDER or (k == "NONE" and not none_erlaubt):
        raise Abgelehnt("unbekannte Datenklasse '%s' in %s" % (wert, wo), kennwort)
    return k


# ── Richtlinie ────────────────────────────────────────────────────────────────

def stat_schluessel(st):
    """Stat-Schluessel einer Datei: ctime_ns und Inode fangen auch eine Aenderung mit gleicher Groesse und mtime."""
    return (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns, st.st_mode, st.st_uid)


def zustand(pfade, folgen=True):
    """Je Pfad der Stat-Schluessel, 'fehlt' oder ('fehler', errno). folgen=False: lstat (Verweis selbst)."""
    out = []
    for p in pfade:
        try:
            out.append(stat_schluessel(os.stat(p) if folgen else os.lstat(p)))
        except FileNotFoundError:
            out.append("fehlt")
        except OSError as exc:
            out.append(("fehler", exc.errno))
    return tuple(out)


def begrenzt_lesen(fd, grenze):
    """Text der Datei hinter FD (UTF-8). Nur eine regulaere Datei mit hoechstens GRENZE Bytes; die Pruefung steht wie in
    wb_konfig.py vor dem Lesen, und mehr als GRENZE + 1 Bytes liest das Modul nie, auch wenn die Datei waechst.
    Wirft OSError oder ValueError; die Aufrufer machen daraus ihre Ablehnung."""
    st = os.fstat(fd)
    if not stat.S_ISREG(st.st_mode):
        raise ValueError("keine regulaere Datei")
    if st.st_size > grenze:
        raise ValueError("ueber %d Bytes" % grenze)
    teile, rest = [], grenze + 1
    while rest > 0:
        stueck = os.read(fd, rest)
        if not stueck:
            return b"".join(teile).decode("utf-8")
        teile.append(stueck)
        rest -= len(stueck)
    raise ValueError("ueber %d Bytes" % grenze)


def text_lesen(pfad, grenze):
    """REGEL R-KL-DATEI: oeffnet ohne zu blockieren und liest ueber begrenzt_lesen. Wirft OSError oder ValueError."""
    fd = os.open(pfad, OEFFNEN)
    try:
        return begrenzt_lesen(fd, grenze)
    finally:
        os.close(fd)


# Text einer Richtlinie je (Pfad, uid, euid) -> (Stat-Schluessel der geoeffneten Datei, Text). Der Schluessel kommt
# aus fstat des schon geoeffneten Deskriptors, also VOR dem Lesen und von genau der Datei, die gelesen wird. Gemerkt
# wird nur der Text, nie das gepruefte Ergebnis: richtlinie_pruefen laeuft je Aufruf (realpath der Projektliste,
# Warnungen). Ein Lese- oder JSON-Fehler loescht den Eintrag.
RICHTLINIE_CACHE = {}


def json_lesen(pfad, fd=None):
    try:
        if fd is None:
            fd = os.open(pfad, OEFFNEN | getattr(os, "O_NOFOLLOW", 0))
        ort = (pfad, os.getuid(), os.geteuid())
        try:
            st = os.fstat(fd)
            if not stat.S_ISREG(st.st_mode):
                raise ValueError("keine regulaere Datei")
            schluessel = stat_schluessel(st)
            gemerkt = RICHTLINIE_CACHE.get(ort)
            if gemerkt is not None and gemerkt[0] == schluessel:
                text = gemerkt[1]
            else:
                RICHTLINIE_CACHE.pop(ort, None)
                text = begrenzt_lesen(fd, MAX_RICHTLINIE)
        finally:
            os.close(fd)
        daten = json.loads(text)
        RICHTLINIE_CACHE[ort] = (schluessel, text)
        return daten
    except (OSError, ValueError) as exc:
        RICHTLINIE_CACHE.pop((pfad, os.getuid(), os.geteuid()), None)
        raise Abgelehnt("Richtlinie unlesbar (%s): %s" % (exc, pfad))


def richtlinie_pruefen(rl, wo):
    if not isinstance(rl, dict):
        raise Abgelehnt("Richtlinie ist kein Objekt: %s" % wo)
    if "vorgabeKlasse" in rl:
        raise Abgelehnt("vorgabeKlasse in %s gibt es nicht mehr; die Klasse ohne Marker steht in "
                        "werkbank.json unter datenklasse.unmarkiert" % wo)
    for k in rl:
        if not k.startswith("_") and k not in SCHLUESSEL:
            raise Abgelehnt("unbekannter Schluessel '%s' in %s" % (k, wo))
    routen = rl.get("routen", [])
    if not isinstance(routen, list):
        raise Abgelehnt("routen ist keine Liste: %s" % wo)
    for r in routen:
        if not isinstance(r, dict) or "maxKlasse" not in r:
            raise Abgelehnt("Route ohne maxKlasse in %s" % wo)
        for f in r:
            if not f.startswith("_") and f not in REGEL_FELDER:
                raise Abgelehnt("unbekanntes Feld '%s' einer Route in %s" % (f, wo))
        if "host" in r and (not isinstance(r["host"], str) or not r["host"].strip()):
            raise Abgelehnt("host einer Route muss ein Muster sein: %s" % wo)
        r["maxKlasse"] = klasse_pruefen(r["maxKlasse"], wo, none_erlaubt=True)
    for feld in ("kunden", "nutzerprofile", "nutzerZuordnung"):
        if not isinstance(rl.get(feld, {}), dict):
            raise Abgelehnt("%s ist kein Objekt: %s" % (feld, wo))
    for kid, kv in rl.get("kunden", {}).items():
        if kid.startswith("_"):
            continue
        if not isinstance(kv, dict) or not isinstance(kv.get("erlaubteRouten", []), list):
            raise Abgelehnt("Kunde %s: erlaubteRouten muss eine Liste sein: %s" % (kid, wo))
    for name, prof in rl.get("nutzerprofile", {}).items():
        if not isinstance(prof, dict) or not isinstance(prof.get("gesperrteHarnesses", []), list):
            raise Abgelehnt("Nutzerprofil %s: gesperrteHarnesses muss eine Liste sein: %s"
                            % (name, wo))
    if not isinstance(rl.get("gesperrteHarnesses", []), list):
        raise Abgelehnt("gesperrteHarnesses muss eine Liste sein: %s" % wo)
    projekte = rl.get("projekte", [])
    if not isinstance(projekte, list):
        raise Abgelehnt("projekte ist keine Liste: %s" % wo)
    gesehen = set()
    for e in projekte:
        if not isinstance(e, dict) or set(k for k in e if not k.startswith("_")) != {"pfad", "klasse"}:
            raise Abgelehnt("Eintrag in projekte braucht genau pfad und klasse: %s" % wo)
        if not isinstance(e["pfad"], str) or not os.path.isabs(e["pfad"]):
            raise Abgelehnt("projekte: pfad muss absolut sein (%r) in %s" % (e["pfad"], wo))
        e["klasse"] = klasse_pruefen(e["klasse"], wo)
        e["echt"] = os.path.realpath(e["pfad"])
        if e["echt"] in gesehen:
            raise Abgelehnt("projekte: Pfad %s steht doppelt in %s" % (e["pfad"], wo))
        gesehen.add(e["echt"])
    return rl


# ── Projektklasse ─────────────────────────────────────────────────────────────

def marker_lesen(p):
    """-> (Klasse, [Kunden]) einer Markerdatei. Jeder Fehler lehnt ab."""
    if not os.path.isfile(p):
        raise Abgelehnt("%s ist keine lesbare Datei" % p, "marker")
    try:
        text = text_lesen(p, MAX_MARKER)
    except (OSError, ValueError) as exc:
        raise Abgelehnt("Marker unlesbar (%s): %s" % (exc, p), "marker")
    woerter = text.split()
    k = klasse_pruefen(woerter[0] if woerter else "", p, kennwort="marker")
    kunden = []
    for zeile in text.splitlines():
        m = KUNDE_ZEILE.match(zeile)
        if not m:
            continue
        kid = m.group(1).strip()
        if not KUNDE_ID.match(kid):
            raise Abgelehnt("ungueltige Zeile 'kunde: %s' in %s (genau eine Kunden-ID)" % (kid, p),
                            "marker")
        kunden.append(kid)
    return k, kunden


def pfad_pruefen(p, was, folgen=False):
    """-> os.stat_result oder None. Nur FileNotFoundError und NotADirectoryError heissen 'fehlt'; jeder andere
    Fehler (EACCES, EPERM, ELOOP, ENAMETOOLONG, EIO ...) lehnt mit dem Kennwort marker ab. Ein Pfad, den das Modul
    nicht pruefen darf, wird nie als 'hier liegt nichts' gelesen: ein Marker dahinter koennte die Klasse heben."""
    try:
        return os.stat(p) if folgen else os.lstat(p)
    except (FileNotFoundError, NotADirectoryError):
        return None
    except OSError as exc:
        raise Abgelehnt("%s nicht pruefbar (%s): %s" % (was, exc, p), "marker")


def worktree_eintrag(d):
    """-> (Hauptbaum, Ort relativ zur Wurzel, Worktree-Wurzel, gitdir), wenn D in einem verknuepften Git-Worktree liegt,
    sonst None. Das steht nur so in der Datei .git; ob der Hauptbaum das bestaetigt, sagt worktree_belegt. Ein .git, das sich
    nicht pruefen oder lesen laesst, lehnt ab (Kennwort marker): sonst fiele der Worktree still auf seine eigene Klasse zurueck."""
    cur = d
    while True:
        g = os.path.join(cur, ".git")
        st = pfad_pruefen(g, "Git-Eintrag", folgen=True)
        if st is not None and stat.S_ISREG(st.st_mode):
            try:
                zeile = text_lesen(g, MAX_GIT_EINTRAG).split("\n", 1)[0].strip()
            except (OSError, ValueError) as exc:
                raise Abgelehnt("Git-Eintrag unlesbar (%s): %s" % (exc, g), "marker")
            if not zeile.startswith("gitdir:"):
                return None
            gitdir = os.path.normpath(os.path.join(cur, zeile[len("gitdir:"):].strip()))
            teile = gitdir.split(os.sep)
            if len(teile) >= 3 and teile[-2] == "worktrees" and teile[-3] == ".git":
                return os.sep.join(teile[:-3]) or os.sep, os.path.relpath(d, cur), cur, gitdir
            return None
        if st is not None and stat.S_ISDIR(st.st_mode):
            return None
        oben = os.path.dirname(cur)
        if oben == cur:
            return None
        cur = oben


def hauptbaum(d):
    """Hauptbaum und relativer Ort, wenn D in einem verknuepften Git-Worktree liegt, wie worktree_eintrag."""
    eintrag = worktree_eintrag(d)
    return eintrag[:2] if eintrag else None


def worktree_belegt(haupt, wurzel, gitdir):
    """REGEL R-KL-WORKTREE: Bestaetigt die Git-Verwaltung GITDIR im Hauptbaum, dass WURZEL ihr Worktree ist?

    True: <gitdir>/gitdir nennt <wurzel>/.git, und GITDIR liegt wirklich unter <haupt>/.git/worktrees. Der Vergleich geht ueber
    den Pfad, nicht die Inode: Ein harter oder symbolischer Verweis auf das .git eines echten Worktrees bestaetigt nichts.
    False: Die Verwaltung ist da, nennt aber etwas anderes oder nichts (Worktree von Hand verschoben, Faelschung).
    Abgelehnt (marker): Die Verwaltung fehlt (Hauptbaum verschoben, Faelschung), oder der Rueckverweis ist keine lesbare Datei.
    Dann sind die Marker des Hauptbaums nicht feststellbar. Liest nur ueber text_lesen, also nie blockierend."""
    st = pfad_pruefen(gitdir, "Git-Verwaltung", folgen=True)
    if st is None or not stat.S_ISDIR(st.st_mode):
        raise Abgelehnt("Hauptbaum des Worktrees %s nicht auffindbar (verschoben?): %s" % (wurzel, gitdir), "marker")
    zeiger = os.path.join(gitdir, "gitdir")
    if pfad_pruefen(zeiger, "Git-Rueckverweis", folgen=True) is None:
        return False
    try:
        ziel = text_lesen(zeiger, MAX_GIT_EINTRAG).split("\n", 1)[0].strip()
    except (OSError, ValueError) as exc:
        raise Abgelehnt("Git-Rueckverweis unlesbar (%s): %s" % (exc, zeiger), "marker")
    ziel = os.path.join(gitdir, ziel)   # relativ zur Verwaltung, seit Git 2.48 mit worktree.useRelativePaths
    echt = os.path.realpath(gitdir)
    return (os.path.basename(ziel) == ".git"
            and os.path.realpath(os.path.dirname(ziel)) == os.path.realpath(wurzel)
            and not os.path.islink(os.path.join(wurzel, ".git"))
            and echt == os.path.join(os.path.realpath(haupt), ".git", "worktrees", os.path.basename(echt)))


def pfad_des_kernels(pfad):
    """REGEL R-KL-PFAD: Der absolute PFAD ohne `.`, doppelte Trenner und `..`, ohne die Bedeutung fuer den Kernel zu aendern.
    Der Teil bis zum letzten `..` wird aufgeloest, denn dort entscheidet das Linkziel. Dahinter bleibt der Name, wie er
    genannt ist. Ein Pfad ohne `..` behaelt also seine Verweise (der genannte Weg zaehlt mit)."""
    teile = pfad.split(os.sep)
    if os.pardir not in teile:
        return os.path.normpath(pfad)
    letzter = len(teile) - 1 - teile[::-1].index(os.pardir)
    return os.path.normpath(os.path.join(os.path.realpath(os.sep.join(teile[:letzter + 1])), *teile[letzter + 1:]))


def marker_auf_dem_weg(d):
    """Alle Markerdateien von D bis zur Wurzel (D zuerst). Ein Weg, den der Prozess nicht pruefen darf (etwa ein
    Ordner ohne Betretrecht), lehnt ab; nur ein fehlender Pfad heisst 'kein Marker'."""
    out = []
    while True:
        p = os.path.join(d, MARKER)
        if pfad_pruefen(p, "Marker") is not None:
            out.append(p)
        oben = os.path.dirname(d)
        if oben == d:
            return out
        d = oben


_LESER = {}   # Pfad -> geladenes Modul wb_konfig.py (nur Code, keine Daten)

# Ergebnis des Lesers je (Leserpfad, heim oder None, Heimat, uid, euid) -> {"pfade": Konfig.gelesen des letzten Laufs,
# "schluessel": zustand(pfade) VOR dem Lesen, "mod", "konfig"}. Der Schluessel wird vor lade() mit der Pfadliste
# des Laufs davor genommen; nur wenn lade() genau diese Pfade gelesen oder gesucht hat, gilt das Ergebnis als
# gemerkt. Eine Aenderung waehrend des Lesens faellt so beim naechsten Aufruf auf. Der erste Lauf je Merkstelle
# merkt nur die Pfadliste. Jeder Fehler (lade wirft, ein Schalter ist ungueltig oder fehlt) loescht den Eintrag.
KONFIG_CACHE = {}


def leser_laden(pfad):
    if pfad not in _LESER:
        if not os.path.isfile(pfad):
            raise Abgelehnt("Konfigurationsleser fehlt neben dem Tor: %s" % pfad, "konfig")
        try:
            spec = importlib.util.spec_from_file_location("wb_konfig", pfad)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
        except BaseException as exc:   # auch SystemExit beim Laden: fail closed
            raise Abgelehnt("Konfigurationsleser unbrauchbar (%s: %s): %s"
                            % (type(exc).__name__, exc, pfad), "konfig")
        _LESER[pfad] = mod
    return _LESER[pfad]


class NutzerEbene:
    """Die Nutzerebene aus dem Speicher statt aus der Heimat (Regel R-KL-NUTZER-FD, w62, 31.10.2026): Die Stelle hat im
    Mehrkontenbetrieb kein Recht in der Heimat, der Aufrufer reicht beide Dateien als Deskriptor herein. Beide None: keine
    Nutzerebene, so am Server (R-KL-SERVER-OHNE-NUTZER). Der Leser wb_konfig.py kennt nur Pfade; fuer ihn liegt der Text
    einmal je Anfrage in einem eigenen 0700-Ordner unter ABLAGE und ist danach wieder weg. Das Ergebnis gilt nur
    fuer diese Anfrage: KONFIG_CACHE und RICHTLINIE_CACHE bleiben aussen vor."""

    def __init__(self, datenklasse=None, werkbank=None, ablage=None):
        self.datenklasse, self.werkbank, self.ablage = datenklasse, werkbank, ablage
        self._konfig = None

    def konfig(self, mod):
        if self._konfig is None:
            import tempfile
            with tempfile.TemporaryDirectory(prefix="nutzer-", dir=self.ablage) as heim:
                if self.werkbank is not None:
                    ordner = os.path.join(heim, ".config", "agent-workbench")
                    os.makedirs(ordner, mode=0o700)
                    fd = os.open(os.path.join(ordner, "werkbank.json"), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                    with os.fdopen(fd, "wb") as fh:
                        fh.write(self.werkbank)
                self._konfig = mod.lade(env={"HOME": heim}, heim=heim)
        return self._konfig


class Lage:
    """Was EIN Aufruf liest: Richtlinie und Schalter, jeweils hoechstens einmal, beim ersten Bedarf.

    heim: Heimat fuer Nutzerrichtlinie und Nutzerebene von werkbank.json; None heisst Heimat laut
    Benutzerdatenbank. system_ordner/system_eigentuemer: None heisst die Werte dieses Moduls; ''
    als Ordner heisst: keine Systemrichtlinie. warnung: Rueckruf je Warnung (das Tor schreibt sie
    sofort auf stderr); ohne Rueckruf sammelt die Lage sie in self.warnungen. nutzer: eine NutzerEbene; dann liest
    diese Lage nichts unter der Heimat."""

    def __init__(self, heim=None, system_ordner=None, system_eigentuemer=None, warnung=None, nutzer=None):
        self.heim = heim or None
        self.nutzer = nutzer
        self.system_ordner = SYSTEM_ORDNER if system_ordner is None else system_ordner
        self.system_eigentuemer = (SYSTEM_EIGENTUEMER if system_eigentuemer is None
                                   else system_eigentuemer)
        self.warnungen = []
        self._warnung = warnung
        self._rl = None
        self._konfig = None
        self._schalter = {}
        self._lan = None
        self._merkstelle = None

    def warnung(self, text):
        if self._warnung is None:
            self.warnungen.append(text)
        else:
            self._warnung(text)

    def heimat(self):
        if self.heim:
            return self.heim
        try:
            return pwd.getpwuid(os.geteuid()).pw_dir
        except KeyError:
            raise Abgelehnt("Kennung %d steht nicht in der Benutzerdatenbank" % os.geteuid(),
                            "kennung")

    # -- Richtlinie --

    def system_richtlinie(self):
        """-> (Richtlinie, Quelle) oder None ohne Systemrichtlinie. Unsicher abgelegt: Warnung,
        STRENGE_RICHTLINIE. Kaputtes JSON oder ein falscher Typ: Abgelehnt (fail closed)."""
        if not self.system_ordner:
            return None
        pfad = os.path.join(self.system_ordner, RICHTLINIE_NAME)
        eigen = self.system_eigentuemer
        try:
            ordner = os.lstat(self.system_ordner)
        except FileNotFoundError:
            return None
        except OSError as exc:
            raise Abgelehnt("Systemordner nicht pruefbar (%s): %s" % (exc, self.system_ordner))
        try:
            fd = os.open(pfad, OEFFNEN | getattr(os, "O_NOFOLLOW", 0))
        except FileNotFoundError:
            return None
        except OSError as exc:
            if os.path.islink(pfad):
                return self.streng(pfad, "ein Verweis zaehlt nicht")
            raise Abgelehnt("Systemrichtlinie unlesbar (%s): %s" % (exc, pfad))
        datei = os.fstat(fd)
        grund = None
        if not stat.S_ISDIR(ordner.st_mode):
            grund = "der Ordner ist kein Verzeichnis"
        elif ordner.st_uid != eigen or ordner.st_mode & 0o022:
            grund = "der Ordner gehoert nicht uid %d oder ist gruppen-/weltschreibbar" % eigen
        elif not stat.S_ISREG(datei.st_mode):
            grund = "keine regulaere Datei"
        elif datei.st_uid != eigen or datei.st_mode & 0o022:
            grund = "die Datei gehoert nicht uid %d oder ist gruppen-/weltschreibbar" % eigen
        if grund:
            os.close(fd)
            return self.streng(pfad, grund)
        return richtlinie_pruefen(json_lesen(pfad, fd), pfad), pfad

    def streng(self, pfad, grund):
        """Systemrichtlinie da, aber unsicher abgelegt: laute Warnung, STRENGE_RICHTLINIE."""
        self.warnung("Systemrichtlinie %s ungueltig: %s; es gilt die strenge Richtlinie (Cloud hoechstens "
                     "PUBLIC)" % (pfad, grund))
        return (richtlinie_pruefen(json.loads(json.dumps(STRENGE_RICHTLINIE)), "strenge Richtlinie"),
                "strenge Richtlinie (%s ungueltig)" % pfad)

    def nutzer_richtlinie(self):
        if self.nutzer is not None:
            if self.nutzer.datenklasse is None:
                return None
            pfad = "Nutzerrichtlinie vom Aufrufer"
            try:
                rl = json.loads(self.nutzer.datenklasse.decode("utf-8"))
            except ValueError as exc:
                raise Abgelehnt("Richtlinie unlesbar (%s): %s" % (exc, pfad))
        else:
            pfad = os.path.join(self.heimat(), ".config", "agent-workbench", RICHTLINIE_NAME)
            try:
                os.lstat(pfad)
            except (FileNotFoundError, NotADirectoryError):
                return None
            except OSError as exc:   # etwa ein Ordner, den das Dienstkonto nicht betreten darf: nie still ueberspringen
                raise Abgelehnt("Nutzerrichtlinie nicht pruefbar (%s): %s" % (exc, pfad))
            rl = json_lesen(pfad)
        rl = richtlinie_pruefen(rl, pfad)
        if rl.pop("projekte", None) is not None:
            self.warnung("Projektliste in %s ignoriert: nur die Systemrichtlinie ordnet Projekte ein"
                         % pfad)
        for r in rl.get("routen", []):
            if "host" in r and r["maxKlasse"] == "CUSTOMER":
                self.warnung("Route '%s' in %s hebt nichts: die Nutzerrichtlinie senkt nur, einen "
                             "Endpunkt hebt nur die Systemrichtlinie" % (r.get("id") or "ohne-id", pfad))
        return rl

    def richtlinie(self):
        """-> dict(basis, nutzer, quelle, system). basis ist System- oder Produktrichtlinie; system sagt, ob eine
        Systemrichtlinie (auch die strenge Ersatzrichtlinie einer unsicher abgelegten) die Basis ist."""
        if self._rl is None:
            try:
                sys_rl = self.system_richtlinie()
                if sys_rl:
                    basis, quelle = sys_rl
                else:
                    basis = richtlinie_pruefen(json.loads(json.dumps(PRODUKT_RICHTLINIE)), PRODUKT_QUELLE)
                    quelle = PRODUKT_QUELLE
                self._rl = {"basis": basis, "nutzer": self.nutzer_richtlinie() or {}, "quelle": quelle,
                            "system": bool(sys_rl)}   # auch die strenge Ersatzrichtlinie zaehlt als Systemrichtlinie
            except Abgelehnt:
                RICHTLINIE_CACHE.clear()   # ein Fehler wird nie gemerkt, auch nicht als Text
                raise
        return self._rl

    # -- Schalter --

    def konfig(self):
        """-> (Modul, Konfig) des Lesers wb_konfig.py neben diesem Modul. Die Nutzerebene liegt
        unter der Heimat (heim geht als lade(heim=...) mit); ausser HOME bekommt der Leser keine Umgebung.
        Die Konfig kann aus KONFIG_CACHE stammen und ist dann mit anderen Lagen geteilt: nie veraendern."""
        if self._konfig is None:
            pfad = os.path.join(os.path.dirname(os.path.realpath(__file__)), KONFIG_MODUL)
            mod = leser_laden(pfad)
            if self.nutzer is not None:
                try:
                    k = self.nutzer.konfig(mod)
                except mod.SchemaUngueltig as exc:
                    raise Abgelehnt("Schemadatei ungueltig: %s" % exc, "konfig")
            else:
                k = self.konfig_gemerkt(mod, pfad)
            for m in k.meldungen:
                if "datenklasse" in m or re.match(r"(system|betreiber|nutzer) \(", m):
                    self.warnung("werkbank.json: " + m)
            self._konfig = (mod, k)
        return self._konfig

    def konfig_gemerkt(self, mod, pfad):
        """Der Leser ueber KONFIG_CACHE, je Pfad unter der Heimat."""
        self._merkstelle = (pfad, self.heim, self.heimat(), os.getuid(), os.geteuid())
        eintrag = KONFIG_CACHE.get(self._merkstelle)
        if (eintrag and eintrag["konfig"] is not None and eintrag["mod"] is mod
                and zustand(eintrag["pfade"]) == eintrag["schluessel"]):
            return eintrag["konfig"]
        pfade = eintrag["pfade"] if eintrag else None
        vorher = zustand(pfade) if pfade is not None else None
        KONFIG_CACHE.pop(self._merkstelle, None)
        try:
            k = mod.lade(env={"HOME": self.heimat()}, heim=self.heim)
        except mod.SchemaUngueltig as exc:
            raise Abgelehnt("Schemadatei ungueltig: %s" % exc, "konfig")
        gelesen = tuple(k.gelesen)
        gemerkt = bool(gelesen) and gelesen == pfade and not k.ungueltig()   # nie eine kaputte Ebene merken
        KONFIG_CACHE[self._merkstelle] = {"pfade": gelesen, "schluessel": vorher if gemerkt else None,
                                          "mod": mod, "konfig": k if gemerkt else None}
        return k

    def schalter(self, schluessel):
        if schluessel not in self._schalter:
            mod, k = self.konfig()
            try:
                self._schalter[schluessel] = k.hole(schluessel)
            except mod.KonfigUngueltig as exc:
                self.vergiss()
                raise Abgelehnt("werkbank.json ungueltig fuer %s: %s" % (schluessel, exc), "konfig")
            except KeyError:
                self.vergiss()
                raise Abgelehnt("Schluessel %s fehlt im Schema" % schluessel, "konfig")
        return self._schalter[schluessel]

    def vergiss(self):
        """Ein Fehler wird nie gemerkt: die Merkstelle dieser Lage faellt aus KONFIG_CACHE."""
        KONFIG_CACHE.pop(self._merkstelle, None)

    def lan_endpunkte(self, host_ok):
        """Schalter datenklasse.lanEndpunkte (Betreiberebene, decision 2026-10-28): Host -> Klasse, wie der Betreiber einen
        Endpunkt im eigenen Netz einordnet. -> {host: Klasse} der brauchbaren Eintraege. host_ok(host) sagt, ob der
        Host die kanonische Form eines Hosts im LAN hat (das Tor kennt die Erkennung); ein Eintrag mit anderer Form
        (Muster, Gross-Schreibung, ein Host ausserhalb des eigenen Netzes) oder mit einer Klasse ausserhalb von ORDER
        wird ignoriert und gewarnt (fail closed: der Host bleibt streng). Die Warnung nennt den Host, nie den Wert.
        Ein ungueltiges werkbank.json lehnt ab (Abgelehnt, Kennwort konfig). Die Auswertung (heben nur ohne
        Systemrichtlinie, sonst nur senken) steht im Tor; hier steht nur das Lesen."""
        if self._lan is None:
            roh = self.schalter("datenklasse.lanEndpunkte")
            if not isinstance(roh, dict):
                self.vergiss()
                raise Abgelehnt("datenklasse.lanEndpunkte: unbekannter Wert (ein Objekt Host -> Klasse erwartet)", "konfig")
            gut = {}
            for host, klasse in roh.items():
                if not (isinstance(host, str) and host_ok(host)):
                    self.warnung("datenklasse.lanEndpunkte: Eintrag %.64r ignoriert: kein kleingeschriebener Host "
                                 "im eigenen Netz ohne Muster" % (host,))
                elif not (isinstance(klasse, str) and klasse in ORDER):
                    self.warnung("datenklasse.lanEndpunkte: Eintrag %s ignoriert: die Klasse ist keine von %s"
                                 % (host, ", ".join(ORDER)))
                else:
                    gut[host] = klasse
            self._lan = gut
        return self._lan

    def kunde_hebt_auf_customer(self):
        """Die EINE Stelle fuer die Regel 'eine kunde:-Zeile hebt auf mindestens CUSTOMER'
        (Beschluss D-2): Schluessel datenklasse.kundeHebtAufCustomer, Vorgabe true."""
        w = self.schalter("datenklasse.kundeHebtAufCustomer")
        if not isinstance(w, bool):
            self.vergiss()
            raise Abgelehnt("datenklasse.kundeHebtAufCustomer: unbekannter Wert %r" % (w,), "konfig")
        return w

    def vorgabe_klasse(self):
        """Die EINE Stelle, an der die Untergrenze gelesen wird: Schalter datenklasse.unmarkiert
        (Beschluss E-02). -> (Klasse, Schalterwert); bei kein-start ist die Klasse die Untergrenze
        fuer Projekte mit Marker."""
        w = self.schalter("datenklasse.unmarkiert")
        if w not in SCHALTER_WERTE:
            self.vergiss()
            raise Abgelehnt("datenklasse.unmarkiert: unbekannter Wert %r" % (w,), "konfig")
        return (UNTERGRENZE_KEIN_START if w == "kein-start" else w), w

    # -- Klasse --

    def projekt_eintrag(self, wege):
        """Eintrag der Liste `projekte` der Systemrichtlinie, der einen der Wege enthaelt; der
        laengste Pfad gewinnt. Die Produktvorgabe hat keine Liste, die Nutzerrichtlinie zaehlt nie."""
        echte = [os.path.realpath(w) for w in wege]
        treffer = [e for e in self.richtlinie()["basis"].get("projekte", [])
                   if any(w == e["echt"] or w.startswith(e["echt"].rstrip(os.sep) + os.sep)
                          for w in echte)]
        return max(treffer, key=lambda e: (len(e["echt"]), ORDER.index(e["klasse"])), default=None)

    def betretbar(self, d):
        """Der Projektordner SELBST muss sich betreten lassen (Suchrecht, auch per ACL nur --x): sonst waere ein
        Marker darin nicht feststellbar. isdir allein genuegt nicht, es braucht nur die Rechte der Eltern."""
        pfad_pruefen(os.path.join(d, os.curdir), "Projektordner", folgen=True)

    def hauptbaum_ort(self, ordner, mit_marker, gruende):
        """-> (gleicher Ort im Hauptbaum, belegt) oder None ohne Hauptbaum. belegt: siehe worktree_belegt."""
        try:
            eintrag = worktree_eintrag(ordner)
            if eintrag is None:
                return None
            haupt, relativ, wurzel, gitdir = eintrag
            belegt = worktree_belegt(haupt, wurzel, gitdir)
        except Abgelehnt:
            # REGEL R-KL-MEHRKONTO-OHNE-ORDNER: ohne Marker und ohne Liste projekte aendert der Hauptbaum nichts; mit
            # einem von beiden lehnt ein unpruefbares .git weiter ab (dieselbe Liste wie projekt_mit_vorgabe)
            if mit_marker or self.richtlinie()["basis"].get("projekte"):
                raise
            gruende.append("Git-Eintrag nicht pruefbar; ohne Marker und ohne Projektliste zaehlt der Hauptbaum nicht")
            return None
        gruende.append(("Git-Worktree: der Hauptbaum %s zaehlt mit" if belegt else
                        "Git-Worktree ohne Rueckverweis des Hauptbaums %s: er hebt nur, die Liste projekte senkt nichts")
                       % haupt)
        return os.path.normpath(os.path.join(haupt, relativ)), belegt

    def projekt(self, start, marker=True, ordner_da=None):
        """-> dict(klasse, kunden, quelle, wege, gruende) ueber alle Marker aller Wege (marker=False: keine).
        ordner_da: das weiss die Stelle aus dem Deskriptor (R-FD-ORT); sie fragt den Ordner dann nicht selbst."""
        mit_marker = marker
        roh = start if start and os.path.isabs(start) else os.path.join(os.getcwd(), start or "")
        d, lexikalisch = pfad_des_kernels(roh), os.path.abspath(roh)   # REGEL R-KL-PFAD
        if ordner_da is None:
            if os.path.isfile(roh):
                d, lexikalisch = os.path.dirname(d), os.path.dirname(lexikalisch)
            ordner_da = os.path.isdir(d) and os.path.exists(roh)   # ein fehlender Teil vor `..` ist fuer den Kernel kein Ordner
        if not ordner_da:
            raise Abgelehnt("kein Verzeichnis: %s" % d, "verzeichnis")
        # REGEL R-KL-MEHRKONTO-OHNE-ORDNER: ohne Marker gibt es im Ordner nichts zu uebersehen; die Stelle betritt ihn nicht
        if mit_marker:
            self.betretbar(d)
        wege = [d, os.path.realpath(d)]
        gruende = []
        ort = self.hauptbaum_ort(os.path.realpath(d), mit_marker, gruende)
        if ort and ort[1]:
            wege.append(ort[0])
        senken = list(wege)   # REGEL R-KL-WORKTREE: nur diese Wege duerfen die Untergrenze senken
        if ort and not ort[1]:
            wege.append(ort[0])
        if lexikalisch not in wege:   # REGEL R-KL-PFAD: ein logisches `cd` landet dort; dieser Weg hebt nur
            wege.append(lexikalisch)
            gruende.append("Pfad mit ..: die lexikalische Lesart %s zaehlt mit und hebt nur" % lexikalisch)
            ort = self.hauptbaum_ort(os.path.realpath(lexikalisch), mit_marker, gruende)
            if ort and ort[0] not in wege:
                wege.append(ort[0])
        marker = []
        # REGEL R-KL-MEHRKONTO: ohne Marker zaehlt kein .datenklasse auf dem Weg; sonst bleibt alles gleich
        for w in (wege if mit_marker else []):
            for p in marker_auf_dem_weg(w):
                if p not in marker:
                    marker.append(p)
        klasse, quelle, kunden = None, None, set()
        hebt = None
        for p in marker:
            k, ks = marker_lesen(p)
            kunden.update(ks)
            q = p
            if ks and hebt is None:
                hebt = self.kunde_hebt_auf_customer()
            if ks and hebt and ORDER.index(k) < ORDER.index("CUSTOMER"):
                gruende.append("Marker %s: %s, nennt Kunden und hebt auf CUSTOMER" % (p, k))
                k, q = "CUSTOMER", p + ":kunde"
            else:
                gruende.append("Marker %s: %s%s" % (p, k, ", nennt Kunden" if ks else ""))
            if klasse is None or ORDER.index(k) > ORDER.index(klasse):
                klasse, quelle = k, q
        if not mit_marker:
            gruende.append("Marker zaehlen nicht (Mehrkontenbetrieb): die Klasse kommt aus dem Register der Stelle")
        elif not marker:
            gruende.append("kein Marker auf dem Weg zur Wurzel")
        return {"klasse": klasse, "quelle": quelle, "kunden": sorted(kunden), "wege": wege, "senken": senken,
                "gruende": gruende}

    def projekt_mit_vorgabe(self, start, marker=True, ordner_da=None):
        """Klasse = Maximum aus Markern und Untergrenze. Die Untergrenze ist der Schalter oder,
        fuer ein Projekt der Systemliste, dessen Klasse."""
        self.richtlinie()
        info = self.projekt(start, marker, ordner_da)
        e = self.projekt_eintrag(info["senken"])
        if e:
            grenze, q = e["klasse"], "projekte:" + e["pfad"]
            info["gruende"].append("Untergrenze %s aus der Liste projekte der Systemrichtlinie (%s)"
                                   % (grenze, e["pfad"]))
        else:
            grenze, wert = self.vorgabe_klasse()
            if wert == "kein-start" and info["klasse"] is None:
                raise KeinStart()
            q = "vorgabe:" + grenze
            info["gruende"].append("Untergrenze %s aus dem Schalter datenklasse.unmarkiert (%s)"
                                   % (grenze, wert))
        hebt = self.projekt_eintrag(info["wege"])   # REGEL R-KL-WORKTREE: ein unbelegter Weg hebt, senkt aber nie
        if hebt and ORDER.index(hebt["klasse"]) > ORDER.index(grenze):
            grenze, q = hebt["klasse"], "projekte:" + hebt["pfad"]
            info["gruende"].append("Untergrenze %s aus der Liste projekte, nur als Hebung (%s)" % (grenze, hebt["pfad"]))
        if info["klasse"] is None or ORDER.index(grenze) > ORDER.index(info["klasse"]):
            info["klasse"], info["quelle"] = grenze, q
        return info

    def klasse_aus_aufruf(self, wert, kunden):
        """pruefe --klasse K [--kunde ID ...]: Klasse und Kunden nennt der Aufrufer. Sie zaehlen wie
        ein Marker: --kunde hebt wie eine kunde:-Zeile, der Schalter ist Untergrenze."""
        klasse, quelle = klasse_pruefen(wert, "--klasse", kennwort="marker"), "--klasse"
        for kid in kunden:
            if not KUNDE_ID.match(kid):
                raise Abgelehnt("ungueltige Kunden-ID '%s' in --kunde" % kid, "marker")
        if (kunden and ORDER.index(klasse) < ORDER.index("CUSTOMER")
                and self.kunde_hebt_auf_customer()):
            klasse, quelle = "CUSTOMER", "--kunde"
        grenze, _ = self.vorgabe_klasse()
        if ORDER.index(grenze) > ORDER.index(klasse):
            klasse, quelle = grenze, "vorgabe:" + grenze
        return {"klasse": klasse, "quelle": quelle, "kunden": sorted(set(kunden)),
                "bezug": "die Daten sind"}


def zeile(erg):
    """Die Vertragszeile KLASSE<TAB>KUNDEN<TAB>QUELLE eines Ergebnisses von klasse_fuer."""
    return "%s\t%s\t%s" % (erg["klasse"], ",".join(erg["kunden"]) or "-", erg["quelle"])


def klasse_fuer(ordner, heim=None, lage=None, marker=True, nutzer=None, ordner_da=None):
    """Klasse des Ordners nach der Regel des Tors. Wirft nie.

    -> {"klasse", "kunden", "quelle", "gruende", "warnungen"} oder {"fehler", "kennwort"}.
    lage: eine vorbereitete Lage (das Tor reicht damit seine Testnaht herein und liest bei
    `klassen` die Richtlinie einmal fuer viele Ordner); ohne sie liest dieser Aufruf frisch.
    marker=False: kein Marker zaehlt (Regel R-KL-MEHRKONTO, nur fuer die Stelle im Mehrkontenbetrieb).
    nutzer: eine NutzerEbene statt der Dateien unter der Heimat. ordner_da: siehe Lage.projekt."""
    try:
        if lage is None:
            lage = Lage(heim=heim, nutzer=nutzer)
        info = lage.projekt_mit_vorgabe(ordner, marker, ordner_da)
        return {"klasse": info["klasse"], "kunden": list(info["kunden"]),
                "quelle": info["quelle"] or "vorgabe", "gruende": info["gruende"],
                "warnungen": list(lage.warnungen)}
    except Abgelehnt as exc:
        return {"fehler": str(exc), "kennwort": exc.kennwort}
    except (Exception, SystemExit) as exc:   # fail closed: auch ein Programmfehler oder SystemExit aus dem Leser ist ein Fehler, nie eine Klasse
        return {"fehler": "interner Fehler (%s: %s)" % (type(exc).__name__, exc), "kennwort": "intern"}
