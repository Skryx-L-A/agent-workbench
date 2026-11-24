#!/usr/bin/env -S -u DEVELOPER_DIR -u SDKROOT -u TOOLCHAINS /usr/bin/python3 -E -s -S
"""wb_naht_ziel — ist das ZIEL eines tmux-Aufrufs der echte Werkbank-Server? (23.10.2026)

DIE REGEL (22.10.2026): Unter der Testnaht WB_TESTUMGEBUNG=1 gilt eine
Lockerung (tmux aus dem PATH, Testheimat) nur, wenn das ZIEL nicht der echte Werkbank-Server ist.
Schreibende Wege auf den echten Server enden unter der Naht mit Exit 78 (NAHT_ECHT_ABGELEHNT);
lesende laufen wie ohne Naht. Massgeblich ist das Ziel, nicht die Ahnenreihe: Suiten laufen aus
Werkbank-Panes und erben TMUX.

ZIEL. Der Server aus -S, sonst aus -L (die Werkzeuge reichen WB_TMUX_SOCKET als -L herein, so wie
sie es an tmux geben), sonst aus TMUX (erstes Feld), sonst der Standardsocket der Kennung
(${TMUX_TMPDIR:-/tmp}/tmux-<uid>/default). Das ist dieselbe Wahl, die tmux selbst trifft. Die
Werkzeuge geben genau die Serverargumente herein, die sie auch tmux geben; ein Schirm im PATH,
der selbst -L anhaengt, ist hier unsichtbar. Suiten mit so einem Schirm setzen deshalb TMUX auf
ihren eigenen Socket oder geben -L/WB_TMUX_SOCKET.

DER ECHTE SERVER kommt NUR aus diesem Modul, in dieser Reihenfolge:
  1. tmuxSocket aus der root-eigenen Systemebene (SYSTEM_PFADE). Gelesen ueber denselben Pruefweg
     wie wb_konfig._lies_datei(art='system'): regulaere Datei, nicht gruppen- oder weltschreibbar,
     Eigentuemer in SYSTEM_EIGENTUEMER, die ganze Ordnerkette bis SYSTEM_KETTE_BIS ebenso,
     schema 'wb-konfig/1'. Eine ungueltige Systemdatei zaehlt als nicht da.
  2. sonst AUSGEROLLTER_SOCKET: wb-ausrollen schreibt beim Ausrollen den tmuxSocket des
     Betreibers in diese eine Zeile des INSTALLIERTEN Moduls.
  3. sonst /tmp/tmux-<uid>/default.
Der Standardsocket /tmp/tmux-<uid>/default zaehlt immer zusaetzlich als echt (strengste Lesart,
wie wb-decide werkbank_sockets). NIE aus ~/.claude/workbench/config.json oder
~/.config/agent-workbench, NIE aus der Umgebung.

Verglichen wird per Geraet und Inode (os.path.samefile), dazu der aufgeloeste Pfad: ein echter
Socket, den es noch nicht gibt, bleibt echt (tmux legte dort sonst einen Server an).

FAIL CLOSED. Laesst sich das Ziel nicht bestimmen, gilt es als echt. Fehlt dieses Modul, gilt die
Naht in den Werkzeugen NICHT (sie behandeln das Ziel als echt).

GRENZE TMUX_TMPDIR: ziel() beachtet TMUX_TMPDIR, weil tmux es fuer -L und den Standardsocket nimmt.
Die echten Sockets (Standard, Name aus der Systemebene, wb-ausrollen) liegen dagegen fest unter
/tmp/tmux-<uid>/, denn die Umgebung darf den echten Server nicht bestimmen. Laeuft die Werkbank unter
einem anderen TMUX_TMPDIR, braucht die Systemebene tmuxSocket als absoluten Pfad; sonst erkennt die
Regel den echten Server nicht. Ein -L-Label mit "/" gilt als echt (fail closed).

GRENZE, offen benannt: AUSGEROLLTER_SOCKET ist nur so stark wie der Text-Guard ueber ~/.local/bin
(werkzeug-pfad schuetzt wb_*). Wer das installierte Modul umschreiben kann, kann den echten Server
umbenennen; die Systemebene ist root-eigen und davon frei.

Tests schreiben die Konstanten SYSTEM_PFADE, SYSTEM_EIGENTUEMER, SYSTEM_KETTE_BIS und
AUSGEROLLTER_SOCKET (je eine Zeile) in einer KOPIE um, nie ueber eine Umgebungsvariable.

Kommandozeile (fuer Bash-Werkzeuge):
  wb_naht_ziel.py lockert [tmux-serverargumente]   Exit 0: Naht gesetzt und Ziel nicht echt.
  wb_naht_ziel.py echt    [tmux-serverargumente]   Exit 0: Ziel ist der echte Server.
  wb_naht_ziel.py sockets                          Die echten Sockets, einer je Zeile.
  wb_naht_ziel.py aktiv                            Exit 0: die Naht wirkt (naht_aktiv), sonst 1.
  wb_naht_ziel.py gesetzt                          Exit 0: WB_TESTUMGEBUNG=1 ist gesetzt (naht_gesetzt), auch
                                                   mit NAHT_ERLAUBT = False; sonst 1. Nur zum Verschaerfen.
  wb_naht_ziel.py variable NAME [WERKZEUG]         Den Wert von WB_MODELS_FILE oder AWB_SETTINGS_FILE
                                                   (nur unter der Naht), sonst nichts. Siehe unten.
  wb_naht_ziel.py schreiben WERKZEUG [--heim PFAD] [--ziel DATEI ...]
                                                   Exit 0: darf schreiben; 79: abgelehnt (siehe unten).
  wb_naht_ziel.py schreibsperre [--heim PFAD] [--ziel DATEI ...]
                                                   Die sperrenden Namen kommagetrennt, leer wenn frei.
Exit 1 heisst nein, Exit 2 Aufruffehler. `variable` und `schreibsperre` enden immer mit Exit 0.

DIE NAHT-VARIABLEN (decision 2026-10-23, task). WB_MODELS_FILE (Registry), AWB_MODELS_FILE (dieselbe Registry, so
heisst sie in der App; seit 30.10.2026, Stufe 2c R2) und AWB_SETTINGS_FILE (Einstellungen) lenken eine Datei der Heimat um. Seit task lesen wb-state und das Datenklassen-Tor
beide NUR unter der Testnaht WB_TESTUMGEBUNG=1; ohne Naht gilt die Datei der Heimat. Jedes andere Werkzeug
unter shell/ und deploy/, das eine der beiden Variablen liest, folgt derselben Regel durch naht_variable()
(Python) oder `variable` (Bash): unter der Naht gilt der Wert wie bisher; ohne Naht meldet das Werkzeug die
gesetzte Variable LAUT (eine Zeile auf stderr, nur der Name, nie der Wert) und ignoriert sie. Eine leere
Variable zaehlt als nicht gesetzt. Fehlt dieses Modul neben dem Werkzeug, gilt die Naht nicht (fail closed):
die Variable wird ignoriert und gemeldet. test-naht-variable-leser.py findet jeden Leser ohne diese Stelle.

SCHREIBEN OHNE NAHT (decision 2026-10-23, task). Ignorieren reicht nur fuers Lesen: ein Testlauf, der
WB_MODELS_FILE, AWB_SETTINGS_FILE oder ein umgelenktes HOME setzt und WB_TESTUMGEBUNG=1 vergisst, schrieb sonst
in die ECHTE Registry bzw. settings.json der Heimat (gemessen 23.10.). Deshalb fragt jedes Werkzeug, das
models.json oder settings.json der Heimat schreibt, vor dem Schreiben schreib_leck() bzw. `schreiben`:
  - ohne Naht ist eine der beiden Naht-Variablen gesetzt (nicht leer), oder
  - ohne Naht ist HOME gesetzt und zeigt nicht auf die Heimat laut Benutzerdatenbank (HEIM_DB), oder
  - unter der Naht ist das Ziel (--ziel) die echte models.json/settings.json der Heimat (HOME nicht
    umgelenkt; Name WB_TESTUMGEBUNG), oder
  - WB_SCHREIBSPERRE traegt einen dieser Namen (ein Werkzeug, das HOME und die Variablen fuer seine Kinder
    bereinigt, gibt den Befund so weiter; wb-state setzt sie im Kopf).
Dann endet der Schreibweg mit SCHREIB_ABBRUCH (79) und einer Zeile mit dem NAMEN, nie dem Wert. Die Datei
bleibt unveraendert. Fehlt dieses Modul neben dem Werkzeug, bricht das Schreiben in diesen Faellen ebenfalls
ab (fail closed). Lesen meldet weiter nur (naht_variable). Unter der Naht gilt HOME als Testheimat.
Tests schreiben HEIM_DB (eine Zeile) in einer KOPIE um; wb-state gibt seine eigene Heimat-Zeile mit --heim mit.

DIE SCHALTERZEILE (Stufe 2c, Zug Z0, 25.10.2026). Die Naht wirkt nur, wenn NAHT_ERLAUBT = True im Modul steht:
naht_aktiv() = NAHT_ERLAUBT und WB_TESTUMGEBUNG=1. Das Repo traegt True; das Ausrollen schreibt False ein
(Umschreibpunkt wie AUSGEROLLTER_SOCKET, genau eine Zeile), damit das installierte Bild keine wirksame Naht hat.
Jede LOCKERUNG fragt naht_aktiv(): naht_lockert, naht_variable, schreiben/schreibsperre (HOME und die
Naht-Variablen sperren, wenn die Naht nicht wirkt). Die SPERREN bleiben am Signal WB_TESTUMGEBUNG=1 selbst:
naht_sperrt (Exit 78 auf dem echten Server) und die echte Datei der Heimat als Schreibziel. Unter False ist ein
Lauf mit WB_TESTUMGEBUNG=1 also strenger als einer ohne, nie lockerer. Fehlt das Modul, wirkt die Naht nicht.
hooks/lib/rollen.py und hooks/lib/naht_variable.py tragen dieselbe Zeile und ein privates _naht_aktiv();
Abschnitt P (Schalter) in test-naht-ziel-modul.py haelt die drei gleich.

DIE DEV-FASSUNG (Stufe 2c, R4, 31.10.2026). wb-dev setzt AWB_DEV=1 statt der Marke und lenkt seine Pfade ueber dieselben
Variablen um (app/src/main/naht.ts, DIE DEV-FASSUNG). naht_variable nimmt AWB_DEV=1 deshalb wie die App an, nur bei erlaubter
Naht. naht_aktiv bleibt unveraendert: AWB_DEV oeffnet keine Lockerung, kein Schreiben und keinen tmux-Server.
hooks/lib/naht_variable.py nimmt AWB_DEV bewusst nicht an: Ein Hook entscheidet ueber Guard-Schalter, die Umgebung nicht.
"""
import json
import os
import stat
import sys

NAHT_ECHT_ABGELEHNT = 78

SYSTEM_PFADE = {'linux': '/etc/agent-workbench/werkbank.json', 'darwin': '/Library/Application Support/agent-workbench/werkbank.json'}
SYSTEM_EIGENTUEMER = (0,)
SYSTEM_KETTE_BIS = '/'
SCHEMA = 'wb-konfig/1'
MAX_BYTES = 1 << 20
AUSGEROLLTER_SOCKET = ''   # wb-ausrollen schreibt hier beim Ausrollen den Socket des Betreibers ein
NAHT_ERLAUBT = True   # Schalterzeile (Stufe 2c, Z0): das Ausrollen schreibt False ein; Tests nur in einer KOPIE

NAHT_VARIABLEN = ('WB_MODELS_FILE', 'AWB_MODELS_FILE', 'AWB_SETTINGS_FILE', 'AWB_GUARD_GRANTS_DIR', 'AWB_ROLLEN_DIR',
                  'AWB_WB_IDENTITAET', 'AWB_REMOTE_MACHINES', 'AWB_CONFIG', 'AWB_STATE_DIR', 'AWB_WB_STATE',
                  'AWB_WB_REVIVE', 'AWB_WELTEN_FERN', 'WB_REPLIK_SSH', 'AWB_GUARD_LOG', 'AWB_GUARD_BLOCKS_DIR')

SCHREIB_ABBRUCH = 79
SPERR_NAMEN = ('HOME',) + NAHT_VARIABLEN + ('WB_TESTUMGEBUNG',)
HEIM_DB = ''   # leer: Heimat laut Benutzerdatenbank (getpwuid); Tests schreiben diese Zeile in einer KOPIE um

NAME_RE_ZEICHEN = set('ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-')


def naht_aktiv(umgebung=None):
    """True: die Schalterzeile erlaubt die Naht UND WB_TESTUMGEBUNG=1 (Vorgabe os.environ). Jede Lockerung fragt das."""
    umgebung = os.environ if umgebung is None else umgebung
    return NAHT_ERLAUBT is True and umgebung.get('WB_TESTUMGEBUNG') == '1'


def naht_gesetzt(umgebung=None):
    """True: WB_TESTUMGEBUNG=1 ist gesetzt, unabhaengig von NAHT_ERLAUBT (Vorgabe os.environ). Nur fuer Sperren:
    wer das fragt, wird damit strenger, nie lockerer; jede Lockerung fragt naht_aktiv()."""
    umgebung = os.environ if umgebung is None else umgebung
    return umgebung.get('WB_TESTUMGEBUNG') == '1'


def _dev_fassung(umgebung):
    """True: die Dev-Fassung (wb-dev) bei erlaubter Naht. Oeffnet nur naht_variable, nicht naht_aktiv."""
    return NAHT_ERLAUBT is True and umgebung.get('AWB_DEV') == '1'


def _standard():
    return '/tmp/tmux-%d/default' % os.getuid()


def _socket_aus_wert(wert):
    """tmuxSocket ist ein schlichter Name (unter /tmp/tmux-<uid>/) oder ein absoluter Pfad."""
    if not isinstance(wert, str) or not wert:
        return None
    if os.path.isabs(wert):
        return wert
    if len(wert) <= 64 and set(wert) <= NAME_RE_ZEICHEN and wert not in ('.', '..'):
        return '/tmp/tmux-%d/%s' % (os.getuid(), wert)
    return None


def _kette_ok(pfad):
    """Wie wb_konfig._kette_fehler: jeder Ordner bis SYSTEM_KETTE_BIS gehoert SYSTEM_EIGENTUEMER
    und ist nicht gruppen- oder weltschreibbar."""
    d = os.path.dirname(os.path.realpath(pfad))
    bis = os.path.realpath(SYSTEM_KETTE_BIS)
    while True:
        try:
            st = os.stat(d)
        except OSError:
            return False
        if st.st_mode & 0o022 or st.st_uid not in SYSTEM_EIGENTUEMER:
            return False
        if d == bis or d == os.path.dirname(d):
            return True
        d = os.path.dirname(d)


def system_socket(plattform=None):
    """tmuxSocket aus der gueltigen Systemdatei, sonst None. Ungueltig wie in wb_konfig (Variante C, 23.10.2026):
    Rechte, Eigentuemer, Ordnerkette, JSON samt doppeltem Schluessel, schema und eine kaputte Zeile gesperrt.
    Ein Typfehler in einer anderen Zeile verwirft dort nur jene Zeile; tmuxSocket gilt weiter."""
    def paare(liste):
        d = {}
        for k, v in liste:
            if k in d:
                raise ValueError("doppelter Schluessel '%s'" % k)
            d[k] = v
        return d

    plattform = sys.platform if plattform is None else plattform
    pfad = next((p for pl, p in SYSTEM_PFADE.items() if plattform.startswith(pl)), None)
    if not pfad:
        return None
    try:
        st = os.stat(pfad)
    except OSError:
        return None
    if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_BYTES:
        return None
    if st.st_mode & 0o022 or st.st_uid not in SYSTEM_EIGENTUEMER or not _kette_ok(pfad):
        return None
    try:
        with open(pfad, 'r', encoding='utf-8') as f:
            daten = json.load(f, object_pairs_hook=paare)
    except (OSError, ValueError, UnicodeDecodeError):
        return None
    if not isinstance(daten, dict) or daten.get('schema') != SCHEMA:
        return None
    g = daten.get('gesperrt', [])
    if not (isinstance(g, list) and all(isinstance(x, str) and x for x in g)):
        return None       # wie wb_konfig: eine kaputte Zeile gesperrt macht die ganze Ebene ungueltig
    return _socket_aus_wert(daten.get('tmuxSocket'))


def echte_sockets():
    """Die Sockets, die als echter Werkbank-Server gelten: zuerst der gewaehlte, dann der Standard."""
    gewaehlt = system_socket() or _socket_aus_wert(AUSGEROLLTER_SOCKET) or _standard()
    return [gewaehlt] + ([_standard()] if gewaehlt != _standard() else [])


def ziel(umgebung, argv):
    """Der Socketpfad, an den tmux mit diesen Serverargumenten und dieser Umgebung geht."""
    s = l = None
    i = 0
    argv = list(argv or [])
    while i < len(argv):
        a = argv[i]
        if a in ('-S', '-L'):
            if i + 1 >= len(argv):
                return None
            if a == '-S':
                s = argv[i + 1]
            else:
                l = argv[i + 1]
            i += 2
            continue
        if a.startswith('-S') and len(a) > 2:
            s = a[2:]
        elif a.startswith('-L') and len(a) > 2:
            l = a[2:]
        i += 1
    basis = os.path.join(umgebung.get('TMUX_TMPDIR') or '/tmp', 'tmux-%d' % os.getuid())
    if s:   # tmux: -S vor -L
        return s
    if l:
        if '/' in l:   # tmux setzt <basis>/<label> zusammen, join nicht: lieber echt (fail closed)
            return None
        return os.path.join(basis, l)
    tmux = (umgebung.get('TMUX') or '').split(',')[0]
    return tmux or os.path.join(basis, 'default')


def _gleich(a, b):
    try:
        if os.path.samefile(a, b):
            return True
    except (OSError, ValueError):
        pass
    return os.path.realpath(a) == os.path.realpath(b)


def ziel_echt(umgebung, argv):
    """True, wenn das Ziel der echte Werkbank-Server ist oder sich nicht bestimmen laesst."""
    try:
        z = ziel(umgebung, argv)
        if not z:
            return True
        return any(_gleich(z, e) for e in echte_sockets())
    except Exception:
        return True


def naht_lockert(umgebung, argv):
    """True: Testnaht gesetzt und das Ziel ist nicht der echte Werkbank-Server."""
    return naht_aktiv(umgebung) and not ziel_echt(umgebung, argv)


def naht_sperrt(umgebung, argv):
    """True: Testnaht gesetzt und das Ziel ist der echte Server -- ein schreibender Weg endet mit 78.
    Bewusst am Signal selbst, nicht an naht_aktiv(): auch unter NAHT_ERLAUBT = False tippt ein Lauf mit
    WB_TESTUMGEBUNG=1 nie in den echten Server (Suiten rufen auch das installierte Werkzeug)."""
    return umgebung.get('WB_TESTUMGEBUNG') == '1' and ziel_echt(umgebung, argv)


_GEMELDET = set()


def naht_variable(name, umgebung=None, werkzeug=None, fehler=None):
    """Der Wert der Naht-Variablen `name` (NAHT_VARIABLEN), wenn sie gesetzt ist UND die Testnaht gilt,
    sonst None. Ist sie ohne Naht gesetzt, geht EINE Zeile mit dem Namen (nie dem Wert) auf `fehler`
    (Vorgabe sys.stderr), und der Aufrufer behandelt sie als nicht gesetzt. Ein Prozess meldet jeden
    Namen nur einmal, auch wenn das Werkzeug die Variable an mehreren Stellen liest."""
    if name not in NAHT_VARIABLEN:
        raise ValueError('keine Naht-Variable: %r' % (name,))
    umgebung = os.environ if umgebung is None else umgebung
    wert = umgebung.get(name)
    if not wert:
        return None
    if naht_aktiv(umgebung) or _dev_fassung(umgebung):
        return wert
    if fehler is None:
        if name in _GEMELDET:
            return None
        _GEMELDET.add(name)
    werkzeug = werkzeug or os.path.basename(sys.argv[0] or '') or 'werkbank'
    (sys.stderr if fehler is None else fehler).write(
        '%s: %s ist gesetzt und wird ignoriert (gilt nur unter WB_TESTUMGEBUNG=1); es gilt die Datei der Heimat.\n' % (werkzeug, name))
    return None


def heim_db():
    if HEIM_DB:
        return HEIM_DB
    import pwd
    return pwd.getpwuid(os.getuid()).pw_dir


def echte_dateien(heim=None):
    """Die Registry und die Einstellungen der Heimat laut Benutzerdatenbank."""
    h = heim or heim_db()
    return [os.path.join(h, '.claude', 'workbench', n) for n in ('models.json', 'settings.json')]


def schreib_leck(umgebung=None, heim=None, ziele=()):
    """Die Namen (SPERR_NAMEN, nie Werte), die das Schreiben in Registry oder Einstellungen verbieten.
    Leer: Schreiben erlaubt. `heim` ersetzt HEIM_DB (wb-state gibt seine Heimat-Zeile mit). `ziele` sind die
    Dateien, die der Aufrufer gleich schreibt: unter der Naht ist die ECHTE Datei der Heimat kein Ziel
    (WB_TESTUMGEBUNG im Ergebnis; ein Testlauf mit Naht, der HOME nicht umlenkt, schrieb sonst live)."""
    umgebung = os.environ if umgebung is None else umgebung
    namen = set(n for n in (umgebung.get('WB_SCHREIBSPERRE') or '').split(',') if n in SPERR_NAMEN)
    if umgebung.get('WB_TESTUMGEBUNG') == '1' and ziele:   # Sperre: am Signal selbst, nicht an naht_aktiv()
        try:
            echt = echte_dateien(heim)
            if any(_gleich(z, e) for z in ziele if z for e in echt):
                namen.add('WB_TESTUMGEBUNG')
        except Exception:
            namen.add('WB_TESTUMGEBUNG')    # Heimat nicht bestimmbar: fail closed
    if not naht_aktiv(umgebung):
        namen.update(n for n in NAHT_VARIABLEN if umgebung.get(n))
        home = umgebung.get('HOME')
        if home:
            try:
                if not _gleich(home, heim or heim_db()):
                    namen.add('HOME')
            except Exception:
                namen.add('HOME')   # Heimat nicht bestimmbar: fail closed
    return [n for n in SPERR_NAMEN if n in namen]


def sperrmeldung(werkzeug, namen):
    gruende = []
    for n in namen:
        if n == 'HOME':
            gruende.append('HOME weicht ohne WB_TESTUMGEBUNG=1 von der Heimat laut Benutzerdatenbank ab')
        elif n == 'WB_TESTUMGEBUNG':
            gruende.append('WB_TESTUMGEBUNG=1, aber das Ziel ist die echte Datei der Heimat (HOME nicht umgelenkt?)')
        else:
            gruende.append('%s ist ohne WB_TESTUMGEBUNG=1 gesetzt' % n)
    return ('%s: Schreiben abgelehnt (Exit %d): %s. Testlauf ohne WB_TESTUMGEBUNG=1? '
            'Die echte Datei bleibt unveraendert.\n' % (werkzeug, SCHREIB_ABBRUCH, '; '.join(gruende)))


def schreiben_pruefen(werkzeug, umgebung=None, heim=None, fehler=None, ziele=()):
    """0, wenn das Werkzeug schreiben darf; sonst eine Zeile auf `fehler` (Vorgabe sys.stderr) und 79."""
    namen = schreib_leck(umgebung, heim, ziele)
    if not namen:
        return 0
    (sys.stderr if fehler is None else fehler).write(sperrmeldung(werkzeug, namen))
    return SCHREIB_ABBRUCH


def _optionen(rest):
    """--heim PFAD und beliebig viele --ziel PFAD."""
    heim, ziele = None, []
    while rest:
        if len(rest) < 2 or rest[0] not in ('--heim', '--ziel') or not rest[1]:
            raise ValueError
        if rest[0] == '--heim':
            heim = rest[1]
        else:
            ziele.append(rest[1])
        rest = rest[2:]
    return heim, ziele


def main(argv):
    if argv[:1] in (['schreiben'], ['schreibsperre']):
        # schreiben WERKZEUG [--heim PFAD]: Exit 0 erlaubt, 79 abgelehnt (Meldung auf stderr).
        # schreibsperre [--heim PFAD]: die Namen kommagetrennt auf stdout, leer wenn erlaubt; Exit 0.
        try:
            if argv[0] == 'schreiben':
                if len(argv) < 2 or not argv[1] or argv[1].startswith('--'):
                    raise ValueError
                heim, ziele = _optionen(argv[2:])
                return schreiben_pruefen(argv[1], heim=heim, ziele=ziele)
            heim, ziele = _optionen(argv[1:])
            sys.stdout.write(','.join(schreib_leck(heim=heim, ziele=ziele)) + '\n')
            return 0
        except ValueError:
            sys.stderr.write('Aufruf: wb_naht_ziel.py schreiben WERKZEUG [--heim PFAD] [--ziel DATEI ...] | schreibsperre [--heim PFAD] [--ziel DATEI ...]\n')
            return 2
    if argv[:1] == ['aktiv']:
        if len(argv) != 1:
            sys.stderr.write('Aufruf: wb_naht_ziel.py aktiv\n')
            return 2
        return 0 if naht_aktiv() else 1
    if argv[:1] == ['gesetzt']:
        if len(argv) != 1:
            sys.stderr.write('Aufruf: wb_naht_ziel.py gesetzt\n')
            return 2
        return 0 if naht_gesetzt() else 1
    if argv[:1] == ['variable']:
        if len(argv) not in (2, 3) or argv[1] not in NAHT_VARIABLEN:
            sys.stderr.write('Aufruf: wb_naht_ziel.py variable %s [werkzeug]\n' % '|'.join(NAHT_VARIABLEN))
            return 2
        wert = naht_variable(argv[1], werkzeug=argv[2] if len(argv) == 3 else None)
        if wert is not None:
            sys.stdout.write(wert + '\n')
        return 0
    if not argv or argv[0] not in ('lockert', 'echt', 'sockets'):
        sys.stderr.write('Aufruf: wb_naht_ziel.py lockert|echt [tmux-serverargumente] | sockets | aktiv | gesetzt | variable NAME\n')
        return 2
    rest = argv[1:]
    if rest[:1] == ['--']:
        rest = rest[1:]
    if argv[0] == 'sockets':
        sys.stdout.write(''.join(s + '\n' for s in echte_sockets()))
        return 0
    if argv[0] == 'lockert':
        return 0 if naht_lockert(os.environ, rest) else 1
    return 0 if ziel_echt(os.environ, rest) else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
