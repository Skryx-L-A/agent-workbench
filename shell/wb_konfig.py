"""wb_konfig -- die eine Quelle fuer Einstellungen der Werkbank (Durchstich task, Stufe 1e, 22.10.2026).

ANLASS. Heute stehen Pfade, Routen und Hosts fest im Code oder in Umgebungsvariablen, die
tmux-Server, MCP-Prozesse und warme Dienste erst nach einer neuen Anmeldung sehen. Eine Datei
liest jeder Prozess beim Aufruf. Entwurf: bericht-workbench-erweiterung.md 1.4, 4 und 8 (U2, U6).

SCHEMA. Typ, Vorgabe, Klasse und Richtung jedes bekannten Schluessels stehen in EINER Datei,
einstellungen-schema.json neben diesem Modul (SCHEMA_DATEI). Eine ungueltige oder fehlende
Schemadatei ist ein Fehler (SchemaUngueltig), nie ein stilles Weitermachen. Unbekannte Schluessel
in den Konfigurationsdateien bleiben erlaubt (Vorwaertsvertraeglichkeit).

SCHEMA-FRAGMENTE (task, 27.10.2026). Ein anderes Paket (etwa der Vault) liefert seine Zeilen als eigene Datei
im Ordner schema.d/ neben der Schemadatei (FRAGMENT_ORDNER; installiert ~/.local/bin/schema.d/<name>.json), im
selben Format plus optional "herkunft". Gelesen wird in fester Reihenfolge nach Dateiname. Regeln:
  - Der Dateiname ist das Praefix: vault.json darf nur Schluessel unter "vault." anlegen. Ein Praefix, das schon
    erstes Segment eines Schluessels des Hauptschemas ist, gehoert dem Hauptschema. So bleibt jede Klassenbildung
    je Praefix (Schema.betreiber) des Hauptschemas unberuehrt, und kein Fragment kann dort die Umgebung oeffnen.
  - Ein Schluessel, der schon im Hauptschema steht, ist ein Fehler; er ueberschreibt nichts.
  - Ein kaputtes Fragment faellt als Ganzes aus und steht in Schema.meldungen (Konfig.meldungen); das Hauptschema
    und die anderen Fragmente laufen weiter. Ein fehlender Ordner heisst: keine Fragmente.

EBENEN, die erste gefundene gewinnt je Schluessel (Objekte werden je Unterschluessel gemischt):
  Schluessel der Klasse "betreiber":  System > Betreiber > Nutzer > Vorgabe
  alle anderen Schluessel:            Umgebung > Nutzer > Betreiber > System > Vorgabe
  - Umgebung: ENV_ALIASE (WB_MODELS_FILE, WB_WERKZEUGE_SRC), danach die Datei WB_KONFIG. Sie wirkt NIE
              auf Betreiberschluessel: ein Aufrufer, der die Umgebung setzt, darf keine Richtlinie lenken.
  - Nutzer:   ${XDG_CONFIG_HOME:-~/.config}/agent-workbench/werkbank.json fuer Nutzerschluessel. Betreiberschluessel
              lesen IMMER die Datei in der Heimat laut Benutzerdatenbank (pw_dir der Kennung, <heim>/.config/...),
              nie unter XDG_CONFIG_HOME oder HOME aus der Umgebung: ein umgesetztes XDG_CONFIG_HOME lenkt sie nicht um.
              lade(heim=<Ordner>) nennt die Heimat der Nutzerebene ausdruecklich (task, 23.10.2026): dann
              lesen Nutzer- UND Betreiberschluessel <heim>/.config/agent-workbench/werkbank.json, nie HOME,
              XDG_CONFIG_HOME oder die Heimat der Prozesskennung. So liest eine Stelle unter einem Dienstkonto die
              Datei der Person, fuer die sie entscheidet. Ist sie unlesbar, ist die Ebene ungueltig (fail closed).
  - Betreiber: BETREIBER_PFAD, die Datei der Entscheidungsstelle (betreiber.json unter dem Heim der
              Stellen-Kennung; seit 2b+ der Ordner aus wb_identitaet.stelle_ordner(), siehe _stelle). Sie gilt nur, wenn sie der Stellen-Kennung gehoert und nicht gruppen-
              oder weltschreibbar ist; fehlt sie, ist die Ebene leer. Solange die Stelle unter derselben
              Kennung wie der Nutzer laeuft, ist das eine Richtlinie, keine Grenze.
  - System:   SYSTEM_PFADE je Plattform (Linux /etc/agent-workbench/werkbank.json, macOS /Library/
              Application Support/agent-workbench/werkbank.json). Datei UND die ganze Ordnerkette bis / gehoeren
              root (SYSTEM_EIGENTUEMER) und sind nicht gruppen- oder weltschreibbar, sonst ungueltig.
  - Vorgabe:  die Zeile `vorgabe` im Schema und das, was der Leser als `vorgabe=` mitgibt.
  Tests pruefen die Ebenen NIE ueber eine Umgebungsvariable und nie ueber die echten Dateien, sondern
  ueber eine Kopie dieses Moduls, in der die Konstanten SYSTEM_PFADE, SYSTEM_EIGENTUEMER, SYSTEM_KETTE_BIS,
  BETRIEBSART, BETREIBER_PFAD und NUTZER_HEIMAT (je eine Zeile) umgeschrieben sind. Die Stelle selbst benennt nur
  stelle.konto in der (Test-)Systemdatei, keine Konstante.

SPERREN. Die Systemdatei darf "gesperrt": ["geheimnisFilter", "geschuetzteDienste"] enthalten.
Ein gesperrter Schluessel (und alles darunter) kommt nur aus System oder Vorgabe; Werte aus
Umgebung, Nutzer- und Betreiberdatei werden gemeldet und ignoriert. "gesperrt" in einer anderen Ebene
als System wirkt nicht und wird gemeldet.

NUR VERSCHAERFEN. Ein Betreiberschluessel mit einer Richtung (wahl: streng-zuerst, zahl: groesser-
oder kleiner-lockert, bool: true- oder false-lockert) kennt eine Ordnung von locker nach streng. Gibt es
eine gueltige System- oder Betreiberdatei, nimmt der Leser aus jeder tieferen Ebene nur einen Wert,
der nicht lockerer ist als die Untergrenze (oberste Betreiber-Ebene mit Wert, sonst die Vorgabe);
einen lockereren Wert meldet er und ignoriert ihn. Bei den Mengentypen mehrfachwahl und liste mit streng-zuerst
verschaerft ein tieferer Wert nur, wenn er eine Teilmenge des oberen ist (Eintraege wegnehmen); die gleiche Menge in
anderer Reihenfolge wird still verworfen. Ohne beide Dateien waehlt die Nutzerebene frei.

OHNE NUTZEREBENE. Ein Betreiberschluessel mit ohneNutzerebene true (pfade.registry, Stufe 1a Nachtrag 2) lernt
nichts aus der Nutzerdatei: ein Wert dort wird gemeldet und ignoriert, und eine kaputte Nutzerdatei bricht das Lesen
dieses Schluessels nicht ab. System und Betreiber (und die Vorgabe) bestimmen ihn; die Umgebung wirkt nie auf einen
Betreiberschluessel. Das Praefix (hier `pfade`) darf neben Betreiberschluesseln Nutzerschluessel tragen: die Regel
"nie aus der Umgebung" gilt je Schluessel, sobald das Praefix gemischt ist.
Seit w42 (28.10.2026, Anlass agents.routenpruefung: Abschalten nur durch einen Menschen ueber Betreiber oder System)
darf ohneNutzerebene true neben nurSystemebene "mandant" stehen, nicht neben nurSystemebene true (dort wirkt die
Nutzerebene ohnehin nie). Es gelten beide Regeln: der Nutzerwert wird gemeldet und ignoriert, eine kaputte Nutzerdatei
bricht den Schluessel nicht; im Einzelplatz bestimmen System, Betreiber und Vorgabe (die Betreiberebene darf lockern), im
Mandantenmodus kommt ein lockererer Wert als die Vorgabe nur aus der Systemdatei.

SYSTEM ALLEIN IM MANDANTEN. Ein Schluessel mit nurSystemebene "mandant", einer Richtung und systemAlleinImMandant true (vault.betrieb,
decision 2026-11-04, Fall G) liest im Mandantenmodus neben der Vorgabe nur die Systemdatei: Betreiber- und Nutzerwerte werden gemeldet und
ignoriert, auch wenn sie nach der Ordnung nur verschaerfen wuerden (die Person darf nicht von angebunden zurueck zum lokalen Brain).
Eine fehlende Systemdatei ergibt die Vorgabe, eine ungueltige den strengsten Wert (SYSTEMEBENE UNGUELTIG). Im Einzelplatz aendert das
Feld nichts: dort gelten die Ebenen wie bei jedem Betreiberschluessel mit Richtung (Nutzer und Betreiber duerfen verschaerfen).

NUR SYSTEMEBENE JE WERT. nurSystemebeneWerte (decision 2026-11-05, Anlass runner.werkzeugstufe ohne-sandbox aus w75) bindet einzelne
Werte eines Betreiberschluessels vom typ wahl mit Richtung an die Systemdatei, in beiden Betriebsarten. Jeder Eintrag steht in wahl
und ist nach der Ordnung lockerer als jede Vorgabe (auch vorgabeJeProfil); einen strengen Wert zu binden, schuetzte nichts. Kommt ein
solcher Wert aus Betreiber-, Nutzer- oder Umgebungsebene, wird er mit Schluessel, Wert und Ebene gemeldet und ignoriert: der Schluessel
liest weiter, als haette diese Ebene keinen Wert (Konfig.nur_system_roh behaelt ihn). Aus der Systemdatei gilt er; eine kaputte
Systemdatei ergibt weiter den strengsten Wert. Agenten unter derselben Kennung schreiben Nutzer- und oft auch Betreiberdatei, nur die
root-eigene Systemdatei ist fuer sie zu, darum zaehlt auch die Betreiberebene am Einzelplatz nicht. setze() lehnt einen solchen Wert
ab und nennt den Weg ueber die Systemdatei. Neben systemAlleinImMandant ist das Feld ungueltig (05.11.2026).

NUR SYSTEMEBENE. Ein Schluessel mit nurSystemebene true bekommt einen lockereren Wert als die Vorgabe nur aus
der Systemdatei. Betreiber- und Nutzerebene duerfen dort nur verschaerfen, auch ohne Systemdatei (die Vorgabe ist dann
die Untergrenze); eine fehlende oder kaputte Systemdatei bricht das Lesen dieses Schluessels nicht ab, es gilt die
Vorgabe. Ein Schluessel vom typ text darf nurSystemebene ohne Richtung (richtung "keine") tragen: sein Wert kommt
NUR aus der Systemdatei oder der Vorgabe. Werte aus Betreiber- und Nutzerdatei werden gemeldet und ignoriert, und keine
kaputte Datei bricht das Lesen ab (es gilt die Vorgabe). Ebenso darf ein Schluessel vom typ zahl nurSystemebene ohne
Richtung tragen (decision 2026-11-07, Anlass vault.quellen.intervall: fuer einen Takt ist kein Wert eindeutig strenger, laenger verzoegert einen
Entzug, kuerzer belastet den Server): wie bei typ text kommt der Wert NUR aus der Systemdatei oder der Vorgabe, Werte aus Betreiber-, Nutzer- und
Umgebungsebene werden gemeldet und ignoriert (Konfig.nur_system_roh), eine fehlende oder kaputte Systemdatei bricht das Lesen nicht ab, es gilt die
Vorgabe. Die Vorgabe ist bei typ zahl PFLICHT (anders als bei text). Ein Systemwert ausserhalb von `bereich` oder vom falschen Typ gilt nicht: es
gilt die Vorgabe, mit Meldung. Die Zeilen mit Richtung und nurSystemebene true bleiben, wie sie sind (groesser-lockert, kleiner-lockert: Betreiber und
Nutzer duerfen dort verschaerfen). Ebenso darf ein Schluessel vom typ liste nurSystemebene ohne
Richtung tragen (teilbaeume.suchwurzeln, decision 2026-10-24): sein Wert kommt nur aus der Systemdatei oder der Vorgabe. Anders
als bei typ text ist die Vorgabe dort KEIN Ersatz fuer eine ungueltige Systemdatei, denn eine Liste kann mehr zulassen als
die der Firma (Suchwurzeln: die Vorgabe ist weiter als eine verengte Firmenliste); der Schluessel bricht stattdessen ab
(KonfigUngueltig). Die Werte, die dabei aus Nutzer-, Betreiber- und Umgebungsebene verworfen werden, behaelt
Konfig.nur_system_roh (Schluessel -> [(Ebene, Rohwert)]), damit ein Aufrufer die Warnung dazu selbst formuliert
(wb_suchwurzeln.py) und nicht erst nach dem stillen Verwerfen nachfragt. nurSystemebene "mandant" wirkt erst im Mandantenmodus
(betriebsart(), siehe BETRIEBSART) wie true; im Einzelplatz gilt die Betreiberebene wie bei jedem
Betreiberschluessel. Mit ohneNutzerebene true zusammen (w42, 28.10.2026, agents.routenpruefung) wird zusaetzlich der
Nutzerwert verworfen, siehe OHNE NUTZEREBENE. Bei nurSystemebene true oder "mandant" darf die Vorgabe null sein, wenn der Schluessel typ text hat, oder typ zahl oder wahl ohne Richtung (zahl und wahl seit
11.11.2026, Pflichtangabe der Firma; eine Wahl ohne Richtung ist nur ohne Vorgabe und nur mit true gueltig)
(decision 2026-10-27; "mandant" seit E3 Zusatz 3; eine Liste ohne Richtung braucht weiter eine Vorgabe): der Wert
gilt dann als "nicht gesetzt", solange die Systemdatei (bei "mandant" im Einzelplatz auch die Betreiberebene) keinen nennt. `get` endet mit
Exit 4, `zeige --json` liefert wert und quelle null, und es gibt keinen Ersatzwert; Betreiber- und Nutzerwerte wirken
nicht. In allen anderen Faellen bleibt eine Schemazeile ohne Vorgabe ungueltig. Die Systemdatei prueft die ganze Ordnerkette bis SYSTEM_KETTE_BIS ('/'): jeder Ordner gehoert root und ist nicht gruppen- oder weltschreibbar.

BETRIEBSART. betriebsart() ergibt 'mandant', wenn die Zeile BETRIEBSART oder der Systemschluessel betriebsart es sagt oder die
Marke MARKE_NAME neben der Systemdatei liegt. Die Systemdatei kann nur verschaerfen: Ist sie ungueltig, gilt 'mandant', nie
'einzelplatz'. Die Marke haelt die Firmenregeln, wenn die Systemdatei fehlt. Ein fester Pfad, keine Datei neben dem Modul: Das gilt
auch fuer die Kopien in ~/.local/bin. Der Ort leitet sich aus SYSTEM_PFADE ab. Eine Marke mit falschem Eigentuemer zaehlt wie eine
ungueltige Systemdatei, also auch als 'mandant'; deshalb genuegt hier ihr Dasein, wb-arbeitsplatz pruefe prueft Eigentuemer und Kette.

UNGUELTIGE DATEIEN. Eine Ebene ist ungueltig bei Lesefehler, JSON-Fehler, doppeltem Schluessel,
falschem Schema, falschem Typ oder Wert eines bekannten Schluessels oder bei falschen Rechten oder
Eigentuemern (System, Betreiber). Fuer Betreiberschluessel ist das ein harter Abbruch
(KonfigUngueltig): sonst fielen die Sperren der Systemdatei still weg. Fuer alle anderen Schluessel
wird die Ebene mit Meldung uebersprungen. Die Datei WB_KONFIG zaehlt dabei nicht (sie wirkt auf
Betreiberschluessel nicht).

SYSTEMEBENE UNGUELTIG (Variante C, decision 2026-10-23). Ist die Systemdatei ungueltig, gilt je Betreiberschluessel
(Konfig.system_ersatz, laute Meldung in Konfig.meldungen und Konfig.system_ungueltig):
  - mit Richtung: der STRENGSTE Wert (strengster()); er laesst sich durch keine Ebene lockern oder verschaerfen.
    Traegt die Zeile "aussperrend": true (ihr strengster Wert laesst keinen Weg offen, etwa eine leere Menge
    von Anmeldewegen), bricht der Schluessel stattdessen ab (KonfigUngueltig): laut statt still ausgesperrt;
  - nurSystemebene ohne Richtung (typ text und zahl): die Vorgabe; (typ liste): Abbruch wie bei den uebrigen Betreiberschluesseln;
  - uebrige Betreiberschluessel: Abbruch wie oben.
Nutzerschluessel ueberspringen die Ebene wie bisher; ihre Sperren sind dann unbekannt und fallen.
Harte Fehler sind Lese-, JSON- und Schemafehler, Eigentuemer, Rechte, Ordnerkette und eine kaputte Zeile
"gesperrt". Ein Typ- oder Wertfehler in anderen Zeilen verwirft NUR diese Zeilen (Schicht.verworfen; Einheit ist
die Schemazeile, eine Karte zaehlt als eine Zeile, ein Zwischenobjekt, das kein Objekt ist, als Fehler aller
Zeilen darunter). Fuer eine verworfene Betreiberzeile gilt dieselbe Regel je Schluessel; der Rest der Systemdatei
bleibt in Kraft, auch die Sperren: eine gesperrte verworfene Zeile faellt auf die Vorgabe, nie auf einen
Nutzerwert. Das gilt nur fuer die Systemdatei; in den anderen Ebenen macht jeder Typfehler die Ebene ungueltig.

KARTEN. Unterschluessel einer Karte (geheimnisFilter.aufbrechen: Host -> Route) koennen Punkte
enthalten. Ab einer Karte ist der Rest des Schluessels ein einziger Name; Sperren gelten fuer die
ganze Karte.

KARTEN MIT EINTRAG UND RICHTUNG (task, 28.10.2026). Eine Karte, deren Werte Objekte sind (tickets.ziele),
beschreibt die Schemazeile im Feld `eintrag`: Feldname -> {typ text|wahl|liste, wahl, mengen, muster, pflicht, fehlt,
richtung fest|streng-zuerst}. Namen der Eintraege sind [A-Za-z0-9][A-Za-z0-9_-]* (sie werden Teil von Dateipfaden). Ein
unbekanntes Feld, ein fehlendes Pflichtfeld oder ein Wert ohne die vorgeschriebene Form macht den Wert ungueltig; die
Fehlermeldung nennt nie den Wert (ein Zugangswert, der statt eines Verweises dort steht, darf in keiner Meldung landen).
Mit richtung streng-zuerst ist die Karte atomar (die tiefste Ebene mit Wert gewinnt ganz, kein Mischen je Eintrag) und
kennt die Teilmengenregel: ein tieferer Wert (Nutzer, Betreiber unter der Systemebene) ist nicht lockerer, wenn er nur
Eintraege WEGNIMMT oder Felder verschaerft. Jede Lockerung (neuer Eintrag, geaendertes festes Feld, hoehere Wahl, mehr
Elemente, mehr Richtungen) verwirft den ganzen Wert laut mit den Gruenden (karte_lockerung). Strengster Wert: {}.

SCHREIBEN. setze() am Ende dieses Moduls ist der einzige Schreibweg (Nutzerebene, Version, Sperre, atomar).

Nur Python-Standardbibliothek, laeuft ab Python 3.9. Die Bibliothek schreibt nichts nach stderr:
`Konfig.meldungen` sammelt, der Aufrufer gibt aus.
"""
import json
import os
import re
import stat
import struct
import sys

try:
    import pwd
    import grp
except ImportError:      # Windows: keine Stelle, keine Betreiberebene
    pwd = None
    grp = None

SCHEMA = 'wb-konfig/1'
DATEINAME = 'werkbank.json'
MAX_BYTES = 1 << 20

# Schemadatei neben diesem Modul. Eine Zeile je Konstante: Tests schreiben sie in einer Kopie um.
SCHEMA_DATEI = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'einstellungen-schema.json')
FRAGMENT_ORDNER = 'schema.d'     # Ordner der Schema-Fragmente, neben der jeweils gelesenen Schemadatei
SYSTEM_PFADE = {'linux': '/etc/agent-workbench/werkbank.json', 'darwin': '/Library/Application Support/agent-workbench/werkbank.json'}
SYSTEM_EIGENTUEMER = (0,)
SYSTEM_KETTE_BIS = '/'
BETRIEBSART = 'einzelplatz'
MARKE_NAME = 'firmenrechner'
BETREIBER_PFAD = '{heim}/.local/share/agent-workbench/stelle/betreiber.json'
NUTZER_HEIMAT = None     # None: Heimat laut Benutzerdatenbank der Kennung (nie HOME); Tests schreiben einen Ordner hinein.

# Werte, die ein Prozess ueber eine Umgebungsvariable schon heute kennt: bleiben Ebene 1.
ENV_ALIASE = {
    'pfade.registry': 'WB_MODELS_FILE',
    'pfade.werkzeugQuelle': 'WB_WERKZEUGE_SRC',
}

_FEHLT = object()

# Umgebungsvariablen, die nur unter der Testnaht WB_TESTUMGEBUNG=1 gelten (task, 23.10.2026): die Regel steht
# in shell/wb_naht_ziel.py neben dieser Datei (naht_variable). Ohne Naht ignoriert sie den Wert und haengt die
# Meldung an `meldungen`; fehlt das Modul, gilt der Wert ebenfalls nicht (fail closed).
NAHT_VARIABLEN = ('WB_MODELS_FILE', 'AWB_MODELS_FILE', 'AWB_SETTINGS_FILE', 'AWB_GUARD_GRANTS_DIR', 'AWB_ROLLEN_DIR',
                  'AWB_WB_IDENTITAET', 'AWB_REMOTE_MACHINES', 'AWB_CONFIG', 'AWB_STATE_DIR', 'AWB_WB_STATE',
                  'AWB_WB_REVIVE', 'AWB_WELTEN_FERN', 'WB_REPLIK_SSH', 'AWB_GUARD_LOG', 'AWB_GUARD_BLOCKS_DIR')


class _Sammler:
    def __init__(self):
        self.zeilen = []

    def write(self, text):
        self.zeilen.append(text.strip().split(': ', 1)[-1])   # ohne das Werkzeugpraefix: der Aufrufer setzt seins


def _naht_wert(var, env, meldungen):
    wert = env.get(var)
    if not wert:
        return None
    try:
        import importlib.util
        spec = importlib.util.spec_from_file_location(
            'wb_naht_ziel', os.path.join(os.path.dirname(os.path.realpath(__file__)), 'wb_naht_ziel.py'))
        modul = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(modul)
    except Exception:
        meldungen.append('%s ist gesetzt und wird ignoriert (wb_naht_ziel.py fehlt neben wb_konfig.py)' % var)
        return None
    sammler = _Sammler()
    ergebnis = modul.naht_variable(var, umgebung=env, werkzeug='wb_konfig', fehler=sammler)
    meldungen.extend(sammler.zeilen)
    return ergebnis

SCHEMA_TYPEN = ('wahl', 'mehrfachwahl', 'bool', 'zahl', 'text', 'liste', 'karte')
SCHEMA_KLASSEN = ('betreiber', 'nutzer', 'geraet', 'intern')
SCHEMA_RICHTUNGEN = ('streng-zuerst', 'groesser-lockert', 'kleiner-lockert', 'true-lockert', 'false-lockert', 'keine')
SCHEMA_ROLLEN = ('nutzer', 'admin', 'sicherheitsadmin', 'mandanteneigentuemer')
SCHEMA_FELDER = ('schluessel', 'klasse', 'typ', 'wahl', 'bereich', 'richtung', 'vorgabe', 'vorgabeJeProfil',
                 'empfehlung', 'risiko', 'rolle', 'grenzeVon', 'nurMensch', 'nurSystemebene', 'ohneNutzerebene', 'systemAlleinImMandant', 'nurSystemebeneWerte', 'umgebungNurNaht', 'quelle',
                 'wirksamAb', 'seite', 'titel', 'beschreibung', 'aussperrend', 'eintrag')
RICHTUNG_TYP = {'streng-zuerst': ('wahl', 'mehrfachwahl', 'liste', 'karte'), 'groesser-lockert': ('zahl',), 'kleiner-lockert': ('zahl',),
                'true-lockert': ('bool',), 'false-lockert': ('bool',)}
# Typen, deren Wert eine Menge ist: streng-zuerst heisst dort Eintraege wegnehmen (Teilmengenregel).
MENGEN_TYPEN = ('mehrfachwahl', 'liste')
# Felder, die jede Schemazeile tragen muss (seite nur bei Bedarf der Konsole).
SCHEMA_PFLICHT = ('klasse', 'typ', 'vorgabe', 'rolle', 'quelle', 'wirksamAb', 'titel', 'beschreibung')
_SCHLUESSEL_MUSTER = re.compile(r'[A-Za-z0-9_-]+(\.[A-Za-z0-9_-]+)*\Z')
# Karten mit `eintrag` (siehe KARTEN MIT EINTRAG UND RICHTUNG): Aufbau der Beschreibung und Name eines Eintrags.
EINTRAG_SPEC_FELDER = ('typ', 'wahl', 'mengen', 'muster', 'pflicht', 'fehlt', 'richtung')
EINTRAG_TYPEN = ('text', 'wahl', 'liste')
EINTRAG_RICHTUNGEN = ('fest', 'streng-zuerst')
_EINTRAG_FELDNAME = re.compile(r'[A-Za-z][A-Za-z0-9]*\Z')
_EINTRAG_NAME = re.compile(r'[A-Za-z0-9][A-Za-z0-9_-]*\Z')


class KonfigUngueltig(Exception):
    """Eine Ebene ist ungueltig und der Schluessel ist ein Betreiberschluessel."""


class SchemaUngueltig(Exception):
    """Die Schemadatei fehlt oder taugt nicht."""


def _nichtleer(x):
    """Zeichenkette mit mindestens einem Zeichen ausser Leerraum (wie die Konsole: ein Eintrag nur aus Leerzeichen ist leer)."""
    return isinstance(x, str) and bool(x.strip())


def _zahl(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool)


def wert_fehler(zeile, wert):
    """Text, wenn `wert` zum Typ der Schemazeile nicht passt, sonst None."""
    typ, name = zeile['typ'], zeile['schluessel']
    if typ == 'text':
        return None if _nichtleer(wert) else "'%s' muss eine nichtleere Zeichenkette sein" % name
    if typ == 'bool':
        return None if isinstance(wert, bool) else "'%s' muss true oder false sein" % name
    if typ == 'zahl':
        b = zeile.get('bereich') or {}
        ganz = isinstance(b.get('min'), int) and isinstance(b.get('max'), int)
        if not _zahl(wert) or (ganz and not isinstance(wert, int)):
            return "'%s' muss %s sein" % (name, 'eine ganze Zahl' if ganz else 'eine Zahl')
        if 'min' in b and wert < b['min'] or 'max' in b and wert > b['max']:
            return "'%s' muss zwischen %s und %s liegen" % (name, b.get('min'), b.get('max'))
        return None
    if typ == 'wahl':
        return None if isinstance(wert, str) and wert in zeile['wahl'] else \
            "'%s' muss einer von %s sein" % (name, ', '.join(zeile['wahl']))
    if typ == 'mehrfachwahl':
        if isinstance(wert, list) and len(set(map(str, wert))) == len(wert) \
                and all(isinstance(x, str) and x in zeile['wahl'] for x in wert):
            return None
        return "'%s' muss eine Liste verschiedener Werte aus %s sein" % (name, ', '.join(zeile['wahl']))
    if typ == 'liste':
        if isinstance(wert, list) and all(_nichtleer(x) for x in wert):
            return None
        return "'%s' muss eine Liste nichtleerer Zeichenketten sein" % name
    if typ == 'karte':
        if 'eintrag' in zeile:
            return _karte_eintraege_fehler(zeile, wert)
        if isinstance(wert, dict) and all(_nichtleer(k) and _nichtleer(v) for k, v in wert.items()):
            return None
        return "'%s' muss ein Objekt Name -> Zeichenkette sein" % name
    return "'%s' hat den unbekannten Typ '%s'" % (name, typ)


def _feld_fehler(spez, wert):
    """Text (ohne den Wert), wenn `wert` zur Beschreibung eines Eintragsfelds nicht passt, sonst None."""
    typ = spez['typ']
    if typ == 'text':
        if not _nichtleer(wert):
            return 'muss eine nichtleere Zeichenkette sein'
        if 'muster' in spez and not re.match('(?:' + spez['muster'] + r')\Z', wert):
            return 'hat nicht die vorgeschriebene Form'
        return None
    if typ == 'wahl':
        return None if isinstance(wert, str) and wert in spez['wahl'] else 'muss einer von %s sein' % ', '.join(spez['wahl'])
    if not (isinstance(wert, list) and all(_nichtleer(x) for x in wert) and len(set(wert)) == len(wert)):
        return 'muss eine Liste verschiedener nichtleerer Zeichenketten sein'
    if 'wahl' in spez and not all(x in spez['wahl'] for x in wert):
        return 'darf nur Werte aus %s enthalten' % ', '.join(spez['wahl'])
    return None


def _karte_eintraege_fehler(zeile, wert):
    """Erster Fehler einer Karte mit `eintrag` oder None. Nennt Eintrag und Feld, nie einen Wert."""
    name, spez = zeile['schluessel'], zeile['eintrag']
    if not isinstance(wert, dict):
        return "'%s' muss ein Objekt Name -> Objekt sein" % name
    for n, e in wert.items():
        if not (isinstance(n, str) and _EINTRAG_NAME.match(n)):
            return "'%s': ein Name ist nicht aus Buchstaben, Ziffern, _ und - gebildet" % name
        if not isinstance(e, dict):
            return "'%s.%s' muss ein Objekt sein" % (name, n)
        for f in e:
            if f not in spez:
                return "'%s.%s': unbekanntes Feld '%s'" % (name, n, f if isinstance(f, str) and _EINTRAG_FELDNAME.match(f) else '?')
        for f, sp in spez.items():
            if f not in e:
                if sp.get('pflicht'):
                    return "'%s.%s': Pflichtfeld '%s' fehlt" % (name, n, f)
                continue
            fehler = _feld_fehler(sp, e[f])
            if fehler:
                return "'%s.%s': Feld '%s' %s" % (name, n, f, fehler)
    return None


def _eintrag_spez_fehler(name, spez):
    """Fehler der Beschreibung `eintrag` einer Schemazeile oder None."""
    if not (isinstance(spez, dict) and spez):
        return '%s: eintrag muss ein nichtleeres Objekt Feldname -> Beschreibung sein' % name
    for f, sp in spez.items():
        wo = '%s: eintrag.%s' % (name, f)
        if not (isinstance(f, str) and _EINTRAG_FELDNAME.match(f)) or not isinstance(sp, dict):
            return '%s: Feldname oder Beschreibung ungueltig' % wo
        unbekannt = [k for k in sp if k not in EINTRAG_SPEC_FELDER]
        if unbekannt:
            return "%s: unbekanntes Feld '%s'" % (wo, unbekannt[0])
        typ, wahl, richtung = sp.get('typ'), sp.get('wahl'), sp.get('richtung')
        if typ not in EINTRAG_TYPEN:
            return "%s: typ '%s' unbekannt" % (wo, typ)
        if richtung not in EINTRAG_RICHTUNGEN:
            return '%s: richtung muss fest oder streng-zuerst sein' % wo
        if typ == 'text' and richtung != 'fest':
            return '%s: ein Text ist nicht ordenbar, nur richtung fest' % wo
        if typ == 'text' and any(k in sp for k in ('wahl', 'mengen', 'fehlt')):
            return '%s: wahl, mengen und fehlt gehoeren nicht zu typ text' % wo
        if typ == 'wahl' and not (isinstance(wahl, list) and len(wahl) >= 2 and all(_nichtleer(x) for x in wahl)
                                  and len(set(wahl)) == len(wahl)):
            return '%s: wahl muss eine Liste von mindestens zwei verschiedenen Werten sein' % wo
        if typ == 'liste' and wahl is not None and not (isinstance(wahl, list) and wahl and all(_nichtleer(x) for x in wahl)):
            return '%s: wahl muss eine nichtleere Liste sein' % wo
        if typ != 'liste' and 'fehlt' in sp:
            return '%s: fehlt gehoert nur zu typ liste' % wo
        if 'fehlt' in sp and (sp['fehlt'] not in ('alle', 'keine') or sp.get('pflicht')):
            return '%s: fehlt muss alle oder keine sein und schliesst pflicht aus' % wo
        if 'mengen' in sp:
            m = sp['mengen']
            if not (typ == 'wahl' and richtung == 'streng-zuerst' and isinstance(m, dict) and set(m) == set(wahl)
                    and all(isinstance(v, list) and all(_nichtleer(x) for x in v) for v in m.values())):
                return '%s: mengen braucht typ wahl, richtung streng-zuerst und je Wahlwert eine Liste' % wo
        if 'muster' in sp:
            try:
                if typ != 'text':
                    return '%s: muster gehoert nur zu typ text' % wo
                re.compile(sp['muster'])
            except (re.error, TypeError):
                return '%s: muster ist kein regulaerer Ausdruck' % wo
        if 'pflicht' in sp and not isinstance(sp['pflicht'], bool):
            return '%s: pflicht muss true oder false sein' % wo
    return None


def _menge(spez, e, f):
    """Elemente einer Listen- oder Mengenwahl eines Eintrags; None heisst "alle" (nur bei fehlt alle)."""
    if f not in e:
        return None if spez[f].get('fehlt') == 'alle' else frozenset()
    v = e[f]
    return frozenset(spez[f]['mengen'][v]) if 'mengen' in spez[f] else frozenset(v)


def karte_lockerung(zeile, a, b):
    """Gruende, warum der Wert `a` einer Karte mit eintrag und Richtung streng-zuerst lockerer ist als `b`
    (Teilmengenregel); leere Liste: a ist gleich streng oder strenger. Die Gruende nennen nie einen Wert."""
    if not isinstance(a, dict) or not isinstance(b, dict):
        return ['kein Objekt']
    spez, gruende = zeile['eintrag'], []
    for n, e in a.items():
        o = b.get(n)
        if o is None:
            gruende.append("Eintrag '%s' ist neu" % n)
            continue
        for f, sp in spez.items():
            if (f in e) == (f in o) and e.get(f) == o.get(f):
                continue
            if sp['richtung'] == 'fest':
                lockerer = True
            elif sp['typ'] == 'wahl' and 'mengen' not in sp:
                lockerer = f not in e or f not in o or sp['wahl'].index(e[f]) > sp['wahl'].index(o[f])
            else:
                ma, mo = _menge(spez, e, f), _menge(spez, o, f)
                lockerer = mo is not None and (ma is None or not ma <= mo)
            if lockerer:
                gruende.append("Eintrag '%s': Feld '%s' ist geaendert oder lockerer" % (n, f))
    return gruende


def _zeile_fehler(z):
    """Erster Fehler einer Schemazeile oder None."""
    if not isinstance(z, dict):
        return 'keine Zeile (Objekt erwartet)'
    name = z.get('schluessel')
    if not (isinstance(name, str) and _SCHLUESSEL_MUSTER.match(name)):
        return "schluessel fehlt oder ist kein Punktschluessel: %r" % (name,)
    unbekannt = [k for k in z if k not in SCHEMA_FELDER]
    if unbekannt:
        return "%s: unbekanntes Feld '%s'" % (name, unbekannt[0])
    for pflicht in SCHEMA_PFLICHT:
        if pflicht not in z:
            return "%s: Pflichtfeld '%s' fehlt" % (name, pflicht)
    if z['klasse'] not in SCHEMA_KLASSEN:
        return "%s: klasse '%s' unbekannt" % (name, z['klasse'])
    typ = z['typ']
    if typ not in SCHEMA_TYPEN:
        return "%s: typ '%s' unbekannt" % (name, typ)
    wahl = z.get('wahl')
    if typ in ('wahl', 'mehrfachwahl'):
        if not (isinstance(wahl, list) and len(wahl) >= 2 and all(_nichtleer(x) for x in wahl)
                and len(set(wahl)) == len(wahl)):
            return "%s: wahl muss bei typ %s eine Liste von mindestens zwei verschiedenen Werten sein" % (name, typ)
    elif wahl is not None:
        return "%s: wahl gehoert nur zu typ wahl oder mehrfachwahl" % name
    if 'eintrag' in z:
        if typ != 'karte':
            return '%s: eintrag gehoert nur zu typ karte' % name
        f = _eintrag_spez_fehler(name, z['eintrag'])
        if f:
            return f
    bereich = z.get('bereich')
    if bereich is not None:
        if typ != 'zahl':
            return "%s: bereich gehoert nur zu typ zahl" % name
        if not (isinstance(bereich, dict) and set(bereich) <= {'min', 'max', 'einheit'}
                and all(_zahl(bereich[k]) for k in ('min', 'max') if k in bereich)
                and ('einheit' not in bereich or _nichtleer(bereich['einheit']))
                and ('min' not in bereich or 'max' not in bereich or bereich['min'] <= bereich['max'])):
            return "%s: bereich muss {min, max, einheit} mit min <= max sein" % name
    richtung = z.get('richtung')
    if richtung is None:
        if z['klasse'] == 'betreiber':
            return "%s: richtung ist bei klasse betreiber Pflicht" % name
    else:
        if richtung not in SCHEMA_RICHTUNGEN:
            return "%s: richtung '%s' unbekannt" % (name, richtung)
        if richtung in RICHTUNG_TYP and typ not in RICHTUNG_TYP[richtung]:
            return "%s: richtung %s passt nicht zu typ %s" % (name, richtung, typ)
        if typ == 'karte' and richtung == 'streng-zuerst' and 'eintrag' not in z:
            return '%s: eine Karte mit richtung streng-zuerst braucht eintrag (Teilmengenregel)' % name
    vorgaben = []
    if z['vorgabe'] is not None:
        vorgaben.append(z['vorgabe'])
    je_profil = z.get('vorgabeJeProfil')
    if je_profil is not None:
        if not (isinstance(je_profil, dict) and je_profil and set(je_profil) <= {'einzelplatz', 'mandant'}):
            return "%s: vorgabeJeProfil braucht einzelplatz und/oder mandant" % name
        vorgaben.extend(je_profil.values())
    if z.get('empfehlung') is not None:
        vorgaben.append(z['empfehlung'])
    for v in vorgaben:
        f = wert_fehler(z, v)
        if f:
            return '%s: Vorgabe oder Empfehlung: %s' % (name, f)
    risiko = z.get('risiko')
    if risiko is not None:
        erlaubt = wahl if typ in ('wahl', 'mehrfachwahl') else ['true', 'false'] if typ == 'bool' else None
        if not (isinstance(risiko, dict) and erlaubt is not None and all(k in erlaubt and _nichtleer(v) for k, v in risiko.items())):
            return "%s: risiko muss Werte aus wahl (bool: true, false) auf einen Satz abbilden" % name
    if 'nurSystemebene' in z:
        ns = z['nurSystemebene']
        if not (ns is True or ns == 'mandant'):
            return "%s: nurSystemebene muss true oder \"mandant\" sein (oder fehlen)" % name
        # Vorgabe null heisst "nicht gesetzt": erlaubt nur bei typ text (decision 2026-10-27), fuer nurSystemebene true und
        # seit E3 Zusatz 3 (decision 2026-10-27: die vier pfade.* der Gruppe B) auch fuer "mandant". Bei true kommt der Wert
        # dann nur aus der Systemdatei; fehlt er dort, liefert der Leser keinen Ersatz. Bei "mandant" gilt im Einzelplatz
        # die Betreiberebene wie bei jedem Betreiberschluessel ohne Vorgabe. Eine Liste ohne Richtung braucht weiter eine
        # Vorgabe: fuer sie ist die Vorgabe kein Ersatz, ohne sie gaebe es keinen Rueckfall.
        # Pflichtangabe (decision 2026-11-11, audit.aufbewahrungTage und audit.einsichtRegel): auch typ zahl und typ wahl duerfen
        # ohne Vorgabe stehen, aber nur ohne Richtung (mit Richtung ersetzt der strengste Wert die Vorgabe, dann ist die
        # Vorgabe keine Pflicht der Firma); typ wahl ohne Richtung gilt nur ohne Vorgabe (ohne Richtung gibt es keinen strengeren Wert; eine
        # Vorgabe waere eine beliebige Wahl, die der Firma die Entscheidung abnimmt). Beides wirkt nur mit nurSystemebene
        # true; die Firma setzt den Wert in der Systemdatei, sonst gilt er als nicht gesetzt (der Aufrufer bricht ab).
        ohne_vorgabe_erlaubt = typ == 'text' or (typ in ('zahl', 'wahl') and richtung in (None, 'keine'))
        wahl_ohne_richtung = typ == 'wahl' and richtung in (None, 'keine')
        if z['klasse'] != 'betreiber' or (richtung in (None, 'keine') and typ not in ('text', 'liste', 'zahl', 'wahl')) \
                or (z['vorgabe'] is None and not ohne_vorgabe_erlaubt) \
                or (wahl_ohne_richtung and (z['vorgabe'] is not None or z['nurSystemebene'] is not True)):
            return "%s: nurSystemebene braucht klasse betreiber, eine Richtung (ausser typ text, liste, zahl und wahl; eine Wahl ohne" \
                   " Richtung nur mit nurSystemebene true und ohne Vorgabe) und eine Vorgabe (ohne Vorgabe nur bei typ text sowie zahl und wahl ohne Richtung)" % name
    if 'aussperrend' in z and (z['aussperrend'] is not True or richtung in (None, 'keine')):
        return '%s: aussperrend muss true sein und braucht eine Richtung' % name
    if 'ohneNutzerebene' in z:
        if z['ohneNutzerebene'] is not True:
            return '%s: ohneNutzerebene muss true sein (oder fehlen)' % name
        # Neben nurSystemebene nur "mandant" (w42, 28.10.2026: agents.routenpruefung); bei true wirkt die Nutzerebene ohnehin nie.
        if z['klasse'] != 'betreiber' or z.get('nurSystemebene') is True:
            return '%s: ohneNutzerebene braucht klasse betreiber und kein nurSystemebene true' % name
    if 'systemAlleinImMandant' in z:
        # Nur neben nurSystemebene "mandant" und einer Richtung: ohne Richtung wirkt "mandant" schon allein, bei true ohnehin immer.
        if z['systemAlleinImMandant'] is not True:
            return '%s: systemAlleinImMandant muss true sein (oder fehlen)' % name
        if z['klasse'] != 'betreiber' or z.get('nurSystemebene') != 'mandant' or richtung in (None, 'keine'):
            return '%s: systemAlleinImMandant braucht klasse betreiber, nurSystemebene "mandant" und eine Richtung' % name
    if 'nurSystemebeneWerte' in z:
        # Einzelne Werte nur aus der Systemdatei (NUR SYSTEMEBENE JE WERT): typ wahl mit Richtung, nur lockerere Werte als jede Vorgabe.
        nw = z['nurSystemebeneWerte']
        if z['klasse'] != 'betreiber' or typ != 'wahl' or richtung in (None, 'keine'):
            return '%s: nurSystemebeneWerte braucht klasse betreiber, typ wahl und eine Richtung' % name
        if not (isinstance(nw, list) and nw and all(isinstance(x, str) and x in wahl for x in nw) and len(set(nw)) == len(nw)):
            return '%s: nurSystemebeneWerte muss eine nichtleere Liste verschiedener Werte aus wahl sein' % name
        if 'systemAlleinImMandant' in z:
            return '%s: nurSystemebeneWerte und systemAlleinImMandant stehen nicht auf derselben Zeile' % name
        alle_vorgaben = [v for v in [z['vorgabe']] + list((je_profil or {}).values()) if v is not None]
        if not alle_vorgaben or any(not lockerer(z, x, v) for x in nw for v in alle_vorgaben):
            return '%s: jeder Wert in nurSystemebeneWerte muss lockerer sein als die Vorgabe' % name
    if 'umgebungNurNaht' in z:
        if z['umgebungNurNaht'] is not True:
            return '%s: umgebungNurNaht muss true sein (oder fehlen)' % name
        if z['klasse'] != 'betreiber' or 'nurSystemebene' in z:
            return '%s: umgebungNurNaht braucht klasse betreiber und kein nurSystemebene' % name
    if z.get('rolle') is not None and z['rolle'] not in SCHEMA_ROLLEN:
        return "%s: rolle '%s' unbekannt" % (name, z['rolle'])
    if z.get('nurMensch') is not None and not isinstance(z['nurMensch'], bool):
        return '%s: nurMensch muss true oder false sein' % name
    if z.get('quelle') is not None and not (isinstance(z['quelle'], list) and all(_nichtleer(x) for x in z['quelle'])):
        return '%s: quelle muss eine Liste von Zeichenketten sein' % name
    for f in ('grenzeVon', 'wirksamAb', 'seite', 'titel', 'beschreibung'):
        if z.get(f) is not None and not _nichtleer(z[f]):
            return '%s: %s muss eine nichtleere Zeichenkette sein' % (name, f)
    return None


class Schema:
    """Die geprueften Zeilen der Schemadatei und der gueltigen Fragmente (daten: beide zusammen)."""

    def __init__(self, daten, pfad=None, fragmente=(), meldungen=()):
        self.pfad = pfad
        self.daten = daten
        self.fragmente = list(fragmente)    # je gueltigem Fragment {'datei', 'praefix', 'schluessel'}
        self.meldungen = list(meldungen)    # je verworfenem Fragment eine Zeile
        self.zeilen = {z['schluessel']: z for z in daten['einstellungen']}
        self.karten = tuple(k for k, z in self.zeilen.items() if z['typ'] == 'karte')
        # Erste Segmente, unter denen NUR Betreiberschluessel stehen: darunter wirkt die Umgebung nie (auch nicht
        # auf unbekannte Schluessel). Ein gemischtes Praefix (pfade: Nutzerschluessel und pfade.registry) steht
        # nicht hier; seine Betreiberschluessel nennt betreiber_schluessel, die Umgebung wirkt dort je Schluessel nie.
        # Ein Praefix mit einem umgebungNurNaht-Schluessel zaehlt ebenfalls nicht hierher (E3 Zusatz 3, 27.10.2026):
        # seit alle pfade.* Betreiberschluessel sind, haette die Praefixsperre sonst auch die Naht WB_MODELS_FILE von
        # pfade.registry verworfen. Seine Betreiberschluessel laufen je Schluessel ueber betreiber_schluessel, dort
        # mit der Naht-Ausnahme; unbekannte Schluessel darunter bleiben damit ausserhalb der Praefixsperre.
        klassen, mit_naht = {}, set()
        for k, z in self.zeilen.items():
            klassen.setdefault(k.split('.')[0], set()).add(z['klasse'])
            if z.get('umgebungNurNaht'):
                mit_naht.add(k.split('.')[0])
        self.betreiber = frozenset(p for p, ks in klassen.items() if ks == {'betreiber'} and p not in mit_naht)
        self.betreiber_schluessel = tuple(k for k, z in self.zeilen.items()
                                          if z['klasse'] == 'betreiber' and k.split('.')[0] not in self.betreiber)
        # Betreiberschluessel, deren Umgebungsalias (ENV_ALIASE, nur Naht-Variablen) unter der Testnaht gilt.
        self.naht_umgebung = frozenset(k for k, z in self.zeilen.items() if z.get('umgebungNurNaht'))
        self.vorgaben = {}
        for k, z in self.zeilen.items():
            if z['vorgabe'] is not None:
                _setze(self.vorgaben, k.split('.'), json.loads(json.dumps(z['vorgabe'])))


def _lies_schemadatei(pfad, kopf=('version', 'einstellungen')):
    """Liest und prueft eine Schemadatei (Haupt oder Fragment); SchemaUngueltig beim ersten Fehler."""
    try:
        with open(pfad, 'r', encoding='utf-8') as f:
            daten = json.load(f, object_pairs_hook=_paare)
    except FileNotFoundError:
        raise SchemaUngueltig('%s: Schemadatei fehlt' % pfad)
    except (OSError, ValueError, UnicodeDecodeError) as e:
        raise SchemaUngueltig('%s: kein gueltiges JSON (%s)' % (pfad, str(e).splitlines()[0] if str(e) else type(e).__name__))
    if not (isinstance(daten, dict) and {'version', 'einstellungen'} <= set(daten) <= set(kopf)):
        raise SchemaUngueltig('%s: erwartet {"version":1,"einstellungen":[...]}' % pfad)
    if 'herkunft' in daten and not _nichtleer(daten['herkunft']):
        raise SchemaUngueltig('%s: herkunft muss eine nichtleere Zeichenkette sein' % pfad)
    if daten['version'] != 1 or isinstance(daten['version'], bool):
        raise SchemaUngueltig('%s: version %r wird nicht unterstuetzt (erwartet 1)' % (pfad, daten['version']))
    if not isinstance(daten['einstellungen'], list) or not daten['einstellungen']:
        raise SchemaUngueltig('%s: einstellungen muss eine nichtleere Liste sein' % pfad)
    gesehen = []
    for i, z in enumerate(daten['einstellungen']):
        f = _zeile_fehler(z)
        if f:
            raise SchemaUngueltig('%s: Zeile %d: %s' % (pfad, i + 1, f))
        if z['schluessel'] in gesehen:
            raise SchemaUngueltig("%s: Zeile %d: doppelter Schluessel '%s'" % (pfad, i + 1, z['schluessel']))
        for g in gesehen:
            if z['schluessel'].startswith(g + '.') or g.startswith(z['schluessel'] + '.'):
                raise SchemaUngueltig("%s: Zeile %d: '%s' und '%s' liegen ineinander" % (pfad, i + 1, g, z['schluessel']))
        gesehen.append(z['schluessel'])
    return daten


def _lade_fragmente(ordner, haupt, spur=None):
    """Gueltige Fragmente aus `ordner` nach Dateiname -> (zeilen, fragmente, meldungen). Wirft nie: ein kaputtes
    Fragment faellt als Ganzes aus und wird gemeldet. `haupt`: die Schluessel des Hauptschemas."""
    _spur(spur, ordner)
    try:
        namen = sorted(n for n in os.listdir(ordner) if n.endswith('.json') and not n.startswith('.'))
    except (FileNotFoundError, NotADirectoryError):
        return [], [], []
    except OSError as e:
        return [], [], ['SCHEMA-FRAGMENT: %s: Ordner unlesbar (%s), kein Fragment geladen' % (ordner, e.strerror or e)]
    erste = {k.split('.')[0] for k in haupt}
    zeilen, fragmente, meldungen = [], [], []
    for n in namen:
        pfad, praefix = os.path.join(ordner, n), n[:-len('.json')]
        _spur(spur, pfad)
        try:
            daten = _lies_schemadatei(pfad, ('version', 'einstellungen', 'herkunft'))
            for i, z in enumerate(daten['einstellungen']):
                if z['schluessel'] in haupt:
                    raise SchemaUngueltig("%s: Zeile %d: '%s' steht schon im Hauptschema; ein Fragment ueberschreibt nichts"
                                          % (pfad, i + 1, z['schluessel']))
            if not re.match(r'[A-Za-z0-9_-]+\Z', praefix):    # ein Segment: zwei Fragmente koennen sich nie ueberlappen
                raise SchemaUngueltig("%s: Dateiname '%s' ist kein einzelnes Schluesselsegment" % (pfad, praefix))
            if praefix in erste:
                raise SchemaUngueltig("%s: Praefix '%s.' gehoert dem Hauptschema" % (pfad, praefix))
            for i, z in enumerate(daten['einstellungen']):
                if not z['schluessel'].startswith(praefix + '.'):
                    raise SchemaUngueltig("%s: Zeile %d: '%s' liegt nicht unter dem Praefix '%s.' dieses Fragments"
                                          % (pfad, i + 1, z['schluessel'], praefix))
        except SchemaUngueltig as e:
            meldungen.append('SCHEMA-FRAGMENT verworfen: %s' % e)
            continue
        zeilen.extend(daten['einstellungen'])
        fragmente.append({'datei': pfad, 'praefix': praefix, 'schluessel': [z['schluessel'] for z in daten['einstellungen']]})
    return zeilen, fragmente, meldungen


def lade_schema(pfad=None, spur=None):
    """Hauptschema (Fehler: SchemaUngueltig) plus die gueltigen Fragmente aus FRAGMENT_ORDNER daneben."""
    pfad = pfad or SCHEMA_DATEI
    _spur(spur, pfad)
    daten = _lies_schemadatei(pfad)
    zeilen, fragmente, meldungen = _lade_fragmente(os.path.join(os.path.dirname(pfad), FRAGMENT_ORDNER),
                                                   {z['schluessel'] for z in daten['einstellungen']}, spur)
    if zeilen:
        daten = dict(daten, einstellungen=daten['einstellungen'] + zeilen)
    return Schema(daten, pfad, fragmente, meldungen)


def segmente(schluessel, schema):
    """'a.b.c' -> ['a','b','c']; ab einer Karte ist der Rest ein Name."""
    teile = schluessel.split('.')
    for k in schema.karten:
        n = k.count('.') + 1
        if teile[:n] == k.split('.') and len(teile) > n + 1:
            return teile[:n] + ['.'.join(teile[n:])]
    return teile


def _pruefe(daten, schema, pfad=''):
    """Erster Typfehler oder None; geht nur durch Objekte, nicht in Karten hinein."""
    for k, v in daten.items():
        p = pfad + '.' + k if pfad else k
        z = schema.zeilen.get(p)
        if z is not None:
            f = wert_fehler(z, v)
            if f:
                return f
        elif isinstance(v, dict):
            f = _pruefe(v, schema, p)
            if f:
                return f
    return None


def _zeilen_fehler(daten, schema, pfad=()):
    """Alle Typfehler je Schemazeile: [(Schluesselpfad, [Zeilen], Fehler)]. Ein Zwischenobjekt, das kein Objekt
    ist, ist ein Fehler aller Zeilen darunter, nicht still fehlend. Unbekannte Schluessel bleiben erlaubt."""
    fehler = []
    for k, v in daten.items():
        p = pfad + (k,)
        name = '.'.join(p)
        z = schema.zeilen.get(name)
        if z is not None:
            f = wert_fehler(z, v)
            if f:
                fehler.append((p, [name], f))
        elif isinstance(v, dict):
            fehler.extend(_zeilen_fehler(v, schema, p))
        else:
            unter = [n for n in schema.zeilen if n.startswith(name + '.')]
            if unter:
                fehler.append((p, unter, "'%s' muss ein Objekt sein" % name))
    return fehler


def _paare(paare):
    d = {}
    for k, v in paare:
        if k in d:
            raise ValueError("doppelter Schluessel '%s'" % k)
        d[k] = v
    return d


class Schicht:
    def __init__(self, name, pfad=None, daten=None):
        self.name = name
        self.pfad = pfad
        self.daten = daten if daten is not None else {}
        self.fehler = None        # Text, wenn die Datei da ist, aber nicht taugt
        self.vorhanden = daten is not None
        self.hinweis = None       # warum die Ebene ausbleibt (Plattform, Kennung)
        self.verworfen = {}       # nur System: Zeile -> Fehler der Zeilen, die verworfen wurden
        self.alias = False        # nur die Umgebungsebene der Variablen aus ENV_ALIASE (nicht die Datei WB_KONFIG)


_IDENTITAET = []


def _identitaet():
    """shell/wb_identitaet.py neben dieser Datei (realpath), einmal geladen; None, wenn es fehlt oder nicht laedt."""
    if not _IDENTITAET:
        modul = None
        try:
            import importlib.util
            spec = importlib.util.spec_from_file_location(
                'wb_identitaet', os.path.join(os.path.dirname(os.path.realpath(__file__)), 'wb_identitaet.py'))
            modul = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(modul)
        except Exception:
            modul = None
        _IDENTITAET.append(modul)
    return _IDENTITAET[0]


def _stelle():
    """(uid, heim) der Entscheidungsstelle oder None, wenn es die Kennung nicht gibt.

    Mit '{heim}' in BETREIBER_PFAD folgt die Stelle allein wb_identitaet.stelle_ordner() (Stufe 2b+, 27.10.2026):
    stelle.konto aus der Systemdatei nennt das Konto, ohne den Schluessel gilt das eigene (Einzelplatz wie
    bisher). Es gibt keinen zweiten Weg, die Stelle zu benennen. Ein leerer Stellen-Ordner (Konto unbekannt, Wert
    ungueltig) heisst keine Betreiberebene; fehlt wb_identitaet.py, ebenso (fail closed). Ein fester BETREIBER_PFAD
    (Testkopie) bleibt beim eigenen Konto."""
    if pwd is None or not hasattr(os, 'getuid'):
        return None
    try:
        if '{heim}' not in BETREIBER_PFAD:
            p = pwd.getpwuid(os.getuid())
        else:
            ident = _identitaet()
            if ident is None or not ident.stelle_ordner():
                return None
            konto = ident.stelle_konto()
            if konto is ident.FEHLT:
                return os.getuid(), ident.heimat()
            p = pwd.getpwnam(konto)
    except (KeyError, ValueError, TypeError):
        return None
    return p.pw_uid, p.pw_dir


def _heimat_der_kennung():
    """Heimat (pw_dir) der effektiven Kennung laut Benutzerdatenbank, None ohne Eintrag. Nie HOME aus der Umgebung."""
    if NUTZER_HEIMAT is not None:
        return NUTZER_HEIMAT
    if pwd is None or not hasattr(os, 'geteuid'):
        return None
    try:
        return pwd.getpwuid(os.geteuid()).pw_dir
    except KeyError:
        return None


def _spur(spur, pfad):
    """Merkt einen gelesenen oder gesuchten Pfad (Konfig.gelesen), wenn der Aufrufer eine Liste fuehrt."""
    if spur is not None and pfad is not None and pfad not in spur:
        spur.append(pfad)


def _kette_fehler(pfad, eigentuemer, bis=None, spur=None):
    """Fehlertext, wenn ein Ordner von dem der Datei (nach Aufloesung von Verweisen) bis `bis` einschliesslich
    nicht einem der Eigentuemer gehoert oder fuer Gruppe/Welt beschreibbar ist; sonst None."""
    d = os.path.dirname(os.path.realpath(pfad))
    bis = os.path.realpath(bis or SYSTEM_KETTE_BIS)
    while True:
        _spur(spur, d)
        try:
            st = os.stat(d)
        except OSError as e:
            return 'Ordner %s nicht lesbar: %s' % (d, e.strerror)
        if st.st_mode & 0o022:
            return 'Ordner %s mit Schreibrechten fuer Gruppe/Welt' % d
        if st.st_uid not in eigentuemer:
            return 'Ordner %s gehoert nicht root (uid %d)' % (d, st.st_uid)
        if d == bis or d == os.path.dirname(d):
            return None
        d = os.path.dirname(d)


def _lies_datei(name, pfad, schema, art=None, eigentuemer=None, ausdruecklich=False, spur=None):
    """art 'system' oder 'betreiber': Eigentuemer und Schreibrechte von Datei und Ordner pruefen.
    ausdruecklich: der Pfad wurde benannt (WB_KONFIG), nicht gesucht -- fehlt er, ist das ein Fehler
    und kein stiller Rueckfall (wie WB_MODELS_FILE in wb-state). spur: Liste, in die jeder gelesene oder
    gesuchte Pfad kommt (auch ein fehlender, auch die geprueften Ordner)."""
    s = Schicht(name, pfad)
    _spur(spur, pfad)
    try:
        st = os.stat(pfad)
    except FileNotFoundError:
        if ausdruecklich:
            s.fehler = 'ausdruecklich benannt, aber nicht vorhanden'
        return s
    except OSError as e:
        s.fehler = 'nicht lesbar: %s' % e.strerror
        return s
    s.vorhanden = True
    if not stat.S_ISREG(st.st_mode):
        s.fehler = 'keine regulaere Datei'
        return s
    if st.st_size > MAX_BYTES:
        s.fehler = 'ueber %d Bytes' % MAX_BYTES
        return s
    if art is not None:
        if st.st_mode & 0o022:
            s.fehler = 'Schreibrechte fuer Gruppe/Welt'
            return s
        if st.st_uid not in eigentuemer:
            s.fehler = 'gehoert nicht %s (uid %d)' % ('root' if art == 'system' else 'der Entscheidungsstelle', st.st_uid)
            return s
        if art == 'system':
            s.fehler = _kette_fehler(pfad, eigentuemer, spur=spur)
            if s.fehler:
                return s
        else:
            _spur(spur, os.path.dirname(pfad))
            try:
                dst = os.stat(os.path.dirname(pfad))
            except OSError as e:
                s.fehler = 'Ordner nicht lesbar: %s' % e.strerror
                return s
            if dst.st_mode & 0o022:
                s.fehler = 'Ordner mit Schreibrechten fuer Gruppe/Welt'
                return s
    try:
        with open(pfad, 'r', encoding='utf-8') as f:
            d = json.load(f, object_pairs_hook=_paare)
    except OSError as e:   # Rechte, nicht Inhalt: der Text darf die Ursache nicht als kaputtes JSON ausgeben
        s.fehler = 'nicht lesbar (%s)' % ('Rechte' if isinstance(e, PermissionError) else e.strerror or type(e).__name__)
        return s
    except (ValueError, UnicodeDecodeError) as e:
        s.fehler = 'kein gueltiges JSON (%s)' % (str(e).splitlines()[0] if str(e) else type(e).__name__)
        return s
    if not isinstance(d, dict) or d.get('schema') != SCHEMA:
        s.fehler = "schema muss '%s' sein" % SCHEMA
        return s
    daten = {k: v for k, v in d.items() if k != 'schema'}
    if art == 'system':
        # Nur die kaputten Zeilen verwerfen; eine kaputte Zeile gesperrt macht die ganze Ebene ungueltig.
        fehler = _zeilen_fehler(daten, schema)
        hart = [f for _, zeilen, f in fehler if 'gesperrt' in zeilen]
        if hart:
            s.fehler = hart[0]
            return s
        for p, zeilen, f in fehler:
            _entferne(daten, list(p))
            for n in zeilen:
                s.verworfen[n] = f
    else:
        f = _pruefe(daten, schema)
        if f:
            s.fehler = f
            return s
    s.daten = daten
    return s


def _system_pfad():
    return next((p for pl, p in SYSTEM_PFADE.items() if sys.platform.startswith(pl)), None)


def _betriebsart_aus(s, pfad):
    """Die Betriebsart zur gelesenen Systemebene S (Schicht) am Ort PFAD. Sie kann nur verschaerfen."""
    if BETRIEBSART != 'einzelplatz' or pfad is None:
        return 'mandant' if BETRIEBSART != 'einzelplatz' else BETRIEBSART
    if s.fehler or s.verworfen or s.daten.get('betriebsart') == 'mandant':
        return 'mandant'
    try:
        os.lstat(os.path.join(os.path.dirname(pfad), MARKE_NAME))
    except (FileNotFoundError, NotADirectoryError):
        return 'einzelplatz'
    except OSError:
        pass
    return 'mandant'


def betriebsart():
    """'mandant' oder 'einzelplatz' fuer diesen Rechner (Kopf, BETRIEBSART). Jeder Fehler ergibt 'mandant'."""
    pfad = _system_pfad()
    try:
        s = Schicht('system', None) if pfad is None else _lies_datei('system', pfad, lade_schema(), art='system', eigentuemer=SYSTEM_EIGENTUEMER)
        return _betriebsart_aus(s, pfad)
    except Exception:
        return 'mandant'


def _setze(daten, segs, wert):
    for sg in segs[:-1]:
        daten = daten.setdefault(sg, {})
    daten[segs[-1]] = wert


def _holen(daten, segs):
    for sg in segs:
        if not isinstance(daten, dict) or sg not in daten:
            return _FEHLT
        daten = daten[sg]
    return daten


def _entferne(daten, segs):
    """Entfernt segs aus daten; True, wenn etwas stand."""
    for sg in segs[:-1]:
        daten = daten.get(sg) if isinstance(daten, dict) else None
        if not isinstance(daten, dict):
            return False
    if isinstance(daten, dict) and segs[-1] in daten:
        del daten[segs[-1]]
        return True
    return False


def _kopie(x):
    return json.loads(json.dumps(x))


def _mische(oben, unten):
    """Objekte Unterschluessel fuer Unterschluessel, alles andere ersetzt die untere Ebene."""
    if isinstance(oben, dict) and isinstance(unten, dict):
        r = dict(unten)
        for k, v in oben.items():
            r[k] = _mische(v, unten[k]) if k in unten else v
        return r
    return oben


def strengster(zeile):
    """Strengster Wert einer Schemazeile mit Richtung; _FEHLT ohne Richtung oder ohne Grenze im bereich."""
    r, typ = zeile.get('richtung'), zeile['typ']
    if r == 'streng-zuerst':
        return {} if typ == 'karte' else [] if typ in MENGEN_TYPEN else zeile['wahl'][0]
    b = zeile.get('bereich') or {}
    if r == 'groesser-lockert':
        return b.get('min', _FEHLT)
    if r == 'kleiner-lockert':
        return b.get('max', _FEHLT)
    if r == 'true-lockert':
        return False
    if r == 'false-lockert':
        return True
    return _FEHLT


def lockerer(zeile, a, b):
    """True, wenn a nach der Richtung der Schemazeile lockerer ist als b. Richtung 'keine': nie."""
    r = zeile.get('richtung')
    if r == 'streng-zuerst' and zeile['typ'] == 'karte':
        return bool(karte_lockerung(zeile, a, b))       # Eintraege wegnehmen oder Felder verschaerfen
    if r == 'streng-zuerst' and zeile['typ'] in MENGEN_TYPEN:
        return not set(a) <= set(b)       # strenger oder gleich ist nur eine Teilmenge (Eintraege wegnehmen)
    if r == 'streng-zuerst':
        return zeile['wahl'].index(a) > zeile['wahl'].index(b)
    if r == 'groesser-lockert':
        return a > b
    if r == 'kleiner-lockert':
        return a < b
    if r == 'true-lockert':
        return a is True and b is False
    if r == 'false-lockert':
        return a is False and b is True
    return False


class Konfig:
    def __init__(self, env=None, plattform=None, schema=None, heim=None):
        env = os.environ if env is None else env
        plattform = sys.platform if plattform is None else plattform
        # Jeder Pfad, den dieser Aufbau gelesen oder gesucht hat (Schemadatei, Ebenendateien auch fehlend, Ordnerkette
        # der Systemdatei, Ordner der Betreiberdatei). Ein Aufrufer im Prozess (wb_datenklasse_regel.py) behaelt das
        # Ergebnis nur, solange keiner dieser Pfade sich aendert. Ein mitgegebenes Schema steht nicht darin.
        self.gelesen = []
        self.schema = schema or lade_schema(spur=self.gelesen)
        sc = self.schema
        self.meldungen = list(sc.meldungen)     # verworfene Schema-Fragmente zuerst
        self.schichten = []       # Reihenfolge der Nutzerschluessel: Umgebung, Nutzer, Betreiber, System
        self.untergrenzen = {}    # Schluessel -> (Wert, Ebene) der Betreiberebenen
        # Umgebung: Umgebungsvariablen, danach die Datei WB_KONFIG.
        u = Schicht('umgebung', None, {})
        u.alias = True
        for key, var in ENV_ALIASE.items():
            wert = _naht_wert(var, env, self.meldungen) if var in NAHT_VARIABLEN else env.get(var)
            if wert:
                _setze(u.daten, segmente(key, sc), wert)
        self.schichten.append(u)
        if env.get('WB_KONFIG'):
            self.schichten.append(_lies_datei('umgebung', os.path.expanduser(env['WB_KONFIG']), sc, ausdruecklich=True,
                                              spur=self.gelesen))
        # Nutzer. Nutzerschluessel lesen die Datei unter XDG_CONFIG_HOME (sonst HOME). Betreiberschluessel lesen
        # die Datei in der Heimat laut Benutzerdatenbank: diese eine Regel steht hier, nicht im Leser.
        # Mit heim (ausdruecklich) lesen beide die Datei unter heim, nie HOME, XDG_CONFIG_HOME oder die Prozesskennung.
        if heim is not None:
            if os.path.isabs(heim):
                nutzer = _lies_datei('nutzer', os.path.join(heim, '.config', 'agent-workbench', DATEINAME), sc,
                                     spur=self.gelesen)
            else:         # fail closed: eine relative Heimat hinge vom Arbeitsordner ab
                nutzer = Schicht('nutzer', heim)
                nutzer.vorhanden, nutzer.fehler = True, 'Heimat %r ist kein absoluter Pfad' % (heim,)
            self.schichten.append(nutzer)
            nutzer_b = nutzer
        else:
            home = env.get('HOME') or os.path.expanduser('~')
            xdg = env.get('XDG_CONFIG_HOME') or os.path.join(home, '.config')
            pfad_nutzer = os.path.join(xdg, 'agent-workbench', DATEINAME)
            nutzer = _lies_datei('nutzer', pfad_nutzer, sc, spur=self.gelesen)
            self.schichten.append(nutzer)
            heim = _heimat_der_kennung()
            pfad_betreiber_nutzer = None if heim is None else os.path.join(heim, '.config', 'agent-workbench', DATEINAME)
            if heim is None:
                nutzer_b = Schicht('nutzer', None)
                nutzer_b.hinweis = 'ausgelassen (Kennung ohne Heimat in der Benutzerdatenbank)'
                self.schichten.append(nutzer_b)
            elif os.path.normpath(pfad_betreiber_nutzer) == os.path.normpath(pfad_nutzer):
                nutzer_b = nutzer
            else:
                nutzer_b = _lies_datei('nutzer', pfad_betreiber_nutzer, sc, spur=self.gelesen)
                self.schichten.append(nutzer_b)
        self.nutzerdatei = {'nutzer': nutzer, 'betreiber': nutzer_b}
        # Betreiber: die Datei der Entscheidungsstelle.
        stelle = _stelle()
        if stelle is None:
            b = Schicht('betreiber', None)
            b.hinweis = 'ausgelassen (Kennung der Stelle ohne Eintrag)'
        else:
            # Heimat '/' (etwa nobody unter Linux) ergaebe sonst '//.local/...'; so gleicht der Pfad stelle_ordner().
            b = _lies_datei('betreiber', BETREIBER_PFAD.replace('{heim}', stelle[1].rstrip('/')), sc,
                            art='betreiber', eigentuemer=(stelle[0],), spur=self.gelesen)
        self.schichten.append(b)
        # System.
        pfad = next((p for pl, p in SYSTEM_PFADE.items() if plattform.startswith(pl)), None)
        if pfad is None:
            s = Schicht('system', None)
            s.hinweis = 'ausgelassen (nur Linux und macOS)'
        else:
            s = _lies_datei('system', pfad, sc, art='system', eigentuemer=SYSTEM_EIGENTUEMER, spur=self.gelesen)
        self.schichten.append(s)
        self.betriebsart = _betriebsart_aus(s, pfad)
        self.vorgaben = Schicht('vorgabe', None, _kopie(sc.vorgaben))
        self.vorgaben.vorhanden = True
        self._bereinige()
        self._system_ersatz()
        self.gelesen = tuple(self.gelesen)

    # -- Aufbau -----------------------------------------------------------------------------------
    def _schicht(self, name):
        return next((s for s in self.schichten if s.name == name), None)

    def _gut(self, s):
        return s is not None and s.vorhanden and not s.fehler

    def _bereinige(self):
        sc = self.schema
        system = self._schicht('system')
        self.gesperrt = []
        self.nur_system_roh = {}      # Schluessel -> [(Ebene, Rohwert)] der verworfenen Werte nur-Systemebene-Schluessel
        if self._gut(system):
            self.gesperrt = list(system.daten.get('gesperrt', []))
        locks = [segmente(k, sc) for k in self.gesperrt]
        for s in self.schichten:
            if s.name == 'system' or s.fehler:
                continue
            if 'gesperrt' in s.daten:
                self.meldungen.append("'gesperrt' in %s wirkt nicht (nur die Systemebene sperrt); ignoriert" % s.name)
                del s.daten['gesperrt']
            if s.name == 'umgebung':
                for k in [k for k in s.daten if k in sc.betreiber]:
                    self.meldungen.append("'%s' ist ein Betreiberschluessel (nie aus der Umgebung); Wert aus umgebung ignoriert" % k)
                    del s.daten[k]
            if s.name == 'umgebung':
                for k in sc.betreiber_schluessel:
                    if s.alias and k in sc.naht_umgebung:
                        continue      # gilt unter der Naht (naht_variable hat es sonst schon verworfen), unter System und Sperre
                    if _entferne(s.daten, segmente(k, sc)):
                        self.meldungen.append("'%s' ist ein Betreiberschluessel (nie aus der Umgebung); Wert aus umgebung ignoriert" % k)
            for L in locks:
                if _entferne(s.daten, L):
                    self.meldungen.append("'%s' ist gesperrt (System); Wert aus %s ignoriert" % ('.'.join(L), s.name))
            if s.name == 'nutzer':
                for name, z in sc.zeilen.items():
                    if z.get('ohneNutzerebene') and _entferne(s.daten, segmente(name, sc)):
                        self.meldungen.append("'%s' kommt nicht aus der Nutzerebene (Betreiberschluessel); Wert aus nutzer ignoriert" % name)
            for name, z in sc.zeilen.items():
                if not self.nur_system_fest(z):
                    continue
                segs = segmente(name, sc)
                roh = _holen(s.daten, segs)
                if _entferne(s.daten, segs):
                    self.meldungen.append("'%s' kommt nur aus der Systemebene; Wert aus %s ignoriert" % (name, s.name))
                    self.nur_system_roh.setdefault(name, []).append((s.name, roh))
            for name, z in sc.zeilen.items():
                nw = z.get('nurSystemebeneWerte')
                if not nw:
                    continue
                segs = segmente(name, sc)
                roh = _holen(s.daten, segs)
                if roh is not _FEHLT and roh in nw and _entferne(s.daten, segs):
                    self.meldungen.append("'%s': der Wert %s gilt nur aus der Systemdatei; Wert aus %s ignoriert"
                                          % (name, json.dumps(roh, ensure_ascii=False), s.name))
                    self.nur_system_roh.setdefault(name, []).append((s.name, roh))
        self._verschaerfe()
        for s in self.schichten:
            if s.fehler:
                self.meldungen.append('%s (%s): %s' % (s.name, s.pfad, s.fehler))

    def _system_ersatz(self):
        """Ist die Systemdatei ungueltig oder hat sie verworfene Zeilen: je betroffenem Betreiberschluessel der
        Ersatz nach SYSTEMEBENE UNGUELTIG.
        system_ersatz: Schluessel -> ('streng', Wert) | ('vorgabe', None) | ('abbruch', Grund)."""
        self.system_ersatz = {}
        self.system_ungueltig = None
        system = self._schicht('system')
        if system is None or not (system.fehler or system.verworfen):
            return
        self.streng = Schicht('streng', system.pfad, {})
        if system.fehler:
            art, grund, betroffen = 'ebene', system.fehler, list(self.schema.zeilen)
            kopf = 'SYSTEMEBENE UNGUELTIG (%s: %s)' % (system.pfad, grund)
        else:
            art, betroffen = 'zeilen', list(system.verworfen)
            grund = 'Zeilen verworfen: ' + '; '.join('%s (%s)' % kv for kv in system.verworfen.items())
            kopf = 'SYSTEMEBENE (%s): %s' % (system.pfad, grund)
        listen = {'streng': [], 'vorgabe': [], 'abbruch': []}
        for name in betroffen:
            z = self.schema.zeilen[name]
            if z['klasse'] != 'betreiber':
                continue
            if z.get('richtung') not in (None, 'keine'):
                w = strengster(z)
                if z.get('aussperrend') or w is _FEHLT:
                    e = ('abbruch', "%s; der strengste Wert von '%s' sperrt aus" % (kopf, name))
                else:
                    e = ('streng', w)
                    self.untergrenzen[name] = (w, 'streng')
            elif self.nur_system_fest(z) and z['typ'] != 'liste':
                e = ('vorgabe', None)
            elif system.fehler:
                e = ('abbruch', '%s (%s): %s' % (system.name, system.pfad, system.fehler))
            else:
                e = ('abbruch', "%s (%s): Zeile '%s' verworfen: %s" % (system.name, system.pfad, name, system.verworfen[name]))
            self.system_ersatz[name] = e
            listen[e[0]].append(name)
        self.system_ungueltig = {'art': art, 'pfad': system.pfad, 'grund': grund,
                                 'zeilen': dict(system.verworfen) if art == 'zeilen' else None,
                                 'streng': listen['streng'], 'vorgabe': listen['vorgabe'], 'abbruch': listen['abbruch']}
        self.meldungen.append('%s: strengster Wert fuer %s; Vorgabe fuer %s; Abbruch fuer %s'
                              % (kopf, ', '.join(listen['streng']) or '-', ', '.join(listen['vorgabe']) or '-',
                                 ', '.join(listen['abbruch']) or '-'))

    def nur_system(self, zeile):
        """True, wenn ein lockererer Wert als die Vorgabe nur aus der Systemdatei kommt: nurSystemebene true,
        oder "mandant" im Mandantenmodus (betriebsart())."""
        ns = zeile.get('nurSystemebene')
        return ns is True or (ns == 'mandant' and self.betriebsart == 'mandant')

    def nur_system_fest(self, zeile):
        """True bei nurSystemebene ohne Richtung (typ text): der Wert kommt nur aus System oder Vorgabe. Ebenso bei
        systemAlleinImMandant im Mandantenmodus: Betreiber- und Nutzerwerte werden auch dann verworfen, wenn sie verschaerfen
        wuerden (vault.betrieb; die Ordnung der Wahl bleibt, die kaputte Systemdatei ergibt weiter den strengsten Wert)."""
        return self.nur_system(zeile) and (zeile.get('richtung') in (None, 'keine') or zeile.get('systemAlleinImMandant') is True)

    def _verschaerfe(self):
        """Nur verschaerfen. Unterhalb der obersten gueltigen Betreiber-Ebene (System, Betreiber) gilt je
        Schluessel mit Richtung nur ein Wert, der nicht lockerer ist als die Untergrenze. Bei nurSystemebene
        ist nur die Systemdatei oben frei; ohne gueltige Systemdatei ist die Vorgabe die Untergrenze."""
        system, betreiber = self._schicht('system'), self._schicht('betreiber')
        ops = [s for s in (system, betreiber) if self._gut(s)]
        for name, z in self.schema.zeilen.items():
            if z['klasse'] != 'betreiber' or z.get('richtung') in (None, 'keine'):
                continue
            if self.nur_system(z):
                oben = system if self._gut(system) else None
                tiefer = [s for s in (betreiber, self._nutzer_fuer('betreiber')) if self._gut(s)]
            elif ops:
                oben, tiefer = ops[0], [s for s in ops[1:] + [self._nutzer_fuer('betreiber')] if self._gut(s)]
            else:
                continue
            segs = name.split('.')
            w0 = _holen(oben.daten, segs) if oben is not None else _FEHLT
            grenze, ebene = (w0, oben.name) if w0 is not _FEHLT else (
                (z['vorgabe'], 'vorgabe') if z['vorgabe'] is not None else (_FEHLT, None))
            unter = (grenze, ebene)
            for s in tiefer:
                w = _holen(s.daten, segs)
                if w is not _FEHLT:
                    if grenze is not _FEHLT and lockerer(z, w, grenze):
                        if z['typ'] == 'karte':     # die Gruende statt der Werte (Eintraege tragen Verweise, nie Werte)
                            self.meldungen.append("'%s': Wert aus %s ist lockerer als der aus %s (%s); ignoriert"
                                                  % (name, s.name, ebene, '; '.join(karte_lockerung(z, w, grenze))))
                        else:
                            self.meldungen.append("'%s': Wert %s aus %s ist lockerer als %s (%s); ignoriert"
                                                  % (name, json.dumps(w, ensure_ascii=False), s.name,
                                                     json.dumps(grenze, ensure_ascii=False), ebene))
                        _entferne(s.daten, segs)
                    elif grenze is not _FEHLT and (set(w) == set(grenze) if z['typ'] in MENGEN_TYPEN else w == grenze):
                        _entferne(s.daten, segs)      # gleicher Wert: die hoehere Ebene bleibt die Quelle
                    else:
                        grenze, ebene = w, s.name
                if s.name != 'nutzer':
                    unter = (grenze, ebene)
            if unter[0] is not _FEHLT:
                self.untergrenzen[name] = unter

    # -- Abfrage ----------------------------------------------------------------------------------
    def klasse(self, segs):
        z = self.schema.zeilen.get('.'.join(segs))
        if z is not None:
            return z['klasse']
        return 'betreiber' if segs and segs[0] in self.schema.betreiber else 'nutzer'

    def _ebenen(self, klasse='nutzer', verschaerfend=False, naht=False):
        """Ebenen eines Schluessels, die oberste zuerst. Ein Schluessel mit Richtung (verschaerfend) nimmt
        die tiefste Ebene zuerst: was dort noch steht, ist nicht lockerer als alles darueber. naht: das Umgebungsalias
        (umgebungNurNaht) zaehlt zwischen Betreiber und System."""
        if klasse == 'betreiber':
            alias = [s for s in self.schichten if s.alias] if naht else []
            reihe = [self._nutzer_fuer('betreiber'), self._schicht('betreiber')] + alias + [self._schicht('system')]
            if not verschaerfend:
                reihe.reverse()
        else:
            reihe = [s for s in self.schichten if s.name == 'umgebung'] + \
                [self._nutzer_fuer('nutzer'), self._schicht('betreiber'), self._schicht('system')]
        return [s for s in reihe if self._gut(s)] + [self.vorgaben]

    def _nutzer_fuer(self, klasse):
        """Die Nutzerdatei, die Schluessel dieser Klasse lesen: Betreiberschluessel die Datei der Heimat."""
        return self.nutzerdatei['betreiber' if klasse == 'betreiber' else 'nutzer']

    def ungueltig(self, betreiber=False, ohne_system=False, ohne_nutzer=False):
        """Ungueltige Ebenen. Mit betreiber nur die, die ein Betreiberschluessel liest (ohne_nutzer: auch die
        Nutzerdatei nicht, ohne_system: auch die Systemdatei nicht)."""
        def liest(s):
            if not betreiber:
                return True
            if s.name == 'nutzer' and ohne_nutzer:
                return False
            return s.name != 'umgebung' and (s.name != 'nutzer' or s is self.nutzerdatei['betreiber'])
        return [s for s in self.schichten if s.fehler and liest(s) and not (ohne_system and s.name == 'system')]

    def ist_gesperrt(self, segs):
        return any(segs[:len(L)] == L for L in (segmente(k, self.schema) for k in self.gesperrt))

    def _pruefe_betreiber(self, segs):
        if self.klasse(segs) != 'betreiber':
            return
        z = self.schema.zeilen.get('.'.join(segs))
        name = '.'.join(segs)
        e = self.system_ersatz.get(name)
        if e is not None and e[0] == 'abbruch':
            raise KonfigUngueltig(e[1])
        if e is not None:
            return        # strengster Wert oder Vorgabe: keine Ebene kann ihn aendern
        unter = [n for n in self.system_ersatz if n.startswith(name + '.')]
        if unter:         # ein Objekt ueber einer verworfenen Zeile: nicht halb gemischt liefern
            raise KonfigUngueltig("'%s' enthaelt die Systemzeile '%s' (%s)" % (name, unter[0], self.system_ungueltig['grund']))
        if z is not None and self.nur_system_fest(z):
            return        # liest nur System oder Vorgabe; eine kaputte Systemdatei zaehlt als nicht da
        # Bei nurSystemebene gilt eine fehlende oder kaputte Systemdatei als nicht da (Vorgabe, verschaerft).
        fehler = self.ungueltig(betreiber=True, ohne_system=z is not None and self.nur_system(z),
                                ohne_nutzer=z is not None and bool(z.get('ohneNutzerebene')))
        if fehler:
            s = fehler[0]
            raise KonfigUngueltig('%s (%s): %s' % (s.name, s.pfad, s.fehler))

    def quelle(self, schluessel, vorgabe=_FEHLT, pruefen=True):
        """(Wert, Ebenenname) oder (_FEHLT, None); bei Objekten ist der Name der der obersten Ebene."""
        wert, s = self.wirksam(schluessel, vorgabe, pruefen)
        return wert, (s.name if s is not None else None)

    def wirksam(self, schluessel, vorgabe=_FEHLT, pruefen=True):
        """(Wert, Schicht) oder (_FEHLT, None); Schicht ist die oberste Ebene mit Wert, sonst die Vorgabe.
        Die Schicht nennt Name und Pfad der Datei, aus der der Wert stammt."""
        segs = segmente(schluessel, self.schema)
        if pruefen:
            self._pruefe_betreiber(segs)
        e = self.system_ersatz.get('.'.join(segs))
        if e is not None and e[0] == 'streng':
            return _kopie(e[1]), self.streng
        obere, oberste = [], None
        z = self.schema.zeilen.get('.'.join(segs))
        verschaerfend = z is not None and z.get('richtung') not in (None, 'keine')
        atomar = verschaerfend and z['typ'] == 'karte'     # Karte mit Teilmengenregel: die tiefste Ebene gilt ganz
        for s in self._ebenen(self.klasse(segs), verschaerfend, bool(z is not None and z.get('umgebungNurNaht'))):
            w = _holen(s.daten, segs)
            if w is _FEHLT:
                continue
            if oberste is None:
                oberste = s
            obere.append(w)
            if not isinstance(w, dict) or atomar:
                break
        if vorgabe is not _FEHLT:
            obere.append(vorgabe)
            oberste = self.vorgaben if oberste is None else oberste
        if not obere:
            return _FEHLT, None
        wert = obere[-1]
        for w in reversed(obere[:-1]):
            wert = _mische(w, wert)
        return wert, oberste

    def hole(self, schluessel, vorgabe=_FEHLT):
        """Wert des Schluessels; fehlt er in jeder Ebene und gibt der Leser keine Vorgabe mit, KeyError."""
        w, _ = self.quelle(schluessel, vorgabe)
        if w is _FEHLT:
            raise KeyError(schluessel)
        return w

    def registry(self, heim, umgebung=None):
        """(Pfad, Ebene) der Modellregistry: die EINE Aufloesung fuer wb-state, das Datenklassen-Tor und `wb-konfig get`.
        pfade.registry ist ein Betreiberschluessel ohne Nutzerebene: System, Betreiber, Vorgabe (ein Wert in der
        Nutzerdatei wird gemeldet und ignoriert). Die Umgebung (WB_MODELS_FILE) zaehlt NUR unter der Testnaht
        (umgebungNurNaht; naht_variable in wb_naht_ziel.py entscheidet und meldet), und auch dann weder gegen einen Wert
        der Systemebene noch gegen eine Sperre der Systemdatei. Ebene: system | umgebung | betreiber | vorgabe. `~` am
        Anfang steht fuer `heim` (die Heimat des Aufrufers, nie HOME aus dieser Umgebung). KonfigUngueltig, wenn System-
        oder Betreiberdatei unbrauchbar sind (sonst fiele die Sperre still weg). `umgebung`: ein Aufrufer, der die Naht selbst
        geprueft hat und kein Umgebungsalias ueber den Leser braucht (wb-datenklasse), gibt WB_MODELS_FILE hier mit; es gilt
        wie das Alias: unter dem System und seiner Sperre nie."""
        wert, ebene = self.quelle('pfade.registry')
        if umgebung and ebene not in ('system', 'umgebung') and not self.ist_gesperrt(['pfade', 'registry']):
            return umgebung, 'umgebung'      # der Aufrufer hat die Naht schon geprueft (das Tor, auch ohne wb_naht_ziel.py daneben)
        if isinstance(wert, str) and (wert == '~' or wert.startswith('~/')):
            wert = heim + wert[1:]
        return wert, ebene

    def hole_pfad(self, schluessel, vorgabe=_FEHLT):
        w = self.hole(schluessel, vorgabe)
        return os.path.expanduser(w) if isinstance(w, str) else w


def lade(env=None, plattform=None, schema=None, heim=None):
    """heim: Heimat der Nutzerebene fuer Nutzer- und Betreiberschluessel (siehe EBENEN); None wie bisher."""
    return Konfig(env, plattform, schema, heim)


# --- Leser fuer die Ticket-Adapter (task, 28.10.2026) ------------------------------------------------------
# EIN Leser fuer alle tickets.*-Schluessel (Fragment schema.d/tickets.json): Adapter, Hub-Backend und Abgleich rufen ihn
# und werten die Ebenen nie selbst aus. Er liest nur die Verweise der Konten (schluesselbund:<dienst>/<konto>); den Wert
# im Schluesselbund liest er nie, und kein Rueckgabefeld nimmt einen auf.
_KONTO_VERWEIS = re.compile(r'schluesselbund:([A-Za-z0-9][A-Za-z0-9._-]*)/([A-Za-z0-9][A-Za-z0-9._@-]*)\Z')


def konto_teile(verweis):
    """('dienst', 'konto') aus einem Verweis schluesselbund:<dienst>/<konto>; None bei jeder anderen Form."""
    m = _KONTO_VERWEIS.match(verweis) if isinstance(verweis, str) else None
    return (m.group(1), m.group(2)) if m else None


def tickets_einstellungen(konfig=None, **lade_args):
    """Die wirksamen tickets.*-Werte als ein Objekt (Schluessel wie im Fragment, ohne Praefix):
      austausch, importKlasse, csv, csvMaxKlasse, ki {hub, schreibFreigabe}, abgleich {minuten, trockenErst},
      ziele {Name: {system, url, projekt, maxKlasse, kunden, richtung, staende, konto}} -- url, projekt und konto
      sind None, wenn sie fehlen; kunden ist eine Liste (fehlt = []); staende ist eine Liste oder None fuer "alle";
      konto ist nur der Verweis. risikoAbnahme (Ticketkern) gehoert nicht hierher.
    meldungen: die Meldungen des Lesers (verworfene Lockerungen u. a.). fehler: Schluessel -> Grund, wenn der Wert
    nicht gelesen werden konnte (ungueltige Ebene): dann gilt fail closed der strengste Wert der Schemazeile
    (austausch aus, ziele {}), nie die Vorgabe. `konfig`: ein schon geladenes Konfig; sonst lade(**lade_args)."""
    k = konfig or lade(**lade_args)
    werte, fehler = {}, {}
    for name, z in k.schema.zeilen.items():
        if not name.startswith('tickets.') or name == 'tickets.risikoAbnahme':
            continue
        try:
            w, _ = k.wirksam(name)
        except KonfigUngueltig as e:
            fehler[name] = str(e)
            w = _FEHLT
        if w is _FEHLT:
            w = strengster(z)
            if w is _FEHLT:
                w = _kopie(z['vorgabe'])
        _setze(werte, name.split('.')[1:], _kopie(w))
    for e in werte.get('ziele', {}).values():
        for f in ('url', 'projekt', 'konto'):
            e.setdefault(f, None)
        e.setdefault('kunden', [])
        e.setdefault('staende', None)
        if e['konto'] is not None and konto_teile(e['konto']) is None:     # nach der Pruefung nicht moeglich; nie weiterreichen
            e['konto'] = None
    werte['meldungen'] = list(k.meldungen)
    werte['fehler'] = fehler
    return werte


# --- Leser fuer Ausgang und Werkzeugliste (R4, 04.11.2026) ----------------------------------------------------------
# EIN Leser fuer die Zeilen ausgang.*, websuche.*, abruf.*, eingang.*, audit.* und rechte.werkzeuge (docs/AUSGANG.md).
# wb-grenze baut daraus die Regeln des Brokers, wb-frage-gate liest die Werkzeugliste. Fail closed wie bei den Tickets:
# Kann ein Wert nicht gelesen werden, gilt der strengste Wert der Schemazeile, ohne strengsten Wert die Vorgabe.
AUSGANG_PRAEFIXE = ('ausgang.', 'websuche.', 'abruf.', 'eingang.', 'audit.')
# Ein Werkzeug draussen: websuche, webabruf, mcp:<server>, tunnel:<host-muster>. Jede andere Form leert die ganze Liste.
WERKZEUG_FORM = re.compile(r'(websuche|webabruf|mcp:[A-Za-z0-9][A-Za-z0-9_.-]{0,63}|tunnel:[a-z0-9*?][a-z0-9*?.-]{0,252})\Z')


def ausgang_einstellungen(konfig=None, **lade_args):
    """{'werte': {Schluessel: Wert} der Ausgangszeilen, 'werkzeuge': [...], 'meldungen': [...], 'fehler': {Schluessel: Grund}}.
    werkzeuge ist rechte.werkzeuge nach der Formpruefung; ein Eintrag in anderer Form macht sie leer (Grund unter fehler,
    ohne den Wert). Ein Schluessel ohne Wert (websuche.anbieter.url ohne Systemwert) steht als None in werte."""
    k = konfig or lade(**lade_args)
    werte, fehler = {}, {}
    for name, z in k.schema.zeilen.items():
        if not (name.startswith(AUSGANG_PRAEFIXE) or name == 'rechte.werkzeuge'):
            continue
        try:
            w, _ = k.wirksam(name)
        except KonfigUngueltig as e:
            fehler[name] = str(e)
            w = strengster(z)
        if w is _FEHLT:
            w = _kopie(z['vorgabe'])
        werte[name] = _kopie(w)
    liste = werte.pop('rechte.werkzeuge', None) or []
    if not all(isinstance(x, str) and WERKZEUG_FORM.match(x) for x in liste):
        fehler['rechte.werkzeuge'] = 'ein Eintrag hat keine bekannte Form (websuche, webabruf, mcp:<server>, tunnel:<host>)'
        liste = []
    return {'werte': werte, 'werkzeuge': list(liste), 'meldungen': list(k.meldungen), 'fehler': fehler}


# --- Schreiben (E4, 27.10.2026) ------------------------------------------------------------------------------------
# Der EINE Schreibweg fuer Einstellungen: setze() schreibt einen Nutzerschluessel in die Nutzerdatei, die auch der
# Leser fuer Nutzerschluessel liest (Konfig.nutzerdatei['nutzer']). `wb-konfig setze` ruft ihn, der Kern ruft
# `wb-konfig setze` (argv, ohne Shell); spaeter auch `wb-state settings set` (E2). Vertrag: Kopf dieser Datei.
#
# Nur die Nutzerebene und nur Schluessel mit klasse "nutzer" ohne nurSystemebene (true oder "mandant") und ohne
# ohneNutzerebene; alles andere lehnt setze() ab, auch wenn ein Aufrufer die Klasse anders liest. Ein Schluessel, den
# die Systemdatei sperrt, ist ebenfalls zu. Der Wert muss zu Typ (wert_fehler) und Richtung passen: ein Wert, der
# lockerer ist als die Untergrenze der Betreiberebenen, wuerde still verworfen und wird deshalb abgelehnt.
#
# VERSION. Je Schluessel ist die Version ein sha256 ueber Schluessel und den Rohwert in der Nutzerdatei, die setze()
# schreibt ("fehlt" hat eine eigene Version). Sie ist undurchsichtig: Aufrufer vergleichen sie nur. setze() schreibt
# nur, wenn die mitgegebene Version die aktuelle ist (sonst SetzeAbgelehnt 'konflikt' mit der aktuellen Version).
#
# ATOMAR. Eine exklusive flock-Sperre auf <ordner>/.werkbank.json.sperre umfasst Lesen, Versionsvergleich und
# Schreiben; zwei gleichzeitige Schreiber mit derselben Version: der zweite sieht nach dem Warten die neue Version
# und bekommt 'konflikt'. Geschrieben wird in eine temporaere Datei im selben Ordner (fsync), dann os.replace und
# fsync des Ordners. Ein Verweis als Nutzerdatei bleibt erhalten (geschrieben wird an sein Ziel). Eine ungueltige
# Nutzerdatei wird nie ueberschrieben ('datei_ungueltig').
#
# KEIN WERT IN AUSGABEN (decision 2026-10-27). Ein Einstellungswert steht nie im Klartext in einer Meldung, einem Protokoll,
# Audit oder Ereignis: die Meldungen von setze() nennen Schluessel, Typ und Grund (wert_fehler nennt die erlaubte Wahl
# aus dem Schema, nie den Wert). Schreibt ein Aufrufer eine Protokoll- oder Audit-Zeile, traegt sie je Schluessel nur
# wert_merkmal() (sha256 und Laenge des Werts als JSON). Nur die Antwort an den Aufrufer enthaelt den Wert.
SPERRE_WARTEN_S = 5.0
SPERRE_NAME = '.werkbank.json.sperre'


class SetzeAbgelehnt(Exception):
    """setze() hat nicht geschrieben. code: unbekannt, nicht_nutzer, gesperrt, ungueltig, konflikt, datei_ungueltig,
    sperre_belegt, schreibfehler. version: die aktuelle Version, wenn sie bekannt ist."""

    def __init__(self, code, meldung, version=None):
        super().__init__(meldung)
        self.code = code
        self.meldung = meldung
        self.version = version


def wert_merkmal(wert):
    """{'sha256', 'laenge'} des Werts als kanonisches JSON: was eine Protokoll- oder Audit-Zeile statt des Werts traegt."""
    import hashlib
    text = json.dumps(wert, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8')
    return {'sha256': hashlib.sha256(text).hexdigest(), 'laenge': len(text)}


def schreib_fehler(zeile):
    """Text, wenn setze() diesen Schluessel nie schreibt (Klasse, nurSystemebene, ohneNutzerebene), sonst None."""
    name = zeile['schluessel']
    if zeile['klasse'] != 'nutzer':
        return "'%s' ist kein Nutzerschluessel (klasse %s); er wird nicht ueber die Nutzerebene gesetzt" % (name, zeile['klasse'])
    if zeile.get('nurSystemebene') in (True, 'mandant'):
        return "'%s' traegt nurSystemebene; er wird nicht ueber die Nutzerebene gesetzt" % name
    if zeile.get('ohneNutzerebene'):
        return "'%s' traegt ohneNutzerebene; er wird nicht ueber die Nutzerebene gesetzt" % name
    return None


def version_von(schluessel, roh):
    """Undurchsichtige Version eines gespeicherten Rohwerts (_FEHLT: der Schluessel steht nicht in der Datei)."""
    import hashlib
    inhalt = {'schluessel': schluessel, 'fehlt': True} if roh is _FEHLT else {'schluessel': schluessel, 'wert': roh}
    text = json.dumps(inhalt, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
    return 'v1-' + hashlib.sha256(text.encode('utf-8')).hexdigest()


def _nutzer_roh(pfad, schema):
    """Inhalt der Nutzerdatei ohne 'schema' (fehlt sie: {}). SetzeAbgelehnt 'datei_ungueltig', wenn sie nicht taugt."""
    try:
        st = os.stat(pfad)
    except FileNotFoundError:
        return {}
    except OSError as e:
        raise SetzeAbgelehnt('datei_ungueltig', 'Nutzerdatei %s nicht lesbar: %s' % (pfad, e.strerror))
    if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_BYTES:
        raise SetzeAbgelehnt('datei_ungueltig', 'Nutzerdatei %s ist keine regulaere Datei bis %d Bytes' % (pfad, MAX_BYTES))
    try:
        with open(pfad, 'r', encoding='utf-8') as f:
            d = json.load(f, object_pairs_hook=_paare)
    except OSError as e:
        raise SetzeAbgelehnt('datei_ungueltig', 'Nutzerdatei %s nicht lesbar: %s' % (pfad, e.strerror or type(e).__name__))
    except (ValueError, UnicodeDecodeError) as e:
        raise SetzeAbgelehnt('datei_ungueltig', 'Nutzerdatei %s ist kein gueltiges JSON (%s)'
                             % (pfad, str(e).splitlines()[0] if str(e) else type(e).__name__))
    if not isinstance(d, dict) or d.get('schema') != SCHEMA:
        raise SetzeAbgelehnt('datei_ungueltig', "Nutzerdatei %s: schema muss '%s' sein" % (pfad, SCHEMA))
    daten = {k: v for k, v in d.items() if k != 'schema'}
    f = _pruefe(daten, schema)
    if f:
        raise SetzeAbgelehnt('datei_ungueltig', 'Nutzerdatei %s: %s' % (pfad, f))
    return daten


def nutzer_versionen(k):
    """{Schluessel: Version} fuer jede Schemazeile, gemessen an der Nutzerdatei, die setze() schreibt; {} wenn sie
    nicht taugt oder es keine gibt (dann schreibt setze() ohnehin nicht)."""
    pfad = k.nutzerdatei['nutzer'].pfad
    if pfad is None or not os.path.isabs(pfad):
        return {}
    try:
        daten = _nutzer_roh(os.path.realpath(pfad), k.schema)
    except SetzeAbgelehnt:
        return {}
    return {name: version_von(name, _holen(daten, segmente(name, k.schema))) for name in k.schema.zeilen}


def schreibbar(k, name):
    """Ob setze() den Schluessel heute annehmen koennte (Klasse und Sperre; Wert und Version nicht geprueft)."""
    z = k.schema.zeilen.get(name)
    return z is not None and schreib_fehler(z) is None and not k.ist_gesperrt(segmente(name, k.schema))


def _sperre(ordner, warten):
    import fcntl
    import time
    fd = os.open(os.path.join(ordner, SPERRE_NAME), os.O_RDWR | os.O_CREAT, 0o600)
    ende = time.monotonic() + warten
    while True:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return fd
        except BlockingIOError:
            if time.monotonic() >= ende:
                os.close(fd)
                raise SetzeAbgelehnt('sperre_belegt', 'die Nutzerdatei ist seit %.1f s von einem anderen Schreiber gesperrt' % warten)
            time.sleep(0.02)


# --- Schreibmodus der Nutzerdatei (28.10.2026) ------------------------------------------------------------------------
# Der Installer (deploy/arbeitsplatz, Linux) gibt dem Dienstkonto der Entscheidungsstelle Lesezugriff auf werkbank.json
# und datenklasse.json der Person: ein ACL-Eintrag user:_wbstelle:r-- und eine Default-ACL r-- auf dem Ordner. Wirksam
# ist der Eintrag nur, solange das Gruppenlesebit (die ACL-Maske) gesetzt ist. Frueher schrieb dieser Weg neue Dateien
# immer mit 0600 (Maske ---); der Lesezugang war wirkungslos. Regeln:
#   1. 0640 nur, wo die Gruppe der Datei die PRIVATE Gruppe der Person ist (Linux, Gruppe heisst wie die Person, ist ihre
#      Hauptgruppe und hat kein anderes Mitglied; _private_gruppe). Sonst 0600 (macOS: Gruppe meist staff).
#   2. Eine bestehende Datei: Modus und der ACL-Eintrag fuer das Dienstkonto werden uebernommen, wenn Punkt 1 fuer sie
#      gilt. Eine neue Datei: 0640 nur, wenn der Ordner eine Default-ACL fuer das Dienstkonto traegt. Sonst 0600.
#   3. Hoechstens 0640: Bits fuer "andere" und das Gruppenschreibbit fallen IMMER weg. Gruppenlesen bleibt nur, wenn ein
#      Eintrag fuer das Dienstkonto es traegt. Von der ACL einer alten Datei kommt NUR der Eintrag des Dienstkontos
#      (hoechstens r--) mit; fremde Eintraege, auch geerbte aus einer Default-ACL, werden nicht uebernommen.
# POSIX-ACLs liest und schreibt dieser Weg als Attribut system.posix_acl_access (os.getxattr/os.setxattr, nur Linux, ohne
# setfacl und ohne root). Wo das fehlt (macOS, Dateisystem ohne ACL), gilt 0600.
DIENSTKONTO = '_wbstelle'      # wie KONTO in deploy/arbeitsplatz/wb_arbeitsplatz/orte.py
_PLATTFORM = sys.platform      # Tests schreiben einen anderen Wert hinein
_ACL_ZUGRIFF = 'system.posix_acl_access'
_ACL_STANDARD = 'system.posix_acl_default'
_ACL_USER_OBJ, _ACL_USER, _ACL_GROUP_OBJ, _ACL_MASK, _ACL_OTHER = 0x01, 0x02, 0x04, 0x10, 0x20
_ACL_OHNE_ID = 0xFFFFFFFF
_ACL_LESEN = 0o4


def _dienst_uid():
    """uid des Dienstkontos laut Benutzerdatenbank, None ohne Konto."""
    if pwd is None:
        return None
    try:
        return pwd.getpwnam(DIENSTKONTO).pw_uid
    except KeyError:
        return None


def _private_gruppe(uid, gid):
    """Ob `gid` die private Gruppe der Person `uid` ist: Linux, Gruppenname gleich Kennung, Hauptgruppe der Person, kein
    anderes Mitglied (weder in der Mitgliederliste noch als Hauptgruppe eines anderen Kontos). Ohne root lesbar.
    Grenze: Ein Verzeichnisdienst, der `getpwall` nicht aufzaehlt, verbirgt fremde Hauptgruppen-Mitglieder."""
    if not _PLATTFORM.startswith('linux') or pwd is None or grp is None:
        return False
    try:
        p = pwd.getpwuid(uid)
        g = grp.getgrgid(gid)
    except KeyError:
        return False
    if g.gr_gid != p.pw_gid or g.gr_name != p.pw_name:
        return False
    if any(m != p.pw_name for m in g.gr_mem):
        return False
    return all(q.pw_uid == uid for q in pwd.getpwall() if q.pw_gid == gid)


def _acl_lesen(pfad, standard=False):
    """[(tag, rechte, id)] der POSIX-ACL von `pfad`; None ohne ACL, ohne Unterstuetzung oder bei unlesbarem Attribut."""
    lies = getattr(os, 'getxattr', None)
    if lies is None:
        return None
    try:
        roh = lies(pfad, _ACL_STANDARD if standard else _ACL_ZUGRIFF, follow_symlinks=False)
    except OSError:
        return None
    if len(roh) < 4 or (len(roh) - 4) % 8 or struct.unpack('<I', roh[:4])[0] != 2:
        return None
    return [struct.unpack('<HHI', roh[i:i + 8]) for i in range(4, len(roh), 8)]


def _acl_dienst_rechte(eintraege, uid):
    """Rechte (Bits) des Eintrags fuer `uid` in der Liste, None ohne solchen Eintrag."""
    for tag, rechte, kennung in eintraege or ():
        if tag == _ACL_USER and kennung == uid:
            return rechte
    return None


def _acl_nur_dienst(pfad, uid):
    """Setzt auf `pfad` genau diese ACL: Eigentuemer rw-, `uid` r--, Gruppe ---, Maske r--, andere ---. Der Aufrufer
    setzt danach den Modus (chmod stellt die Maske aus den Gruppenbits)."""
    eintraege = [(_ACL_USER_OBJ, 0o6, _ACL_OHNE_ID), (_ACL_USER, _ACL_LESEN, uid), (_ACL_GROUP_OBJ, 0, _ACL_OHNE_ID),
                 (_ACL_MASK, _ACL_LESEN, _ACL_OHNE_ID), (_ACL_OTHER, 0, _ACL_OHNE_ID)]
    os.setxattr(pfad, _ACL_ZUGRIFF, struct.pack('<I', 2) + b''.join(struct.pack('<HHI', *e) for e in eintraege),
                follow_symlinks=False)


def _acl_loeschen(pfad):
    try:
        os.removexattr(pfad, _ACL_ZUGRIFF, follow_symlinks=False)
    except OSError:
        pass


def _schreibplan(ordner, ziel, st_neu):
    """(modus, dienst_eintrag): Modus der neuen Datei (0600 oder 0640) und ob sie den ACL-Eintrag des Dienstkontos
    haelt. `st_neu` ist fstat der Nebendatei: ihre Gruppe wird die der Datei. Regeln im Kommentar darueber."""
    uid = os.geteuid()
    dienst = _dienst_uid()
    if dienst is None or not _private_gruppe(uid, st_neu.st_gid):
        return 0o600, False
    standard = _acl_dienst_rechte(_acl_lesen(ordner, standard=True), dienst)
    ordner_traegt = standard is not None and bool(standard & _ACL_LESEN)
    try:
        alt = os.lstat(ziel)
    except OSError:
        alt = None
    if alt is None or not stat.S_ISREG(alt.st_mode) or alt.st_uid != uid:
        return (0o640, True) if ordner_traegt else (0o600, False)
    if not _private_gruppe(uid, alt.st_gid):
        return 0o600, False
    alt_rechte = _acl_dienst_rechte(_acl_lesen(ziel), dienst)
    eintrag = ordner_traegt or (alt_rechte is not None and bool(alt_rechte & _ACL_LESEN))
    lesen = eintrag and bool(alt.st_mode & 0o040)
    return (0o640 if lesen else 0o600), eintrag


def _modus_setzen(tmp, ordner, ziel, fd):
    """Stellt Modus und ACL der Nebendatei ein (siehe _schreibplan); jeder Fehler an der ACL laesst es bei 0600."""
    modus, eintrag = _schreibplan(ordner, ziel, os.fstat(fd))
    dienst = _dienst_uid()
    try:
        geerbt = _acl_lesen(tmp)
        if geerbt is not None or eintrag:
            if (eintrag or _acl_dienst_rechte(geerbt, dienst) is not None) and dienst is not None:
                _acl_nur_dienst(tmp, dienst)       # nur der Eintrag des Dienstkontos, nie ein geerbter fremder
            else:
                _acl_loeschen(tmp)
    except OSError:
        modus = 0o600
    os.chmod(tmp, modus)


def schreibe_einstellungsdatei(ziel, inhalt):
    """Schreibt die Nutzerdatei `ziel` unteilbar (Nebendatei im selben Ordner, os.replace). Modus und ACL: siehe oben,
    0600 oder 0640, nie weiter. Der Ordner muss da sein."""
    import tempfile
    ordner = os.path.dirname(ziel)
    fd, tmp = tempfile.mkstemp(dir=ordner, prefix='.werkbank.json.')
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as f:
            f.write(inhalt)
            f.flush()
            os.fsync(f.fileno())
            _modus_setzen(tmp, ordner, ziel, f.fileno())
        os.replace(tmp, ziel)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    dfd = os.open(ordner, os.O_RDONLY)
    try:
        os.fsync(dfd)
    finally:
        os.close(dfd)


_schreibe_atomar = schreibe_einstellungsdatei


def setze(schluessel, wert, version, env=None, heim=None, schema=None, warten=SPERRE_WARTEN_S):
    """Setzt einen Nutzerschluessel in der Nutzerdatei, wenn `version` die aktuelle ist. Rueckgabe
    {'schluessel', 'version' (neu), 'geaendert' (False bei gleichem Wert, dann wird nicht geschrieben), 'pfad'}.
    SchemaUngueltig bei kaputter Schemadatei, SetzeAbgelehnt sonst (siehe dort)."""
    schema = schema or lade_schema()
    z = schema.zeilen.get(schluessel)
    if z is None:
        raise SetzeAbgelehnt('unbekannt', "'%s' steht nicht im Schema" % schluessel)
    if isinstance(wert, str) and wert in (z.get('nurSystemebeneWerte') or ()):
        raise SetzeAbgelehnt('nicht_nutzer', "'%s': dieser Wert gilt nur aus der Systemdatei %s; ein Administrator setzt ihn dort "
                                             "(mit sudo)" % (schluessel, _system_pfad() or '(nur Linux und macOS)'))
    f = schreib_fehler(z)
    if f:
        raise SetzeAbgelehnt('nicht_nutzer', f)
    f = wert_fehler(z, wert)
    if f:
        raise SetzeAbgelehnt('ungueltig', f)
    k = Konfig(env, schema=schema, heim=heim)
    segs = segmente(schluessel, schema)
    if k.ist_gesperrt(segs):
        raise SetzeAbgelehnt('nicht_nutzer', "'%s' ist durch die Systemdatei gesperrt" % schluessel)
    grenze = k.untergrenzen.get(schluessel)
    if grenze and z.get('richtung') not in (None, 'keine') and lockerer(z, wert, grenze[0]):
        raise SetzeAbgelehnt('ungueltig', "'%s': der Wert ist lockerer als die Untergrenze aus %s" % (schluessel, grenze[1]))
    nutzer = k.nutzerdatei['nutzer']
    if nutzer.pfad is None or not os.path.isabs(nutzer.pfad):
        raise SetzeAbgelehnt('datei_ungueltig', 'keine Nutzerdatei (%s)' % (nutzer.fehler or nutzer.hinweis or 'ohne Pfad'))
    ziel = os.path.realpath(nutzer.pfad)
    ordner = os.path.dirname(ziel)
    try:
        os.makedirs(ordner, mode=0o700, exist_ok=True)
        fd = _sperre(ordner, warten)
    except OSError as e:
        raise SetzeAbgelehnt('schreibfehler', 'Ordner %s: %s' % (ordner, e.strerror or type(e).__name__))
    try:
        daten = _nutzer_roh(ziel, schema)
        alt = _holen(daten, segs)
        aktuell = version_von(schluessel, alt)
        if version != aktuell:
            raise SetzeAbgelehnt('konflikt', "'%s' wurde inzwischen geaendert" % schluessel, aktuell)
        neu = version_von(schluessel, wert)
        if alt is not _FEHLT and json.dumps(alt, sort_keys=True) == json.dumps(wert, sort_keys=True):
            return {'schluessel': schluessel, 'version': neu, 'geaendert': False, 'pfad': ziel}
        unten = daten
        for sg in segs[:-1]:
            unten = unten.get(sg, {})
            if not isinstance(unten, dict):
                raise SetzeAbgelehnt('datei_ungueltig', "Nutzerdatei %s: '%s' ist kein Objekt" % (ziel, sg))
        _setze(daten, segs, _kopie(wert))
        f = _pruefe(daten, schema)
        if f:
            raise SetzeAbgelehnt('ungueltig', f)
        text = json.dumps(dict([('schema', SCHEMA)] + list(daten.items())), ensure_ascii=False, indent=2) + '\n'
        try:
            _schreibe_atomar(ziel, text)
        except OSError as e:
            raise SetzeAbgelehnt('schreibfehler', 'Nutzerdatei %s: %s' % (ziel, e.strerror or type(e).__name__), aktuell)
        return {'schluessel': schluessel, 'version': neu, 'geaendert': True, 'pfad': ziel}
    finally:
        os.close(fd)


# --- Pruefprotokolle: Rotation und Aufbewahrung (audit.aufbewahrungTage, decision 2026-11-05, Zug 07.11.2026) -------------
# Die Werkbank schreibt Pruefprotokolle als JSON-Zeilen mit O_APPEND: geheimnis-audit.jsonl (wb_geheimnis.py und die
# TS-Bindung app/src/gemeinsam/geheimnis), ausgang-audit.jsonl (wb-ausgang; wb-grenze pflegt die Datei vor jedem Start
# des Brokers) und <stelle>/zustand/audit.jsonl (wb_stelle/entscheide.py). audit_pflegen() haelt sie klein. Dieselbe
# Regel baut app/src/gemeinsam/audit-datei.ts fuer Node; beide teilen Sperre, Namen und Kriterien.
#
# ROTATION. Die laufende Datei behaelt ihren Namen. Beginnt ihr erster Eintrag ("zeit": ISO-Zeit oder Sekunden seit 1970)
# in einem frueheren Monat (UTC) als jetzt, wird sie ein Abschnitt <stamm>.<JJJJ-MM>.jsonl (Monat des ersten Eintrags;
# gibt es den Namen schon, <stamm>.<JJJJ-MM>.<n>.jsonl). Der Weg ist link() und danach unlink(), nie eine Kopie: ein
# Schreiber, der die Datei vor dem unlink() geoeffnet hat, schreibt in denselben Inode, also in den Abschnitt; der naechste
# legt die laufende Datei neu an (O_CREAT, 0600). So geht keine Zeile verloren und keine steht doppelt, auch wenn Python
# und Node gleichzeitig schreiben. Die Schreiber sperren nichts. Nur das Rotieren und das Loeschen laufen unter einer
# flock-Sperre auf <datei>.lock (0600), nicht blockierend: haelt ein anderer sie, pflegt dieser. Vor dem Rotieren wird
# unter der Sperre neu geprueft. Rechte: der Abschnitt ist derselbe Inode (0600), chmod 0600 haelt das fest.
# Rotiert wird nur eine regulaere Datei der eigenen Kennung, nie ein Verweis.
#
# AUFBEWAHRUNG. audit.aufbewahrungTage (Betreiberschluessel nurSystemebene, ganze Zahl >= 0, keine Vorgabe: im Firmenbetrieb
# eine Pflichtangabe, siehe audit_pflicht; ohne Wert gilt hier 0). 0 heisst: nichts wird geloescht, wie vor diesem Zug. Ein Wert N > 0 loescht einen Abschnitt, dessen juengster Eintrag (das
# Spaetere von mtime und dem letzten "zeit") mehr als N Tage zurueckliegt. Die laufende Datei wird nie geloescht. Geprueft
# wird bei jeder Rotation und sonst hoechstens einmal je AUDIT_PFLEGE_S (mtime der Sperrdatei). Ist der Wert nicht lesbar
# (Systemdatei ungueltig, Schema fehlt), gilt 0: im Zweifel bleibt das Protokoll.
#
# GRENZEN. Ein Abschnitt kann nach dem Rotieren noch Zeilen bekommen (ein Schreiber, der vorher geoeffnet hatte); sie tragen
# dann den neuen Monat. Liegt "zeit" im ersten Eintrag hinter AUDIT_KOPF_BYTES, rotiert die Datei nicht. Schreibt niemand,
# rotiert und loescht niemand. Ein Fehler der Pflege bricht nie das Schreiben einer Zeile ab (audit_pflegen wirft nicht).
AUDIT_SCHLUESSEL = 'audit.aufbewahrungTage'
AUDIT_EINSICHT_SCHLUESSEL = 'audit.einsichtRegel'
AUDIT_SICHTBAR_SCHLUESSEL = 'audit.einsicht_sichtbar_fuer_person'
AUDIT_RATE_SCHLUESSEL = 'audit.einsichtAbfragenJeStunde'
AUDIT_EINSICHT_REGELN = ('niemand', 'rolle', 'rolle-vier-augen')
AUDIT_KOPF_BYTES = 65536
AUDIT_PFLEGE_S = 86400
_audit_jetzt = __import__('time').time      # Tests schreiben diese Zeile um
_AUDIT_ZEIT = re.compile(rb'"zeit"\s*:\s*(?:"([^"\\]{10,40})"|(-?\d+(?:\.\d+)?)(?![\d.]))')
_AUDIT_ISO = re.compile(r'(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.\d+)?)?)?(Z|[+-]\d{2}:?\d{2})?\Z')


def _audit_zeitpunkt(roh):
    """Sekunden seit 1970 des ersten "zeit" in ROH (bytes), sonst None."""
    import calendar
    t = _AUDIT_ZEIT.search(roh)
    if not t:
        return None
    if t.group(2) is not None:
        try:
            return float(t.group(2))
        except ValueError:
            return None
    i = _AUDIT_ISO.match(t.group(1).decode('ascii', 'replace'))
    if not i:
        return None
    j, mo, ta, h, mi, s, zone = i.groups()
    try:
        sek = calendar.timegm((int(j), int(mo), int(ta), int(h or 0), int(mi or 0), int(s or 0), 0, 0, 0))
    except (ValueError, OverflowError):
        return None
    if zone and zone != 'Z':
        z = zone.replace(':', '')
        sek -= (1 if z[0] == '+' else -1) * (int(z[1:3]) * 3600 + int(z[3:5]) * 60)
    return float(sek)


def _audit_monat(sek):
    import time
    try:
        return time.strftime('%Y-%m', time.gmtime(sek))
    except (OverflowError, ValueError, OSError):
        return None


def _audit_teile(pfad):
    ordner, name = os.path.split(pfad)
    return ordner, (name[:-len('.jsonl')] if name.endswith('.jsonl') else name)


def _audit_eigen(st):
    return stat.S_ISREG(st.st_mode) and st.st_uid == os.geteuid()


def _audit_erster_monat(pfad):
    """Monat (JJJJ-MM, UTC) des ersten Eintrags der laufenden Datei; None ohne Datei, ohne lesbare Zeit oder bei fremder Datei."""
    try:
        if not _audit_eigen(os.lstat(pfad)):
            return None
        fd = os.open(pfad, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    except OSError:
        return None
    try:
        roh = os.read(fd, AUDIT_KOPF_BYTES)
    except OSError:
        return None
    finally:
        os.close(fd)
    sek = _audit_zeitpunkt(roh.split(b'\n', 1)[0])
    return None if sek is None else _audit_monat(sek)


def _audit_rotation_faellig(pfad, jetzt):
    erster, heute = _audit_erster_monat(pfad), _audit_monat(jetzt)
    return erster if erster is not None and heute is not None and erster < heute else None


def _audit_letzter_zeitpunkt(pfad):
    """Zeit des letzten Eintrags mit "zeit" in den letzten AUDIT_KOPF_BYTES von PFAD, sonst None."""
    try:
        with open(pfad, 'rb') as fh:
            fh.seek(0, os.SEEK_END)
            fh.seek(max(0, fh.tell() - AUDIT_KOPF_BYTES))
            roh = fh.read()
    except OSError:
        return None
    for zeile in reversed(roh.split(b'\n')):
        sek = _audit_zeitpunkt(zeile)
        if sek is not None:
            return sek
    return None


def audit_abschnitte(pfad):
    """[(Name, Pfad)] der abgeschlossenen Abschnitte zur laufenden Datei PFAD, sortiert nach Name."""
    ordner, stamm = _audit_teile(pfad)
    muster = re.compile(re.escape(stamm) + r'\.\d{4}-\d{2}(?:\.\d+)?\.jsonl\Z')
    try:
        namen = os.listdir(ordner or '.')
    except OSError:
        return []
    return [(n, os.path.join(ordner, n)) for n in sorted(namen) if muster.match(n)]


def _audit_rotieren(pfad, monat):
    """Unter der Sperre: PFAD wird der Abschnitt <stamm>.<MONAT>[.<n>].jsonl. -> Pfad des Abschnitts oder None."""
    import errno
    ordner, stamm = _audit_teile(pfad)
    if not _audit_eigen(os.lstat(pfad)):
        return None
    for n in range(10000):
        ziel = os.path.join(ordner, '%s.%s%s.jsonl' % (stamm, monat, '.%d' % n if n else ''))
        try:
            os.link(pfad, ziel)
        except FileExistsError:
            continue
        except OSError as e:
            # Ein Dateisystem ohne harte Verweise: umbenennen, solange der Name frei ist (nur Rotierer legen ihn an, unter der Sperre).
            if e.errno not in (errno.EPERM, errno.ENOTSUP, errno.EOPNOTSUPP, errno.EMLINK) or os.path.lexists(ziel):
                raise
            os.rename(pfad, ziel)
            os.chmod(ziel, 0o600)
            return ziel
        os.chmod(ziel, 0o600)
        os.unlink(pfad)
        return ziel
    return None


AUDIT_WAHL_DATEI = 'einsicht-wahl.json'     # die bestaetigte Wahl der Stelle (wb_stelle/einsicht.py, WAHL_DATEI)


def audit_stelle_zustand(pfad):
    """Der Zustandsordner der Stelle, wenn PFAD ihr Protokoll <ordner>/zustand/audit.jsonl ist, sonst None."""
    pfad = os.path.abspath(pfad)
    ordner = os.path.dirname(pfad)
    if os.path.basename(pfad) == 'audit.jsonl' and os.path.basename(ordner) == 'zustand':
        return ordner
    return None


def audit_aufbewahrung_bestaetigt(zustand):
    """Die Frist, nach der die Stelle ihr Protokoll in ZUSTAND loeschen darf (Regel: nur nach der BESTAETIGTEN Frist).
    Am Einzelplatz (richtlinie/stelle.json nennt nicht den Servermodus, oder sie fehlt und betriebsart() ist 'einzelplatz') gilt die
    Systemdatei wie bisher. Fehlt stelle.json und ist die Betriebsart 'mandant' (Systemschluessel, Marke, ungueltige Systemdatei), gilt 0. Im Servermodus gilt allein
    die bestaetigte Wahl in <zustand>/einsicht-wahl.json (Feld aufbewahrung): fehlt sie, ist sie nicht lesbar oder ungueltig, oder ist die
    stelle.json nicht lesbar, so gilt 0 (nichts loeschen). Weicht die Systemdatei ab, gilt weiter die bestaetigte Frist, bis eine neue Wahl
    bestaetigt ist (auch bei einer Kuerzung). Wirft nie. -> ganze Zahl >= 0."""
    try:
        wurzel = os.path.dirname(os.path.abspath(zustand))
        try:
            with open(os.path.join(wurzel, 'richtlinie', 'stelle.json'), encoding='utf-8') as fh:
                cfg = json.load(fh)
        except FileNotFoundError:
            # w150: ohne stelle.json zaehlt das Zeichen des Firmenbetriebs (betriebsart(): Systemschluessel, Marke, ungueltige Systemdatei):
            # 'mandant' loescht nichts, nur ein sicher gelesener 'einzelplatz' nimmt die Systemdatei
            return audit_aufbewahrung_tage() if betriebsart() == 'einzelplatz' else 0
        if not isinstance(cfg, dict) or cfg.get('modus') != 'server':
            return audit_aufbewahrung_tage()
        with open(os.path.join(zustand, AUDIT_WAHL_DATEI), encoding='utf-8') as fh:
            w = json.load(fh)
        t = w.get('aufbewahrung')
        if (w.get('regel') in AUDIT_EINSICHT_REGELN and isinstance(w.get('sichtbar'), bool)
                and isinstance(t, int) and not isinstance(t, bool) and 0 <= t <= 36500):
            return t
    except Exception:
        pass
    return 0


def audit_aufbewahrung_tage(konfig=None, zustand=None, **lade_args):
    """audit.aufbewahrungTage als ganze Zahl >= 0; jeder Fehler beim Lesen ergibt 0 (nichts loeschen). Mit ZUSTAND (dem Zustandsordner einer
    Stelle) gilt die Frist der Stelle nach audit_aufbewahrung_bestaetigt."""
    if zustand is not None:
        return audit_aufbewahrung_bestaetigt(zustand)
    try:
        w = (konfig or lade(**lade_args)).hole(AUDIT_SCHLUESSEL, 0)
    except Exception:
        return 0
    return w if isinstance(w, int) and not isinstance(w, bool) and w >= 0 else 0


def audit_pflicht(konfig=None, **lade_args):
    """Die beiden Pflichtangaben des Firmenbetriebs (R-AUDIT-PFLICHT der Entscheidungsstelle): audit.aufbewahrungTage und
    audit.einsichtRegel haben keine Vorgabe, nur die Systemdatei setzt sie. -> {schluessel: wert | None}, None heisst: fehlt,
    ist ungueltig oder ist nicht lesbar (Systemdatei oder Schema kaputt). Wirft nie; im Zweifel fehlt der Wert."""
    ergebnis = {AUDIT_SCHLUESSEL: None, AUDIT_EINSICHT_SCHLUESSEL: None}
    try:
        k = konfig or lade(**lade_args)
    except Exception:
        return ergebnis
    for name in ergebnis:
        try:
            # der Leser verwirft einen Systemwert, der nicht zur Zeile passt (Typ, Bereich, Wahl), schon beim Laden: er gilt dann als nicht gesetzt
            w, _ = k.wirksam(name)
            if w is not _FEHLT:
                ergebnis[name] = w
        except Exception:
            pass
    return ergebnis


def audit_einsicht(konfig=None, **lade_args):
    """Die Werte der Einsicht in die Pruefprotokolle (task, R-EINSICHT-*): -> {'regel': 'niemand'|'rolle'|'rolle-vier-augen'|None,
    'aufbewahrung': int|None, 'sichtbar': bool, 'rate': int}. regel und aufbewahrung sind None, wenn die Firma nicht gewaehlt hat
    (kein Wert, ungueltig, Systemdatei oder Schema nicht lesbar); sichtbar gilt ohne lesbaren Wert als True (die Person erfaehrt
    es), rate als 30. Wirft nie; im Zweifel liest niemand."""
    ergebnis = {'regel': None, 'aufbewahrung': None, 'sichtbar': True, 'rate': 30}
    try:
        k = konfig or lade(**lade_args)
    except Exception:
        return ergebnis
    def wert(name):
        try:
            w, _ = k.wirksam(name)
            return None if w is _FEHLT else w
        except Exception:
            return None
    regel = wert(AUDIT_EINSICHT_SCHLUESSEL)
    ergebnis['regel'] = regel if regel in AUDIT_EINSICHT_REGELN else None
    tage = wert(AUDIT_SCHLUESSEL)
    ergebnis['aufbewahrung'] = tage if isinstance(tage, int) and not isinstance(tage, bool) and 0 <= tage <= 36500 else None
    sichtbar = wert(AUDIT_SICHTBAR_SCHLUESSEL)
    ergebnis['sichtbar'] = sichtbar if isinstance(sichtbar, bool) else True
    rate = wert(AUDIT_RATE_SCHLUESSEL)
    ergebnis['rate'] = rate if isinstance(rate, int) and not isinstance(rate, bool) and 1 <= rate <= 600 else 30
    return ergebnis


def _audit_loeschen(pfad, tage, jetzt):
    """Abschnitte von PFAD loeschen, deren juengster Eintrag mehr als TAGE Tage vor JETZT liegt. -> [Name]."""
    weg = []
    grenze = jetzt - tage * 86400
    for name, voll in audit_abschnitte(pfad):
        try:
            st = os.lstat(voll)
        except OSError:
            continue
        if not _audit_eigen(st):
            continue
        juengst = max(st.st_mtime, _audit_letzter_zeitpunkt(voll) or 0.0)
        if juengst < grenze:
            try:
                os.unlink(voll)
                weg.append(name)
            except OSError:
                pass
    return weg


def audit_pflegen(pfad, tage=None, jetzt=None):
    """Rotation und Aufbewahrung der laufenden Datei PFAD (siehe oben). TAGE None liest audit.aufbewahrungTage, erst wenn
    geloescht werden koennte. -> {'abschnitt': Pfad|None, 'geloescht': [Name]}. Wirft nie."""
    ergebnis = {'abschnitt': None, 'geloescht': []}
    try:
        import fcntl
        jetzt = _audit_jetzt() if jetzt is None else jetzt
        sperre = pfad + '.lock'
        try:
            st = os.stat(sperre)
            loeschen = abs(jetzt - st.st_mtime) >= AUDIT_PFLEGE_S
        except FileNotFoundError:
            loeschen = True
        if not loeschen and _audit_rotation_faellig(pfad, jetzt) is None:
            return ergebnis
        fd = os.open(sperre, os.O_RDWR | os.O_CREAT | getattr(os, 'O_NOFOLLOW', 0), 0o600)
        try:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                return ergebnis
            monat = _audit_rotation_faellig(pfad, jetzt)
            if monat is not None:
                ergebnis['abschnitt'] = _audit_rotieren(pfad, monat)
            if loeschen or ergebnis['abschnitt']:
                if tage is None:
                    zustand = audit_stelle_zustand(pfad)
                    tage = audit_aufbewahrung_tage(zustand=zustand) if zustand else audit_aufbewahrung_tage()
                if tage > 0:
                    ergebnis['geloescht'] = _audit_loeschen(pfad, tage, jetzt)
                os.utime(sperre, (jetzt, jetzt))
        finally:
            os.close(fd)
    except Exception:
        pass
    return ergebnis
