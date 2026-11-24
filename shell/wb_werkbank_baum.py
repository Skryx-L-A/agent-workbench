"""wb_werkbank_baum -- wo liegt der Werkbank-Baum? Die eine Aufloesung fuer Shell und Python (task, 25.10.2026).

ANLASS. Die Installationsprobe fand den festen Ort ~/AI/claude-workbench an 26 Stellen des
Pakets, waehrend INSTALL.md "Any directory works" sagt. Ein Baum in ~/agent-workbench wurde von awb-ctl, dem Linux-Starter,
wb-mensch, wb-harness-config, wb-hygiene und wb-instructions nicht gefunden. Entscheidung (25.10.2026): ein Pfad, einmal
festgelegt, als Einstellung pfade.werkbankBaum im bestehenden Einstellungsweg (wb_konfig.py, einstellungen-schema.json);
alle Werkzeuge lesen ihn ueber diese eine Stelle.

EIN BAUM ist ein Ordner mit app/ und shell/ (Klon, Worktree oder das Paket agent-workbench).

REIHENFOLGE, der erste Treffer, der ein Baum ist, gewinnt:
  1. die Einstellung pfade.werkbankBaum. Ein Betreiberschluessel: System > Betreiber > Nutzer, nie aus der Umgebung.
     Die Installation schreibt ihn in die Nutzerdatei (setze()); ein Administrator kann ihn in der Systemdatei festlegen.
  2. der eigene Installationsort des aufrufenden Werkzeugs (Verweise aufgeloest): <baum>/shell/<werkzeug> oder
     <baum>/app/bin/<werkzeug>. Eine Kopie in ~/.local/bin hat keinen Baum ueber sich.
  3. ~/AI/claude-workbench, der Ort vor dieser Einstellung.
Zeigt die Einstellung auf keinen Baum, steht das in den Meldungen, und die Suche geht weiter. Ist keiner der drei ein Baum,
gilt Stufe 3 mit Quelle 'fehlt'.

ZWEI HEIMATEN. Gewoehnlich liest die Nutzerebene die Datei unter $HOME (wb_konfig.lade(heim=HOME)); so bleibt eine Suite mit
eigenem HOME von der echten Datei getrennt. streng=True (Linux-Starter, context-guard) nimmt dagegen die Heimat laut
Benutzerdatenbank, nie HOME, und laesst Stufe 2 aus: ein Werkzeug, das einen Schutz bildet, folgt keinem Ort, den der Aufrufer
waehlt.

NUR SYSTEM (Nachtrag 2 des decision 2026-10-26). nur_system=True (wb-mensch, der M2-Menschen-Nachweis) liest Stufe 1 NUR aus der
Systemdatei (root-eigen, mit den Pruefungen von wb_konfig), nie aus Nutzer- oder Betreiberdatei: Agenten laufen unter
demselben Konto und koennen diese Dateien schreiben; ein umgelenkter Kernordner machte den Nachweis faelschbar. Danach
Stufe 2 (der Baum ueber dem Werkzeug) und Stufe 3 unter der Heimat laut Benutzerdatenbank. Der Linux-Starter liest die
Nutzerebene weiter (streng): er startet nur, er bezeugt nichts.

EINE UNBRAUCHBARE EINSTELLUNGSDATEI (KonfigUngueltig) laesst Stufe 1 mit Meldung aus. Der Rueckfall ist der Stand vor dieser
Einstellung und damit nie lockerer.

Nur Python-Standardbibliothek, laeuft ab Python 3.9 (/usr/bin/python3 am Mac). Die Bibliothek schreibt nichts nach stderr.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))
import wb_konfig  # noqa: E402

SCHLUESSEL = 'pfade.werkbankBaum'
VORGABE_RELATIV = os.path.join('AI', 'claude-workbench')


def ist_baum(pfad):
    return bool(pfad) and os.path.isdir(os.path.join(pfad, 'app')) and os.path.isdir(os.path.join(pfad, 'shell'))


def _heimat(streng, env):
    """streng: die Heimat laut Benutzerdatenbank aus wb_konfig (Tests schreiben dort NUTZER_HEIMAT um), sonst HOME."""
    if streng:
        return wb_konfig._heimat_der_kennung()
    return env.get('HOME') or os.path.expanduser('~')


def _ueber_dem_ort(ort):
    """Der Baum ueber dem Werkzeug an `ort` (shell/<werkzeug> oder app/bin/<werkzeug>), sonst None."""
    if not ort:
        return None
    d = os.path.dirname(os.path.realpath(ort))
    for kandidat in (os.path.dirname(d), os.path.dirname(os.path.dirname(d))):
        if ist_baum(kandidat):
            return kandidat
    return None


def _systemwert(k, meldungen):
    """Der Wert NUR aus der Systemdatei (None ohne gueltige Systemdatei oder ohne Schluessel)."""
    s = k._schicht('system')
    if s is None or not s.vorhanden:
        return None
    if s.fehler:
        meldungen.append('%s: Systemdatei %s unbrauchbar (%s); Einstellung ausgelassen' % (SCHLUESSEL, s.pfad, s.fehler))
        return None
    w = wb_konfig._holen(s.daten, wb_konfig.segmente(SCHLUESSEL, k.schema))
    return None if w is wb_konfig._FEHLT else w


def finde(ort=None, streng=False, env=None, meldungen=None, nur_system=False):
    """(pfad, quelle): quelle ist einstellung | ort | vorgabe | fehlt. `ort`: Pfad des aufrufenden Werkzeugs.
    nur_system: Stufe 1 nur aus der Systemdatei, Heimat laut Benutzerdatenbank (siehe NUR SYSTEM)."""
    env = os.environ if env is None else env
    meldungen = [] if meldungen is None else meldungen
    streng = streng or nur_system
    heim = _heimat(streng, env)
    try:
        k = wb_konfig.lade(env=env) if streng else wb_konfig.lade(env=env, heim=os.path.abspath(heim))
        if nur_system:
            wert = _systemwert(k, meldungen)
        else:
            wert = k.hole(SCHLUESSEL, None)
            meldungen.extend(m for m in k.meldungen if SCHLUESSEL in m)
    except wb_konfig.KonfigUngueltig as e:
        meldungen.append('%s: Einstellungsdatei unbrauchbar (%s); Einstellung ausgelassen' % (SCHLUESSEL, e))
        wert = None
    except wb_konfig.SchemaUngueltig as e:
        meldungen.append('%s: Schemadatei unbrauchbar (%s); Einstellung ausgelassen' % (SCHLUESSEL, e))
        wert = None
    if isinstance(wert, str) and wert:
        if wert == '~' or wert.startswith('~/'):
            wert = (heim or '') + wert[1:]
        if ist_baum(wert):
            return os.path.normpath(wert), 'einstellung'
        meldungen.append('%s=%s ist kein Werkbank-Baum (app/ und shell/ fehlen); weiter mit dem naechsten Ort' % (SCHLUESSEL, wert))
    if not streng or nur_system:
        oben = _ueber_dem_ort(ort)
        if oben:
            return oben, 'ort'
    vorgabe = os.path.join(heim or '~', VORGABE_RELATIV)
    return vorgabe, ('vorgabe' if ist_baum(vorgabe) else 'fehlt')


def baum(ort=None, streng=False):
    """Nur der Pfad (auch bei Quelle 'fehlt': dann der Ort von Stufe 3)."""
    return finde(ort, streng)[0]


def setze(pfad, env=None):
    """Schreibt pfade.werkbankBaum in die Nutzerdatei $HOME/.config/agent-workbench/werkbank.json (andere Schluessel bleiben).
    Rueckgabe: (dateipfad, wirksam, quelle). Wirft ValueError, wenn `pfad` kein Baum oder die Nutzerdatei kein Objekt ist."""
    env = os.environ if env is None else env
    pfad = os.path.abspath(os.path.expanduser(pfad))
    if not ist_baum(pfad):
        raise ValueError('%s ist kein Werkbank-Baum (app/ und shell/ fehlen)' % pfad)
    heim = os.path.abspath(_heimat(False, env))
    datei = os.path.join(heim, '.config', 'agent-workbench', wb_konfig.DATEINAME)
    daten = {'schema': wb_konfig.SCHEMA}
    if os.path.exists(datei):
        with open(datei, encoding='utf-8') as f:
            daten = json.load(f)
        if not isinstance(daten, dict) or daten.get('schema') != wb_konfig.SCHEMA:
            raise ValueError("%s ist keine Einstellungsdatei mit schema '%s'; nichts geschrieben" % (datei, wb_konfig.SCHEMA))
    pfade = daten.get('pfade')
    if pfade is None:
        pfade = daten['pfade'] = {}
    if not isinstance(pfade, dict):
        raise ValueError("%s: 'pfade' ist kein Objekt; nichts geschrieben" % datei)
    pfade['werkbankBaum'] = pfad
    os.makedirs(os.path.dirname(datei), mode=0o700, exist_ok=True)
    wb_konfig.schreibe_einstellungsdatei(datei, json.dumps(daten, ensure_ascii=False, indent=2, sort_keys=True) + '\n')
    wirksam, quelle = finde(env=env)
    return datei, wirksam, quelle
